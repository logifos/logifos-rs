#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Check v1 wire envelopes and resource identities; never execute an engine."""
import argparse
import base64
import hashlib
import json
import math
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parent
LIMITS = {'request_bytes': 8388608, 'json_depth': 64, 'source_bytes': 1048576,
          'tree_nodes': 100000, 'tree_depth': 128, 'font_count': 16,
          'font_bytes': 4194304, 'total_font_bytes': 4194304,
          'response_bytes': 8388608, 'glyphs': 100000}


class Invalid(ValueError):
    def __init__(self, code, path, message):
        self.code, self.path = code, path
        super().__init__(message)


def fail(code, path, message):
    raise Invalid(code, path, message)


def object_pairs(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            fail('INVALID_JSON', '', 'duplicate object key')
        value[key] = item
    return value


def decode(raw, byte_limit):
    if len(raw) > byte_limit:
        fail('RESOURCE_LIMIT', '', 'message byte limit exceeded')
    try:
        text = raw.decode('utf-8', errors='strict')
    except UnicodeDecodeError:
        fail('INVALID_UTF8', '', 'message is not UTF-8')
    # Bound nesting before json.loads allocates nested containers.
    depth, quoted, escaped = 0, False, False
    for ch in text:
        if quoted:
            if escaped:
                escaped = False
            elif ch == '\\':
                escaped = True
            elif ch == '"':
                quoted = False
        elif ch == '"':
            quoted = True
        elif ch in '[{':
            depth += 1
            if depth > LIMITS['json_depth']:
                fail('RESOURCE_LIMIT', '', 'JSON depth limit exceeded')
        elif ch in ']}':
            depth -= 1
    try:
        value = json.loads(text, object_pairs_hook=object_pairs,
                           parse_constant=lambda _: fail('INVALID_JSON', '', 'nonfinite JSON number'))
    except (json.JSONDecodeError, RecursionError, ValueError) as exc:
        if isinstance(exc, Invalid):
            raise
        fail('INVALID_JSON', '', 'malformed JSON')
    stack = [value]
    while stack:
        item = stack.pop()
        if isinstance(item, dict):
            stack.extend(item.keys())
            stack.extend(item.values())
        elif isinstance(item, list):
            stack.extend(item)
        elif isinstance(item, str):
            try:
                item.encode('utf-8', errors='strict')
            except UnicodeEncodeError:
                fail('INVALID_UTF8', '', 'unpaired Unicode surrogate')
        elif isinstance(item, float) and not math.isfinite(item):
            fail('INVALID_JSON', '', 'nonfinite JSON number')
    return value


def matches(value, schema, root, path=''):
    """The JSON Schema subset used in the two shipped envelope schemas."""
    if '$ref' in schema:
        target = root
        for part in schema['$ref'].removeprefix('#/').split('/'):
            target = target[part]
        return matches(value, target, root, path)
    if 'oneOf' in schema:
        count = 0
        for candidate in schema['oneOf']:
            try:
                matches(value, candidate, root, path)
                count += 1
            except Invalid:
                pass
        if count != 1:
            fail('INVALID_REQUEST', path, 'expected exactly one envelope shape')
        return
    types = {'object': lambda x: isinstance(x, dict),
             'array': lambda x: isinstance(x, list),
             'string': lambda x: isinstance(x, str),
             'integer': lambda x: type(x) is int,
             'number': lambda x: (type(x) is int and -sys.float_info.max <= x <= sys.float_info.max) or (type(x) is float and math.isfinite(x)),
             'boolean': lambda x: type(x) is bool, 'null': lambda x: x is None}
    if 'type' in schema and not types[schema['type']](value):
        fail('INVALID_REQUEST', path, 'wrong JSON type')
    if 'const' in schema and (type(value) is not type(schema['const']) or value != schema['const']):
        fail('INVALID_REQUEST', path, 'unexpected constant')
    if 'enum' in schema and value not in schema['enum']:
        fail('INVALID_REQUEST', path, 'unknown value')
    if isinstance(value, dict):
        props = schema.get('properties', {})
        if any(key not in value for key in schema.get('required', [])):
            fail('INVALID_REQUEST', path, 'missing required field')
        if schema.get('additionalProperties') is False and set(value) - set(props):
            fail('INVALID_REQUEST', path, 'unknown field')
        for key, item in value.items():
            if key in props:
                matches(item, props[key], root, path + '/' + key)
    if isinstance(value, list):
        if len(value) < schema.get('minItems', 0) or len(value) > schema.get('maxItems', sys.maxsize):
            fail('INVALID_REQUEST', path, 'array length outside bounds')
        for i, item in enumerate(value):
            if 'items' in schema:
                matches(item, schema['items'], root, path + '/' + str(i))
    if isinstance(value, str):
        if len(value) < schema.get('minLength', 0) or len(value) > schema.get('maxLength', sys.maxsize):
            fail('INVALID_REQUEST', path, 'string length outside bounds')
        if 'pattern' in schema and re.fullmatch(schema['pattern'], value) is None:
            fail('INVALID_REQUEST', path, 'string pattern mismatch')
    if type(value) in (float, int):
        if value < schema.get('minimum', -math.inf) or value > schema.get('maximum', math.inf):
            fail('INVALID_REQUEST', path, 'number outside bounds')
        if 'exclusiveMinimum' in schema and value <= schema['exclusiveMinimum']:
            fail('INVALID_REQUEST', path, 'number outside bounds')


def external_pin(identity, name, pins):
    pin = pins[name]
    fields = ('origin', 'revision', 'schema_path', 'sha256')
    if any(not pin.get(key) for key in fields):
        fail('UNPINNED_CONTRACT', '/' + name, 'external contract pin is unresolved')
    if any(identity.get(key) != pin[key] for key in fields):
        fail('CONTRACT_MISMATCH', '/' + name, 'external contract identity mismatch')
    # This checker has no external-schema loader. Pins alone never qualify a model.
    fail('NOT_IMPLEMENTED', '/' + name, 'external payload validator must be supplied by the owning card')


def validate_request(raw):
    value = decode(raw, LIMITS['request_bytes'])
    if isinstance(value, dict):
        for field, code in [('abi_version', 'UNSUPPORTED_ABI'), ('schema_version', 'UNSUPPORTED_SCHEMA')]:
            if field in value and (type(value[field]) is not int or value[field] != 1):
                fail(code, '/' + field, 'unsupported version')
        options = value.get('options', {})
        known = {'limits', 'font_size_px', 'display', 'color', 'fonts', 'fallback_order'}
        if isinstance(options, dict) and set(options) - known:
            fail('UNKNOWN_OPTION', '/options', 'unknown per-call option')
    schema = json.loads((ROOT / 'request.schema.json').read_text())
    matches(value, schema, schema)
    options = value.get('options', {})
    limits = {**LIMITS, **options.get('limits', {})}
    # Apply caller reductions as well as immutable ceilings.
    decode(raw, limits['request_bytes'])
    depth_stack = [(value, 0)]
    while depth_stack:
        item, parent_depth = depth_stack.pop()
        if isinstance(item, (dict, list)):
            depth = parent_depth + 1
            if depth > limits['json_depth']:
                fail('RESOURCE_LIMIT', '/options/limits/json_depth', 'per-call JSON depth exceeded')
            depth_stack.extend((x, depth) for x in (item.values() if isinstance(item, dict) else item))
    if value['op'] == 'render' and value['output'] == 'png':
        fail('UNSUPPORTED_OUTPUT', '/output', 'PNG is unavailable in the core')
    if 'source' in value and len(value['source'].encode('utf-8')) > limits['source_bytes']:
        fail('RESOURCE_LIMIT', '/source', 'source byte limit exceeded')
    fonts = options.get('fonts', [])
    if len(fonts) > limits['font_count']:
        fail('RESOURCE_LIMIT', '/options/fonts', 'font count limit exceeded')
    ids, total = [], 0
    for index, font in enumerate(fonts):
        path = '/options/fonts/' + str(index)
        if font['id'] in ids:
            fail('INVALID_FONT', path + '/id', 'duplicate font resource id')
        ids.append(font['id'])
        try:
            data = base64.b64decode(font['bytes_base64'], validate=True)
        except (ValueError, base64.binascii.Error):
            fail('INVALID_FONT', path, 'invalid base64 font bytes')
        if base64.b64encode(data).decode('ascii') != font['bytes_base64']:
            fail('INVALID_FONT', path, 'noncanonical base64 font bytes')
        total += len(data)
        if len(data) > limits['font_bytes'] or total > limits['total_font_bytes']:
            fail('RESOURCE_LIMIT', path, 'font byte limit exceeded')
        if hashlib.sha256(data).hexdigest() != font['sha256']:
            fail('FONT_HASH_MISMATCH', path + '/sha256', 'font hash mismatch')
        # Fonts require Fontisan validation. A wire checker cannot prove font validity.
        if not data:
            fail('INVALID_FONT', path, 'empty font resource')
    order = options.get('fallback_order', [])
    if len(set(order)) != len(order) or set(order) != set(ids):
        fail('INVALID_FONT', '/options/fallback_order', 'fallback must list each font id exactly once')
    if value['op'] == 'render' and not fonts:
        fail('MISSING_FONT', '/options/fonts', 'rendering requires caller-provided fonts')
    pins = json.loads((ROOT / 'pins.json').read_text())
    if value['op'] == 'render':
        external_pin(value['formula_contract'], 'formula', pins)
    if value['op'] == 'transform':
        names = ['mathml_syntax', 'formula'] if value['direction'] == 'syntax_to_formula' else ['formula', 'mathml_syntax']
        external_pin(value['input_contract'], names[0], pins)
        external_pin(value['output_contract'], names[1], pins)
    return value


def validate_response(raw):
    value = decode(raw, LIMITS['response_bytes'])
    schema = json.loads((ROOT / 'response.schema.json').read_text())
    matches(value, schema, schema)
    if value['ok']:
        result = value['result']
        if result['kind'] == 'svg':
            metrics = result['metrics']
            if metrics['baseline'] != metrics['height'] or metrics['ink_bounds']['width'] < 0 or metrics['ink_bounds']['height'] < 0:
                fail('INVALID_REQUEST', '/result/metrics', 'invalid geometry units or bounds')
            # SVG safety/content/determinism requires the engine's structural gate.
            fail('NOT_IMPLEMENTED', '/result/svg', 'SVG structural validator is not part of card 09')
        if result['kind'] in ('syntax', 'formula'):
            name = 'mathml_syntax' if result['kind'] == 'syntax' else 'formula'
            external_pin(result[name.replace('mathml_', '') + '_contract'], name,
                         json.loads((ROOT / 'pins.json').read_text()))
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('message', type=Path)
    parser.add_argument('--response', action='store_true')
    args = parser.parse_args()
    try:
        (validate_response if args.response else validate_request)(args.message.read_bytes())
    except Invalid as exc:
        print(json.dumps({'valid': False, 'code': exc.code, 'path': exc.path, 'message': str(exc)}))
        return 1
    except OSError as exc:
        print(json.dumps({'valid': False, 'code': 'IO_ERROR', 'message': str(exc)}))
        return 2
    print(json.dumps({'valid': True, 'scope': 'wire envelope and resource identity only; no engine execution'}))
    return 0


if __name__ == '__main__':
    sys.exit(main())
