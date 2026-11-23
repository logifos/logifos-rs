#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Validate/select the shared corpus without pretending to execute an engine."""
import argparse
import copy
import hashlib
import json
import re
import shutil
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MAX_METADATA = 2 * 1024 * 1024
MAX_CASE = 64 * 1024


class Invalid(ValueError):
    pass


def require(condition, message):
    if not condition:
        raise Invalid(message)


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, 'duplicate JSON key: ' + key)
        result[key] = value
    return result


def read_json(path):
    require(path.is_file(), 'missing file: ' + str(path))
    require(path.stat().st_size <= MAX_METADATA, 'metadata too large: ' + str(path))
    try:
        return json.loads(path.read_text(encoding='utf-8'), object_pairs_hook=unique_object,
                          parse_constant=lambda value: (_ for _ in ()).throw(Invalid('nonfinite JSON: ' + value)))
    except (UnicodeError, json.JSONDecodeError) as error:
        raise Invalid('invalid UTF-8/JSON: ' + str(path)) from error


def schema_check(value, schema, location='$'):
    """The deliberately small JSON Schema subset used by manifest.schema.json."""
    types = {'object': lambda v: isinstance(v, dict), 'array': lambda v: isinstance(v, list),
             'string': lambda v: isinstance(v, str), 'null': lambda v: v is None,
             'integer': lambda v: type(v) is int, 'boolean': lambda v: type(v) is bool}
    if 'type' in schema:
        allowed = schema['type'] if isinstance(schema['type'], list) else [schema['type']]
        require(any(types[t](value) for t in allowed), location + ': wrong type')
    if 'const' in schema:
        require(type(value) is type(schema['const']) and value == schema['const'], location + ': wrong constant')
    if 'enum' in schema:
        require(value in schema['enum'], location + ': unknown value')
    if isinstance(value, str):
        require(len(value) >= schema.get('minLength', 0), location + ': empty string')
        if 'pattern' in schema:
            require(re.fullmatch(schema['pattern'], value) is not None, location + ': wrong pattern')
    if isinstance(value, dict):
        require(set(schema.get('required', [])) <= set(value), location + ': missing fields')
        props = schema.get('properties', {})
        if schema.get('additionalProperties') is False:
            require(set(value) <= set(props), location + ': unknown fields')
        for key, item in value.items():
            if key in props:
                schema_check(item, props[key], location + '.' + key)
    if isinstance(value, list):
        require(len(value) >= schema.get('minItems', 0), location + ': empty array')
        if 'items' in schema:
            for index, item in enumerate(value):
                schema_check(item, schema['items'], location + '[' + str(index) + ']')


def local_file(root, relative):
    require(isinstance(relative, str) and relative, 'empty relative path')
    path = Path(relative)
    require(not path.is_absolute() and '..' not in path.parts, 'unsafe relative path: ' + relative)
    target = root / path
    require(not any(p.is_symlink() for p in [target, *target.parents] if p != root.parent),
            'symlink path: ' + relative)
    require(target.resolve().is_relative_to(root.resolve()), 'path leaves corpus')
    require(target.is_file(), 'missing file: ' + relative)
    return target


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def validate(manifest_path):
    root = manifest_path.parent
    manifest = read_json(manifest_path)
    schema_check(manifest, read_json(root / 'manifest.schema.json'))
    pin = manifest['formula_contract']
    if pin['status'] == 'pinned':
        require(all(pin[x] for x in ['revision', 'schema_path', 'sha256']), 'incomplete Formula pin')
    else:
        require(all(pin[x] is None for x in ['revision', 'schema_path', 'sha256']), 'unresolved Formula pin must stay null')
    inventory_path = local_file(root, manifest['source_inventory'])
    require(digest(inventory_path) == manifest['source_inventory_sha256'], 'inventory hash mismatch')
    inventory = read_json(inventory_path)
    require(inventory.get('format_version') == 1, 'unknown inventory version')
    require(inventory.get('stage') == 'P1', 'wrong inventory stage')
    require(isinstance(inventory.get('sources'), list) and inventory['sources'], 'empty upstream inventory')
    require(isinstance(inventory.get('gaps'), list), 'missing upstream gap ledger')
    for source in inventory['sources']:
        require(isinstance(source, dict), 'invalid upstream source')
        require(source.get('required_replay') is False or source.get('required_replay') is True, 'missing replay decision')
        require(re.fullmatch(r'[0-9a-f]{40}', source.get('revision', '')) is not None, 'upstream revision not pinned')
        require(source.get('repository', '').startswith('https://github.com/'), 'missing original repository')
        require(isinstance(source.get('inputs'), list), 'missing source input inventory')
        if source['required_replay']:
            require(source.get('coverage') == 'complete' and source['inputs'], 'required upstream coverage incomplete')
            for item in source['inputs']:
                require(item.get('origin_status') == 'eligible', 'uncleared upstream origin')
                require(item.get('original_url', '').startswith('https://'), 'missing direct original input URL')
                require(re.fullmatch(r'[0-9a-f]{64}', item.get('sha256', '')) is not None, 'missing upstream input hash')
                require(item.get('license') and item.get('original_revision'), 'missing upstream license/revision')
    generator = inventory.get('generator', {})
    require(generator.get('path') == 'corpus/conformance/inventory.py', 'unknown inventory generator')
    require(generator.get('sha256') == digest(root / 'inventory.py'), 'inventory generator hash mismatch')
    require(re.fullmatch(r'[0-9a-f]{64}', generator.get('directory_receipt_sha256', '')) is not None, 'missing directory receipt')
    require(generator.get('input_pins') and all(re.fullmatch(r'[0-9a-f]{64}', x.get('sha256', '')) and re.fullmatch(r'[0-9a-f]{40}', x.get('revision', '')) for x in generator['input_pins']), 'missing inventory input pins')
    seen_ids, seen_paths = set(), set()
    for case in manifest['cases']:
        require(case['id'] not in seen_ids and case['path'] not in seen_paths, 'duplicate case id/path')
        seen_ids.add(case['id']); seen_paths.add(case['path'])
        path = local_file(root, case['path'])
        require(0 < path.stat().st_size <= MAX_CASE, 'empty/oversized case: ' + case['id'])
        require(digest(path) == case['sha256'], 'case hash mismatch: ' + case['id'])
        require(case['rule'].startswith(case['stage'] + '.'), 'rule/stage mismatch')
        require(case['formula_revision'] == pin['revision'], 'case Formula pin mismatch')
        if case['origin'] == 'specification-derived':
            require(case['references'] and all(x.startswith('https://www.w3.org/TR/') for x in case['references']), 'missing direct specification reference')
        expected = case['expected']
        if case['stage'] == 'P0':
            require(case['input_kind'] == 'bootstrap-request' and expected['kind'] == 'refusal', 'invalid P0 expectation')
            require(expected['status'] == 'assertion-only' and expected['assertion'] == {'ok': False}, 'invalid bootstrap assertion')
            read_json(path)
        else:
            require(case['input_kind'] == 'mathml' and expected['kind'] == 'syntax', 'invalid P1 expectation')
            require(expected['status'] == 'pending' and expected['assertion'] is None, 'unverified syntax output must remain pending')
            try:
                path.read_text(encoding='utf-8')
            except UnicodeError as error:
                raise Invalid('case is not UTF-8') from error
    tracked = generator.get('tracked_input', {})
    require(tracked.get('path') == 'corpus/conformance/source-metadata.json' and tracked.get('sha256') == digest(root / 'source-metadata.json'), 'stale source metadata receipt')
    return manifest, inventory


def p1_gaps(manifest, inventory):
    gaps = list(inventory['gaps'])
    if manifest['formula_contract']['status'] != 'pinned':
        gaps.append('Formula contract revision/schema/hash are unresolved')
    if not all(s['required_replay'] and s.get('coverage') == 'complete' for s in inventory['sources']):
        gaps.append('upstream mathml-stage input coverage is not eligible for required replay')
    gaps.append('engine replay and independently verified syntax/Formula results are not implemented by this validator')
    return gaps


def self_test(manifest_path):
    validate(manifest_path)
    tests = 0
    with tempfile.TemporaryDirectory(prefix='conformance-') as tmp:
        base = Path(tmp) / 'corpus'
        shutil.copytree(manifest_path.parent, base)
        original = read_json(base / 'manifest.json')
        def reject(name, alter):
            nonlocal tests
            candidate = copy.deepcopy(original)
            alter(candidate)
            (base / 'manifest.json').write_text(json.dumps(candidate))
            try:
                validate(base / 'manifest.json')
            except (Invalid, OSError):
                tests += 1
            else:
                raise Invalid('self-test unexpectedly accepted ' + name)
        reject('empty corpus', lambda m: m.update(cases=[]))
        reject('wrong hash', lambda m: m['cases'][0].update(sha256='0' * 64))
        reject('missing corpus', lambda m: m['cases'][0].update(path='cases/absent.json'))
        reject('unsafe path', lambda m: m['cases'][0].update(path='../outside.json'))
        reject('unknown version', lambda m: m.update(format_version=2))
        reject('unknown field', lambda m: m.update(extra=True))
        reject('duplicate case', lambda m: m['cases'].append(copy.deepcopy(m['cases'][0])))
        reject('unknown origin', lambda m: m['cases'][0].update(origin='unknown'))
        reject('missing origin', lambda m: m['cases'][0].pop('origin'))
        reject('missing references', lambda m: m['cases'][3].pop('references'))
        reject('empty specification references', lambda m: m['cases'][3].update(references=[]))
        reject('non-array references', lambda m: m['cases'][3].update(references='https://www.w3.org/TR/MathML3/'))
        reject('non-string reference', lambda m: m['cases'][3].update(references=[None]))
        reject('empty reference', lambda m: m['cases'][3].update(references=['']))
        reject('insecure specification reference', lambda m: m['cases'][3].update(references=['http://www.w3.org/TR/MathML3/']))
        reject('unrelated specification reference', lambda m: m['cases'][3].update(references=['https://example.com/TR/MathML3/']))
        reject('spoofed specification host', lambda m: m['cases'][3].update(references=['https://www.w3.org.example.com/TR/MathML3/']))
        reject('non-specification W3C reference', lambda m: m['cases'][3].update(references=['https://www.w3.org/Math/']))
        reject('unknown case field', lambda m: m['cases'][0].update(extra=True))
        reject('invented success', lambda m: m['cases'][0]['expected'].update(output='<svg/>'))
        reject('incomplete Formula pin', lambda m: m['formula_contract'].update(status='pinned'))
        reject('inventory hash mismatch', lambda m: m.update(source_inventory_sha256='0' * 64))
        inventory_path = base / original['source_inventory']
        original_inventory = inventory_path.read_bytes()
        for name, mutate in [('empty upstream inventory', lambda i: i.update(sources=[])), ('unqualified upstream replay', lambda i: i['sources'][0].update(required_replay=True)), ('missing generator receipt', lambda i: i.update(generator={})), ('wrong generator hash', lambda i: i['generator'].update(sha256='0' * 64)), ('missing directory receipt', lambda i: i['generator'].update(directory_receipt_sha256='')), ('empty input pins', lambda i: i['generator'].update(input_pins=[])), ('stale metadata receipt', lambda i: i['generator']['tracked_input'].update(sha256='0' * 64))]:
            inventory = json.loads(original_inventory)
            mutate(inventory)
            inventory_path.write_text(json.dumps(inventory))
            candidate = copy.deepcopy(original)
            candidate['source_inventory_sha256'] = digest(inventory_path)
            (base / 'manifest.json').write_text(json.dumps(candidate))
            try:
                validate(base / 'manifest.json')
            except Invalid:
                tests += 1
            else:
                raise Invalid('self-test unexpectedly accepted ' + name)
        inventory_path.write_bytes(original_inventory)
        (base / 'manifest.json').write_text(json.dumps(original))
        path = base / original['cases'][0]['path']
        path.write_bytes(b'')
        candidate = copy.deepcopy(original);candidate['cases'][0]['sha256'] = digest(path)
        (base / 'manifest.json').write_text(json.dumps(candidate))
        try:
            validate(base / 'manifest.json')
        except Invalid:
            tests += 1
        else:
            raise Invalid('self-test accepted an empty input')
    print(json.dumps({'self_tests': tests, 'result': 'pass', 'engine_executed': False}))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manifest', type=Path, default=ROOT / 'corpus/conformance/manifest.json')
    parser.add_argument('--case', action='append', default=[], help='exact case ID (repeatable)')
    parser.add_argument('--stage', choices=['P0', 'P1'])
    parser.add_argument('--self-test', action='store_true')
    parser.add_argument('--check-p1', action='store_true', help='fail until every P1 closure requirement is met')
    parser.add_argument('--replay', action='store_true', help='currently refuses; this tool only validates/selects')
    args = parser.parse_args()
    try:
        if args.self_test:
            self_test(args.manifest)
            return 0
        manifest, inventory = validate(args.manifest)
        ids = {c['id'] for c in manifest['cases']}
        require(set(args.case) <= ids, 'unknown named case: ' + ', '.join(sorted(set(args.case) - ids)))
        selected = [c for c in manifest['cases'] if (not args.case or c['id'] in args.case) and (not args.stage or c['stage'] == args.stage)]
        require(selected, 'selection is empty')
        gaps = p1_gaps(manifest, inventory)
        print(json.dumps({'validated_cases': len(manifest['cases']), 'selected_count': len(selected), 'selected': [c['id'] for c in selected], 'engine_executed': False, 'p1_closed': False, 'p1_gaps': gaps}, sort_keys=True))
        if args.replay or args.check_p1:
            print('REFUSED: ' + ('engine replay is unavailable' if args.replay else '; '.join(gaps)), file=sys.stderr)
            return 2
        return 0
    except (Invalid, OSError, RecursionError) as error:
        print('INVALID: ' + str(error), file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
