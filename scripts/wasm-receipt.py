#!/usr/bin/env python3
"""Build/check an exact source, contract, glue and WASM receipt (stdlib only)."""
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
FILES = {"logifos.js", "logifos.d.ts", "logifos_bg.wasm", "logifos_bg.wasm.d.ts",
         "index.js", "index.d.ts", "package.json", "LICENSE.txt", "THIRD-PARTY-LICENSES.txt"}
CONTRACTS = ("request.schema.json", "response.schema.json", "pins.json")


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def sources():
    paths = [ROOT / name for name in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "LICENSE.txt", "deny.toml")]
    paths += sorted((ROOT / "crates").rglob("Cargo.toml"))
    paths += sorted((ROOT / "crates").rglob("*.rs"))
    paths += [ROOT / name for name in ("scripts/build-wasm.sh", "scripts/check-wasm.sh",
              "scripts/wasm-receipt.py", "crates/logifos-wasm/tests/transport.mjs",
              "crates/logifos-wasm/README.md")]
    return {str(path.relative_to(ROOT)): digest(path) for path in paths}


def artifact_hashes(output):
    result = {}
    if {path.name for path in output.iterdir()} != {"nodejs", "web", "receipt.json"}:
        raise ValueError("unexpected/missing artifact root files")
    for target in ("nodejs", "web"):
        directory = output / target
        if {path.name for path in directory.iterdir()} != FILES:
            raise ValueError("unexpected/missing artifact files: " + target)
        for name in sorted(FILES):
            path = directory / name
            if path.is_symlink() or not path.is_file():
                raise ValueError("artifact must be a regular file")
            result[target + "/" + name] = digest(path)
    return result


def pins():
    lock = (ROOT / "Cargo.lock").read_text()
    match = re.search(r'\[\[package\]\]\nname = "wasm-bindgen"\nversion = "([^"]+)"', lock)
    if not match or match[1] != "0.2.130":
        raise ValueError("lockfile bindgen mismatch")
    compiler = subprocess.check_output(
        ["rustup", "run", "1.95.0", "rustc", "--version"], text=True).strip()
    if not compiler.startswith("rustc 1.95.0 "):
        raise ValueError("WASM compiler mismatch")
    return {"rust": "1.95.0", "wasm_bindgen": match[1], "target": "wasm32-unknown-unknown",
            "memory_maximum_bytes": 67_108_864, "profile": "release"}


def dependency_notices():
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--offline", "--format-version", "1"], cwd=ROOT))
    notices = []
    for package in sorted(metadata["packages"], key=lambda p: (p["name"], p["version"])):
        if package["source"] is None:
            continue
        directory = Path(package["manifest_path"]).parent
        notice = None
        for name in ("LICENSE-MIT", "LICENSE-MIT.txt", "LICENSE", "LICENSE.txt"):
            path = directory / name
            if path.is_file() and "Permission is hereby granted" in path.read_text():
                notice = path.read_text()
                break
        if notice is None:
            raise ValueError("missing dependency MIT notice: " + package["name"])
        label = package["name"] + " " + package["version"]
        notices.append(label + " (MIT)\n" + notice)
        if package["name"] == "unicode-ident":
            notices.append(label + " (Unicode-3.0)\n" + (directory / "LICENSE-UNICODE").read_text())
    return "\n\n".join(notices) + "\n"


def prepare(output):
    notices = dependency_notices()
    for target in ("nodejs", "web"):
        directory = output / target
        package = {"name": "@logifos/wasm-transport", "version": "0.1.0", "private": True,
                   "license": "MIT", "type": "commonjs" if target == "nodejs" else "module",
                   "main": "./index.js", "types": "./index.d.ts",
                   "exports": {".": {"types": "./index.d.ts", "default": "./index.js"}},
                   "files": sorted(FILES - {"package.json"})}
        (directory / "package.json").write_text(json.dumps(package, indent=2) + "\n")
        (directory / "LICENSE.txt").write_bytes((ROOT / "LICENSE.txt").read_bytes())
        (directory / "THIRD-PARTY-LICENSES.txt").write_text(notices)
        # Reject huge JS arrays before bindgen copies the complete input. The
        # refusal still comes from the same Rust facade using a bounded probe.
        body = '''function checked(bytes) {
    if (!(bytes instanceof Uint8Array)) throw new TypeError('expected Uint8Array');
    if (bytes.byteLength > wasm.max_request_bytes()) {
        return wasm.request_json(new Uint8Array(wasm.max_request_bytes() + 1));
    }
    return wasm.request_json(bytes);
}
'''
        if target == "nodejs":
            body = "const wasm = require('./logifos.js');\n" + body + "exports.request_json = checked;\nexports.max_request_bytes = wasm.max_request_bytes;\n"
            declarations = "export { max_request_bytes, request_json } from './logifos';\n"
        else:
            body = "import init, * as wasm from './logifos.js';\n" + body + "export { init as default };\nexport { checked as request_json };\nexport const max_request_bytes = wasm.max_request_bytes;\n"
            declarations = "export { default, max_request_bytes, request_json } from './logifos';\n"
        (directory / "index.js").write_text(body)
        (directory / "index.d.ts").write_text(declarations)
    # The receipt has no timestamp or mutable branch/ref claims.
    receipt = {"receipt_version": 1, "qualification": "transport-only",
               "abi_version": 1, "schema_version": 1, "tools": pins(),
               "source_sha256": sources(),
               "contracts_sha256": {name: digest(CONTRACT_DIR / name) for name in CONTRACTS}}
    (output / "receipt.json").write_text("{}\n")
    receipt["artifacts_sha256"] = artifact_hashes(output)
    (output / "receipt.json").write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n")


def check(output):
    receipt = json.loads((output / "receipt.json").read_text())
    expected = {"receipt_version": 1, "qualification": "transport-only", "abi_version": 1,
                "schema_version": 1, "tools": pins(), "source_sha256": sources(),
                "contracts_sha256": {name: digest(CONTRACT_DIR / name) for name in CONTRACTS},
                "artifacts_sha256": artifact_hashes(output)}
    if receipt != expected:
        raise ValueError("receipt does not match exact sources, contracts, tools or artifacts")
    for target in ("nodejs", "web"):
        if (output / target / "logifos_bg.wasm").read_bytes()[:8] != b"\x00asm\x01\x00\x00\x00":
            raise ValueError("invalid WASM magic/version")
    print("receipt: exact source/lock/toolchain/contracts and 18 artifact hashes OK")


if __name__ == "__main__":
    try:
        if len(sys.argv) != 4 or sys.argv[1] not in ("create", "check"):
            raise ValueError("usage: wasm-receipt.py create|check ARTIFACT_DIRECTORY CONTRACT_DIRECTORY")
        output = Path(sys.argv[2]).resolve()
        CONTRACT_DIR = Path(sys.argv[3]).resolve()
        if sys.argv[1] == "create":
            prepare(output)
        else:
            check(output)
    except (OSError, ValueError, KeyError) as error:
        print("receipt refused: " + str(error), file=sys.stderr)
        sys.exit(1)
