#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Generate source inventory from a fresh tracked metadata transcript only."""
import argparse
import hashlib
import json
import re
from pathlib import Path

REPO = 'https://github.com/plurimath/plurimath-testsuite'
REVISION = '27e5493203290b7910aaccf4efc2230540660cea'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def generate(metadata):
    data = json.loads(metadata.read_text(encoding='utf-8'))
    if data['repository'] != REPO or data['revision'] != REVISION:
        raise ValueError('metadata revision differs from reviewed revision')
    source = data['sources'][0]
    ids = [item['id'] for item in source['inputs']]
    paths = source['stage_payload_paths']
    if len(ids) != 260 or len(set(ids)) != 260 or not all(re.fullmatch('mathml-[a-z0-9-]+', ident) for ident in ids):
        raise ValueError('unexpected or incomplete seed metadata')
    if len(paths) != 27 or len(set(paths)) != 27 or not all(p.startswith('stages/mathml/generations/000003/payloads/') and p.endswith('.yaml') for p in paths):
        raise ValueError('unexpected stage directory metadata')
    if source['required_replay'] or source['coverage'] != 'metadata-only' or any(item['origin_status'] != 'unresolved' for item in source['inputs']):
        raise ValueError('metadata transcript must not enable unverified upstream inputs')
    return {'format_version': 1, 'stage': 'P1',
            'generator': {'path': 'corpus/conformance/inventory.py', 'sha256': sha(Path(__file__)),
                          'input_pins': data['source_pins'],
                          'tracked_input': {'path': 'corpus/conformance/source-metadata.json', 'sha256': sha(metadata)},
                          'directory_receipt_sha256': data['directory_receipt_sha256']},
            'sources': data['sources'], 'gaps': data['gaps']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--metadata', type=Path, default=Path(__file__).with_name('source-metadata.json'))
    args = parser.parse_args()
    print(json.dumps(generate(args.metadata), indent=2))


if __name__ == '__main__':
    main()
