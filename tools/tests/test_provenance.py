"""Fresh synthetic provenance tests; no external source trees or fixtures (MIT)."""

import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


CHECKER = Path(__file__).resolve().parents[1] / "provenance.py"
SPEC = importlib.util.spec_from_file_location("provenance", CHECKER)
provenance = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(provenance)


def entry(**overrides):
    result = {
        "author": "Synthetic Logifos test author",
        "origin": "fresh-logifos",
        "kind": "implementation",
        "description": "Fresh synthetic bytes authored for this test.",
        "specification_refs": [],
        "license": "MIT",
    }
    result.update(overrides)
    return result


class ProvenanceTests(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory(prefix="provenance-")
        self.addCleanup(self.scratch.cleanup)
        self.repo = Path(self.scratch.name)
        self.git("init", "-q")
        self.git("config", "user.name", "Synthetic author")
        self.git("config", "user.email", "synthetic@example.invalid")
        self.write("seed.txt", b"independently authored seed\n")
        self.git("add", "--", "seed.txt")
        self.git("commit", "-qm", "Synthetic base")
        self.base = self.git("rev-parse", "HEAD").strip()
        self.document = {
            "schema_version": 1,
            "declaration": {
                "author": "Synthetic Logifos test author",
                "no_renderer_source_data_output_copied": True,
                "statement": "I wrote these synthetic examples independently; no renderer source, data or output was copied.",
            },
            "files": {"provenance.json": entry(kind="infrastructure", review_signals={"attribution": "Attribution signal explanations are synthetic test metadata."})},
        }

    def git(self, *args):
        env = dict(os.environ, GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull)
        result = subprocess.run(["git", "-C", str(self.repo), *args], env=env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        return result.stdout

    def write(self, name, data, provenance_entry=None):
        target = self.repo / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
        if provenance_entry is not None:
            self.document["files"][name] = provenance_entry

    def finish(self):
        self.write("provenance.json", json.dumps(self.document).encode())
        self.git("add", "--", ".")
        self.git("commit", "-qm", "Synthetic head")
        return self.git("rev-parse", "HEAD").strip()

    def check(self, head=None):
        return provenance.check(self.repo, self.base, head or self.finish(), "provenance.json")

    def rejected(self, text):
        with self.assertRaisesRegex(provenance.Invalid, text):
            self.check()

    def generated(self):
        self.write("generator.py", b"# Fresh Logifos synthetic generator\n", entry())
        self.write("input.txt", b"synthetic input\n", entry(kind="fixture", source_trail="Hand-authored test input; no external source."))
        output = b"synthetic result\x00\xff"
        pin = lambda name: {"path": name, "sha256": hashlib.sha256((self.repo / name).read_bytes()).hexdigest()}
        record = entry(origin="generated-logifos", kind="generated", receipt={
            "command": "python3 generator.py input.txt",
            "generator": pin("generator.py"),
            "inputs": [pin("input.txt")],
            "output_sha256": hashlib.sha256(output).hexdigest(),
        })
        self.write("output.bin", output, record)
        return record

    def test_accepts_fresh_authored_infrastructure(self):
        self.write("tools/fresh.py", b"answer = 42\n", entry(kind="infrastructure"))
        self.assertEqual(self.check()["files_checked"], 2)

    def test_missing_provenance(self):
        self.write("missing.rs", b"// Fresh synthetic case\n")
        self.rejected("missing provenance")

    def test_manifest_also_needs_provenance(self):
        del self.document["files"]["provenance.json"]
        self.rejected("missing provenance")

    def test_forbidden_origin(self):
        self.write("case.rs", b"// Fresh synthetic negative example\n", entry(origin="renderer-derived"))
        self.rejected("forbidden or unknown origin")

    def test_unknown_origin(self):
        self.write("case.rs", b"// Example\n", entry(origin="unknown"))
        self.rejected("forbidden or unknown origin")

    def test_missing_author(self):
        self.document["files"]["provenance.json"]["author"] = ""
        self.rejected("missing or unknown author")

    def test_non_mit_file(self):
        self.document["files"]["provenance.json"]["license"] = "unknown"
        self.rejected("MIT licensing")

    def test_missing_declaration(self):
        self.document.pop("declaration")
        self.rejected("explicit no-copy declaration")

    def test_false_declaration(self):
        self.document["declaration"]["no_renderer_source_data_output_copied"] = False
        self.rejected("explicit no-copy declaration")

    def test_missing_declaration_author(self):
        self.document["declaration"].pop("author")
        self.rejected("declaration author")

    def test_unknown_declaration_statement(self):
        self.document["declaration"]["statement"] = "unknown"
        self.rejected("authorship declaration")

    def test_fixture_needs_source_trail(self):
        self.write("fixture.txt", b"fresh case\n", entry(kind="fixture"))
        self.rejected("fixture/table source trail")

    def test_specification_needs_direct_reference(self):
        self.write("case.txt", b"fresh case\n", entry(origin="public-specification"))
        self.rejected("edition, section and source")

    def test_specification_needs_edition(self):
        self.write("case.txt", b"fresh case\n", entry(origin="public-specification", specification_refs=[{"url": "https://example.invalid/spec", "section": "1"}]))
        self.rejected("specification edition")

    def test_accepts_specification_reference(self):
        self.write("case.txt", b"fresh case\n", entry(origin="public-specification", kind="fixture", source_trail="Hand-derived from the listed section.", specification_refs=[{"url": "https://example.invalid/spec", "edition": "synthetic-1", "section": "1"}]))
        self.assertEqual(self.check()["files_checked"], 2)

    def test_generated_needs_receipt(self):
        self.write("output.bin", b"fresh generated bytes", entry(origin="generated-logifos", kind="generated"))
        self.rejected("generator receipt")

    def test_accepts_generated_receipt(self):
        self.generated()
        self.assertEqual(self.check()["files_checked"], 4)

    def test_generated_output_hash_mismatch(self):
        self.generated()["receipt"]["output_sha256"] = "0" * 64
        self.rejected("output does not match receipt")

    def test_generator_hash_mismatch(self):
        self.generated()["receipt"]["generator"]["sha256"] = "0" * 64
        self.rejected("receipt hash does not match HEAD")

    def test_input_hash_mismatch(self):
        self.generated()["receipt"]["inputs"][0]["sha256"] = "0" * 64
        self.rejected("receipt hash does not match HEAD")

    def test_generated_needs_inputs(self):
        self.generated()["receipt"]["inputs"] = []
        self.rejected("pinned inputs")

    def test_generator_needs_fresh_provenance(self):
        self.generated()
        self.document["files"].pop("generator.py")
        self.rejected("fresh Logifos implementation provenance|missing provenance")

    def test_unchanged_input_needs_provenance(self):
        record = self.generated()
        record["receipt"]["inputs"] = [{"path": "seed.txt", "sha256": hashlib.sha256((self.repo / "seed.txt").read_bytes()).hexdigest()}]
        self.rejected("missing provenance")

    def test_unchanged_input_rejects_forbidden_origin(self):
        record = self.generated()
        record["receipt"]["inputs"] = [{"path": "seed.txt", "sha256": hashlib.sha256((self.repo / "seed.txt").read_bytes()).hexdigest()}]
        self.document["files"]["seed.txt"] = entry(origin="renderer-derived")
        self.rejected("forbidden or unknown origin")

    def test_accepts_unchanged_pinned_input(self):
        record = self.generated()
        record["receipt"]["inputs"] = [{"path": "seed.txt", "sha256": hashlib.sha256((self.repo / "seed.txt").read_bytes()).hexdigest()}]
        self.document["files"]["seed.txt"] = entry()
        self.assertEqual(self.check()["files_checked"], 4)

    def test_generated_input_cycle(self):
        record = self.generated()
        second = json.loads(json.dumps(record))
        self.write("second.bin", b"second synthetic output", second)
        second["receipt"]["output_sha256"] = hashlib.sha256((self.repo / "second.bin").read_bytes()).hexdigest()
        second["receipt"]["inputs"] = [{"path": "output.bin", "sha256": record["receipt"]["output_sha256"]}]
        record["receipt"]["inputs"] = [{"path": "second.bin", "sha256": second["receipt"]["output_sha256"]}]
        self.rejected("receipt cycle")

    def test_dependency_rejects_floating_versions(self):
        for version in ("latest", "main", "^1.2.3", "1.x", ">=1.0", "*"):
            with self.subTest(version=version):
                record = entry(origin="declared-dependency", kind="dependency", dependencies=[{"name": "synthetic", "version": version, "source": "https://example.invalid/package", "license": "MIT"}])
                with self.assertRaisesRegex(provenance.Invalid, "exact numeric version"):
                    provenance.validate_entry(self.repo, self.base, "manifest.txt", record, b"")

    def test_dependency_accepts_exact_version_and_license(self):
        self.write("dependencies.json", b"{}\n", entry(origin="declared-dependency", kind="dependency", dependencies=[{"name": "synthetic", "version": "1.2.3", "source": "https://example.invalid/package", "license": "MIT"}]))
        self.assertEqual(self.check()["files_checked"], 2)

    def test_foreign_reference_needs_review(self):
        self.write("notice.txt", b"Synthetic mention: WebKit\n", entry())
        self.rejected("review explanation for foreign-reference")

    def test_attribution_needs_review(self):
        self.write("notice.txt", b"Copyright synthetic author\n", entry())
        self.rejected("review explanation for attribution")

    def test_accepts_explained_review_signal(self):
        self.write("notice.txt", b"Copyright synthetic author\n", entry(review_signals={"attribution": "Fresh synthetic copyright line; no imported content."}))
        self.assertEqual(self.check()["review_signals"], 2)

    def test_signal_after_leading_plus_signs(self):
        self.write("notice.txt", b"++Copyright synthetic author\n", entry())
        self.rejected("review explanation for attribution")

    def test_path_signal_needs_review(self):
        self.write("notice.txt", b"Synthetic source path /home/example/synthetic\n", entry())
        self.rejected("review explanation for source-path")

    def test_modification_requires_entry(self):
        self.write("seed.txt", b"modified fresh seed\n")
        self.rejected("missing provenance")

    def test_rename_destination_requires_entry(self):
        self.git("mv", "seed.txt", "renamed.txt")
        self.rejected("missing provenance")

    def test_renamed_destination_with_entry(self):
        self.git("mv", "seed.txt", "renamed.txt")
        self.document["files"]["renamed.txt"] = entry()
        self.assertEqual(self.check()["files_checked"], 2)

    def test_binary_requires_provenance(self):
        self.write("binary.dat", b"\x00\xff\x00")
        self.rejected("missing provenance")

    def test_head_manifest_ignores_worktree_edit(self):
        self.write("missing.txt", b"fresh bytes\n")
        head = self.finish()
        self.document["files"]["missing.txt"] = entry()
        self.write("provenance.json", json.dumps(self.document).encode())
        with self.assertRaisesRegex(provenance.Invalid, "missing provenance"):
            self.check(head)

    def test_blob_receipts_ignore_worktree_edit(self):
        self.generated()
        head = self.finish()
        self.write("input.txt", b"different uncommitted input\n")
        self.assertEqual(self.check(head)["files_checked"], 4)

    def test_utf8_space_newline_paths(self):
        self.write("fresh space/\u03b1\ncase.txt", b"fresh bytes\n", entry())
        self.assertEqual(self.check()["files_checked"], 2)

    def test_rejects_symlink(self):
        (self.repo / "link").symlink_to("seed.txt")
        self.document["files"]["link"] = entry()
        self.rejected("regular tracked file")

    def test_invalid_revision(self):
        head = self.finish()
        with self.assertRaisesRegex(provenance.Invalid, "requested revision"):
            provenance.check(self.repo, "no-such-revision", head, "provenance.json")

    def test_bad_schema(self):
        self.document["schema_version"] = 2
        self.rejected("unsupported provenance schema")

    def test_malformed_origin_is_clean_failure(self):
        self.document["files"]["provenance.json"]["origin"] = []
        self.rejected("forbidden or unknown origin")

    def test_cli_exit_codes(self):
        head = self.finish()
        args = [os.sys.executable, str(CHECKER), "--repo", str(self.repo), "--base", self.base, "--head", head, "--manifest", "provenance.json"]
        good = subprocess.run(args, capture_output=True, text=True)
        self.assertEqual(good.returncode, 0, good.stderr)
        self.assertEqual(json.loads(good.stdout)["files_checked"], 1)
        args[args.index("--base") + 1] = "unknown-revision"
        bad = subprocess.run(args, capture_output=True, text=True)
        self.assertEqual(bad.returncode, 1)
        self.assertIn("provenance: FAIL", bad.stderr)


if __name__ == "__main__":
    unittest.main()
