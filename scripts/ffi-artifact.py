#!/usr/bin/env python3
"""Write and verify a local native artifact receipt; never publish artifacts."""

import argparse
import copy
import ctypes
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parent.parent
EXPORTS = {
    'logifos_abi_version', 'logifos_schema_version', 'logifos_features',
    'logifos_engine_version', 'logifos_request_v1', 'logifos_buffer_free_v1',
    'logifos_request_json', 'logifos_string_free',
}


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def build_inputs():
    paths = [ROOT / name for name in ('Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml')]
    paths += list((ROOT / 'crates').glob('*/Cargo.toml'))
    paths += list((ROOT / 'crates').glob('*/src/**/*.rs'))
    paths += [ROOT / 'crates/logifos-ffi/include/logifos.h',
              ROOT / 'crates/logifos-ffi/tests/smoke.c',
              ROOT / 'scripts/check-ffi.sh', ROOT / 'scripts/ffi-artifact.py']
    paths += list((ROOT / '.cargo').glob('config*'))
    paths += list((ROOT / 'crates').glob('*/build.rs'))
    return {str(path.relative_to(ROOT)): sha(path) for path in sorted(set(paths))}


def compiler():
    return subprocess.check_output(['rustc', '-vV'], text=True).strip()


def metadata():
    workspace = json.loads(subprocess.check_output(
        ['cargo', 'metadata', '--format-version=1', '--no-deps', '--locked'],
        cwd=ROOT, text=True))
    version = next(package['version'] for package in workspace['packages']
                   if package['name'] == 'logifos-ffi')
    rustc = compiler()
    return {
        'receipt_version': 1, 'artifact_kind': 'native-cdylib',
        'engine_version': version, 'abi_version': 1, 'schema_version': 1,
        'features': 3, 'qualification': 'contract-transport-only',
        'target': next(line[6:] for line in rustc.splitlines() if line.startswith('host: ')),
        'rustc': rustc,
        'rustflags': os.environ.get('RUSTFLAGS', ''),
        'encoded_rustflags': os.environ.get('CARGO_ENCODED_RUSTFLAGS', ''),
        'build_inputs': build_inputs(), 'exports': sorted(EXPORTS),
    }


def validate(receipt, library):
    expected = metadata()
    expected['artifact'] = {'file': library.name, 'bytes': library.stat().st_size, 'sha256': sha(library)}
    if receipt != expected:
        changed = sorted(key for key in set(expected) | set(receipt) if receipt.get(key) != expected.get(key))
        raise ValueError('receipt mismatch: ' + ', '.join(changed))


def inspect_symbols(library):
    command = ['nm', '-gU'] if sys.platform == 'darwin' else ['nm', '-D', '--defined-only']
    output = subprocess.check_output([*command, str(library)], text=True)
    exports = {line.split()[-1].removeprefix('_') for line in output.splitlines() if line.split()}
    observed = {name for name in exports if name.startswith('logifos_')}
    if observed != EXPORTS:
        raise ValueError('native export set does not match header API')


def inspect_queries(library, receipt):
    native = ctypes.CDLL(str(library.resolve()))
    for symbol, kind, key in [
        ('logifos_abi_version', ctypes.c_uint32, 'abi_version'),
        ('logifos_schema_version', ctypes.c_uint32, 'schema_version'),
        ('logifos_features', ctypes.c_uint64, 'features'),
        ('logifos_engine_version', ctypes.c_char_p, 'engine_version'),
    ]:
        query = getattr(native, symbol)
        query.argtypes = []
        query.restype = kind
        value = query()
        if isinstance(value, bytes):
            value = value.decode('utf-8')
        if value != receipt[key]:
            raise ValueError('native query mismatch: ' + key)


def rejection_checks(receipt, library):
    count = 0
    for key in ['abi_version', 'schema_version', 'engine_version', 'features',
                'target', 'rustc', 'build_inputs', 'artifact', 'exports']:
        altered = copy.deepcopy(receipt)
        altered[key] = None
        try:
            validate(altered, library)
        except ValueError:
            count += 1
        else:
            raise ValueError('altered receipt accepted: ' + key)
    with tempfile.TemporaryDirectory(prefix='ffi-receipt-', dir=os.environ.get('TMPDIR')) as scratch:
        changed = Path(scratch) / library.name
        changed.write_bytes(b'invalid shared library')
        try:
            validate(receipt, changed)
        except ValueError:
            count += 1
        else:
            raise ValueError('altered artifact accepted')
    return count


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('mode', choices=['write', 'check', 'test'])
    parser.add_argument('--library', required=True, type=Path)
    parser.add_argument('--receipt', required=True, type=Path)
    args = parser.parse_args()
    library = args.library.resolve(strict=True)
    if args.mode == 'write':
        receipt = metadata()
        receipt['artifact'] = {'file': library.name, 'bytes': library.stat().st_size, 'sha256': sha(library)}
        inspect_symbols(library)
        inspect_queries(library, receipt)
        args.receipt.write_text(json.dumps(receipt, indent=2, sort_keys=True) + '\n')
    else:
        receipt = json.loads(args.receipt.read_text())
        # Verify bytes and source/header identity BEFORE loading executable code.
        validate(receipt, library)
        inspect_symbols(library)
        inspect_queries(library, receipt)
        if args.mode == 'test':
            count = rejection_checks(receipt, library)
            print(f'Artifact receipt: {count} altered receipt/artifact cases rejected')
    print(f'Artifact receipt: {args.mode} OK (native contract transport only)')


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        print(f'Artifact receipt: FAIL: {error}', file=sys.stderr)
        sys.exit(1)
