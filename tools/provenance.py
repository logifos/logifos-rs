#!/usr/bin/env python3
"""Check revision-pinned provenance without executing repository code (MIT)."""

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys


ORIGINS = {"fresh-logifos", "declared-dependency", "public-specification", "generated-logifos"}
SIGNALS = {
    "foreign-reference": re.compile(r"(?i)\b(?:lasem|webkit|mathjax|katex|mathml-render)\b"),
    "source-path": re.compile(r"(?i)(?:/home/|/Users/|[A-Z]:\\|https?://\S+/(?:blob|raw)/)"),
    "attribution": re.compile(r"(?i)(?:copyright|copied\s+from|ported\s+from|adapted\s+from|SPDX-License-Identifier)"),
}
SHA256 = re.compile(r"[0-9a-f]{64}\Z")


class Invalid(Exception):
    """An invalid revision, manifest or provenance claim."""


def git(repo, *args):
    result = subprocess.run(
        ["git", "--no-pager", "-C", str(repo), *args],
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False,
    )
    if result.returncode:
        raise Invalid("Git could not read the requested revision or path")
    return result.stdout


def path(value):
    if not isinstance(value, str) or not value or "\0" in value or "\\" in value:
        raise Invalid("expected a nonempty repository-relative POSIX path")
    parsed = PurePosixPath(value)
    if parsed.is_absolute() or any(p in {".", "..", ".git"} for p in value.split("/")):
        raise Invalid("unsafe repository-relative path")
    if str(parsed) != value:
        raise Invalid("path must be normalized")
    return value


def blob(repo, revision, filename):
    filename = path(filename)
    # Reject symlinks/submodules: provenance receipts identify regular file bytes.
    listing = git(repo, "ls-tree", "-z", revision, "--", ":(literal)" + filename)
    records = [record for record in listing.split(b"\0") if record]
    if len(records) != 1:
        raise Invalid("missing tracked file: " + filename)
    metadata, actual = records[0].split(b"\t", 1)
    mode, kind, object_id = metadata.split()
    if actual != filename.encode("utf-8") or kind != b"blob" or mode not in {b"100644", b"100755"}:
        raise Invalid("expected a regular tracked file: " + filename)
    return git(repo, "cat-file", "blob", object_id.decode("ascii"))


def nonempty(value, label):
    if not isinstance(value, str) or not value.strip() or value.strip().lower() in {"unknown", "todo", "tbd"}:
        raise Invalid("missing or unknown " + label)
    return value


def references(value):
    if not isinstance(value, list):
        raise Invalid("specification_refs must be a list")
    for item in value:
        if not isinstance(item, dict):
            raise Invalid("invalid specification reference")
        for field in ("url", "edition", "section"):
            nonempty(item.get(field), "specification " + field)
        if not item["url"].startswith("https://"):
            raise Invalid("specification references need a direct HTTPS origin")


def pinned(repo, head, item):
    if not isinstance(item, dict) or not SHA256.fullmatch(str(item.get("sha256", ""))):
        raise Invalid("receipt needs a path and lowercase SHA-256 pin")
    data = blob(repo, head, item.get("path"))
    if hashlib.sha256(data).hexdigest() != item["sha256"]:
        raise Invalid("receipt hash does not match HEAD: " + item["path"])
    return item["path"]


def validate_entry(repo, head, filename, entry, data):
    if not isinstance(entry, dict):
        raise Invalid("missing provenance entry")
    nonempty(entry.get("author"), "author")
    origin = entry.get("origin")
    if not isinstance(origin, str) or origin not in ORIGINS:
        raise Invalid("forbidden or unknown origin")
    if entry.get("license") != "MIT":
        raise Invalid("changed files must retain MIT licensing")
    nonempty(entry.get("description"), "origin description")
    references(entry.get("specification_refs"))
    kind = entry.get("kind")
    if not isinstance(kind, str) or kind not in {"implementation", "infrastructure", "documentation", "fixture", "table", "generated", "dependency"}:
        raise Invalid("missing or unknown file kind")
    if origin == "declared-dependency":
        if kind != "dependency":
            raise Invalid("dependency origin is only for dependency manifests")
        dependencies = entry.get("dependencies")
        if not isinstance(dependencies, list) or not dependencies:
            raise Invalid("declared dependencies need exact versions and licenses")
        for dependency in dependencies:
            if not isinstance(dependency, dict):
                raise Invalid("invalid dependency record")
            for field in ("name", "version", "license", "source"):
                nonempty(dependency.get(field), "dependency " + field)
            if not re.fullmatch(r"\d+(?:\.\d+)*(?:[-+][0-9A-Za-z][0-9A-Za-z.-]*)?", dependency["version"]):
                raise Invalid("dependency version must be an exact numeric version, not a range or floating label")
    if origin == "public-specification" and not entry["specification_refs"]:
        raise Invalid("public specification origin needs edition, section and source")
    if kind in {"fixture", "table"}:
        nonempty(entry.get("source_trail"), "fixture/table source trail")
    if (origin == "generated-logifos") != (kind == "generated"):
        raise Invalid("generated files must use generated-logifos origin and kind")
    if kind == "generated":
        receipt = entry.get("receipt")
        if not isinstance(receipt, dict):
            raise Invalid("generated file needs a Logifos generator receipt")
        nonempty(receipt.get("command"), "generator command")
        generator = pinned(repo, head, receipt.get("generator"))
        if generator == filename:
            raise Invalid("generated output cannot be its own generator")
        inputs = receipt.get("inputs")
        if not isinstance(inputs, list) or not inputs:
            raise Invalid("generated file needs pinned inputs")
        for item in inputs:
            if pinned(repo, head, item) == filename:
                raise Invalid("generated output cannot be its own input")
        if receipt.get("output_sha256") != hashlib.sha256(data).hexdigest():
            raise Invalid("generated output does not match receipt")
        return generator
    return None


def check(repo, base, head, manifest):
    base = git(repo, "rev-parse", "--verify", "--end-of-options", base + "^{commit}").decode().strip()
    head = git(repo, "rev-parse", "--verify", "--end-of-options", head + "^{commit}").decode().strip()
    manifest = path(manifest)
    try:
        document = json.loads(blob(repo, head, manifest))
    except (ValueError, UnicodeError) as exc:
        raise Invalid("HEAD manifest is not valid JSON") from exc
    if not isinstance(document, dict) or document.get("schema_version") != 1:
        raise Invalid("unsupported provenance schema")
    declaration = document.get("declaration")
    if not isinstance(declaration, dict) or declaration.get("no_renderer_source_data_output_copied") is not True:
        raise Invalid("explicit no-copy declaration is required")
    nonempty(declaration.get("author"), "declaration author")
    nonempty(declaration.get("statement"), "authorship declaration")
    entries = document.get("files")
    if not isinstance(entries, dict):
        raise Invalid("files must map exact tracked paths to provenance entries")
    for filename in entries:
        path(filename)
    raw = git(repo, "diff", "--no-ext-diff", "--no-textconv", "--no-renames", "--name-only", "-z", "--diff-filter=ACMT", base, head, "--")
    try:
        changed = [item.decode("utf-8") for item in raw.split(b"\0") if item]
    except UnicodeError as exc:
        raise Invalid("changed paths must be UTF-8") from exc
    errors = []
    signals = 0

    def validate_record(filename, ancestors=()):
        if filename in ancestors or len(ancestors) >= 64:
            raise Invalid("generated receipt cycle or depth over 64")
        data = blob(repo, head, filename)
        entry = entries.get(filename)
        generator = validate_entry(repo, head, filename, entry, data)
        if generator:
            generator_entry = entries.get(generator)
            if not isinstance(generator_entry, dict) or generator_entry.get("origin") != "fresh-logifos" or generator_entry.get("kind") != "implementation":
                raise Invalid("generator must have fresh Logifos implementation provenance")
            ancestors = ancestors + (filename,)
            validate_record(generator, ancestors)
            for item in entry["receipt"]["inputs"]:
                validate_record(item["path"], ancestors)
        return data, entry

    for filename in changed:
        try:
            data, entry = validate_record(filename)
            patch = git(repo, "diff", "--no-ext-diff", "--no-textconv", "--no-renames", "--unified=0", base, head, "--", ":(literal)" + filename)
            added_lines = []
            in_hunk = False
            for line in patch.decode("utf-8", errors="replace").splitlines():
                if line.startswith("@@"):
                    in_hunk = True
                elif in_hunk and line.startswith("+"):
                    added_lines.append(line[1:])
            added = "\n".join(added_lines)
            found = {name for name, pattern in SIGNALS.items() if pattern.search(added)}
            acknowledgements = entry.get("review_signals", {})
            if not isinstance(acknowledgements, dict):
                raise Invalid("review_signals must map signal names to explanations")
            for signal in found:
                signals += 1
                nonempty(acknowledgements.get(signal), "review explanation for " + signal)
        except Invalid as exc:
            errors.append(filename + ": " + str(exc))
    if errors:
        raise Invalid("\n".join(errors))
    return {"base": base, "head": head, "files_checked": len(changed), "review_signals": signals}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", required=True, type=Path)
    parser.add_argument("--base", required=True)
    parser.add_argument("--head", required=True)
    parser.add_argument("--manifest", required=True)
    args = parser.parse_args()
    try:
        result = check(args.repo, args.base, args.head, args.manifest)
    except (Invalid, OSError) as exc:
        print("provenance: FAIL: " + str(exc), file=sys.stderr)
        return 1
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
