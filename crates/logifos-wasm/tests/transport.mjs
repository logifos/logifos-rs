import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { createRequire } from 'node:module';
import { readFileSync, cpSync, mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';
import test from 'node:test';

const root = resolve(process.env.WASM_ARTIFACT_DIR);
const require = createRequire(import.meta.url);
const node = require(join(root, 'nodejs/index.js'));
const raw = require(join(root, 'nodejs/logifos.js'));
const web = await import(pathToFileURL(join(root, 'web/index.js')));
await web.default({ module_or_path: readFileSync(join(root, 'web/logifos_bg.wasm')) });
const encoder = new TextEncoder();
const decoder = new TextDecoder('utf-8', { fatal: true });
const negotiate = { abi_version: 1, schema_version: 1, op: 'negotiate' };
const parse = { ...negotiate, op: 'parse', format: 'mathml', source: '<math/>' };
const identity = { origin: 'https://example.invalid/formula', revision: 'a'.repeat(40), schema_path: 'formula.json', sha256: '0'.repeat(64) };
const render = { ...negotiate, op: 'render', formula: {}, formula_contract: identity, output: 'svg' };
const wire = value => value instanceof Uint8Array ? value : encoder.encode(typeof value === 'string' ? value : JSON.stringify(value));
function reply(value, runtime = node) {
    const input = wire(value);
    const before = input.slice();
    const output = runtime.request_json(input);
    assert.deepEqual(input, before, 'caller input must stay unchanged');
    assert(output instanceof Uint8Array);
    const result = JSON.parse(decoder.decode(output));
    assert.equal(result.abi_version, 1);
    assert.equal(result.schema_version, 1);
    if (result.ok) assert.equal(result.result.kind, 'capabilities', 'only transport may succeed');
    else {
        assert.deepEqual(Object.keys(result.error).sort(), ['code', 'message', 'path', 'stage']);
        assert.equal(typeof result.error.path, 'string');
        assert(result.error.message.length > 0 && result.error.message.length <= 1024);
        assert(!('result' in result));
    }
    return result;
}
function refuses(input, code) {
    const result = reply(input);
    assert.equal(result.ok, false);
    assert.equal(result.error.code, code);
    return result;
}
const font = data => ({ id: 'test', sha256: createHash('sha256').update(data).digest('hex'), bytes_base64: data.toString('base64'), face_index: 0 });
const withFont = (data, limits = {}) => ({ ...negotiate, options: { fonts: [font(data)], fallback_order: ['test'], limits } });
const limits = reply(negotiate).result.limits;

// Every name describes an executed behavior, not a rendering assertion.
test('negotiate: no engine activation, exact wire versions, isolated per-call options', () => {
    assert.deepEqual(reply(negotiate).result.implemented_operations, []);
    assert.deepEqual(reply(negotiate).result.outputs, []);
    assert.equal(reply(negotiate).result.formula_contract, null);
    refuses({ ...negotiate, abi_version: 2 }, 'UNSUPPORTED_ABI');
    refuses({ ...negotiate, schema_version: 2 }, 'UNSUPPORTED_SCHEMA');
    refuses({ ...negotiate, abi_version: 1.0, schema_version: '1' }, 'UNSUPPORTED_SCHEMA');
    refuses({ ...negotiate, options: { mystery: true } }, 'UNKNOWN_OPTION');
    refuses({ ...negotiate, unknown: true }, 'INVALID_REQUEST');
    refuses({ op: 'render' }, 'INVALID_REQUEST');
    refuses({ ...negotiate, op: 'unknown' }, 'INVALID_REQUEST');
    assert.equal(reply(negotiate).ok, true);
});

test('malformed-request: invalid UTF-8, BOM, duplicate keys, surrogates, numbers and trailing bytes', () => {
    for (const input of [Uint8Array.of(0xff), '"\\ud800"', '"\\udc00"']) refuses(input, 'INVALID_UTF8');
    for (const input of ['', '{', 'null', '[]', 'true', 'NaN', '1e999', '{}{}', '\ufeff{}',
        '{"op":"negotiate","op":"parse"}', '{"a":{"x":1,"\\u0078":2}}', '{"x":Infinity}']) {
        refuses(input, ['null', '[]', 'true'].includes(input) ? 'INVALID_REQUEST' : 'INVALID_JSON');
    }
});

test('refusal: parse, transform, SVG and PNG never return successful rendering', () => {
    refuses(parse, 'NOT_IMPLEMENTED');
    refuses(render, 'MISSING_FONT');
    refuses({ ...render, output: 'png' }, 'UNSUPPORTED_OUTPUT');
    refuses({ ...render, formula_contract: {} }, 'INVALID_REQUEST');
    refuses({ ...parse, source: 1 }, 'INVALID_REQUEST');
    refuses({ ...parse, format: 'latex' }, 'INVALID_REQUEST');
    refuses({ ...negotiate, op: 'transform', format: 'mathml', direction: 'syntax_to_formula', input: {}, input_contract: identity, output_contract: identity }, 'UNPINNED_CONTRACT');
    const options = withFont(Buffer.from('fresh identity-only bytes')).options;
    refuses({ ...render, options }, 'UNPINNED_CONTRACT');
});

test('limit: exact request-byte boundary and refusal before copying huge caller input', () => {
    const input = wire(parse);
    const exact = new Uint8Array(limits.request_bytes).fill(32);
    exact.set(input);
    refuses(exact, 'NOT_IMPLEMENTED');
    refuses(new Uint8Array(limits.request_bytes + 1), 'RESOURCE_LIMIT');
    refuses(new Uint8Array(limits.request_bytes * 8), 'RESOURCE_LIMIT');
    refuses({ ...negotiate, options: { limits: { request_bytes: 1 } } }, 'RESOURCE_LIMIT');
    refuses({ ...negotiate, options: { limits: { request_bytes: limits.request_bytes + 1 } } }, 'INVALID_REQUEST');
});

test('limit: UTF-8 source bytes, container depth, lower and raised caller budgets', () => {
    refuses({ ...parse, source: 'é'.repeat(limits.source_bytes / 2) }, 'NOT_IMPLEMENTED');
    refuses({ ...parse, source: 'é'.repeat(limits.source_bytes / 2 + 1) }, 'RESOURCE_LIMIT');
    refuses('['.repeat(limits.json_depth) + ']'.repeat(limits.json_depth), 'INVALID_REQUEST');
    refuses('['.repeat(limits.json_depth + 1) + ']'.repeat(limits.json_depth + 1), 'RESOURCE_LIMIT');
    refuses({ ...negotiate, options: { limits: { json_depth: 1 } } }, 'RESOURCE_LIMIT');
    for (const value of [0, -1, 1.5, limits.json_depth + 1, '8', null]) {
        refuses({ ...negotiate, options: { limits: { json_depth: value } } }, 'INVALID_REQUEST');
    }
    refuses({ ...negotiate, options: { limits: { unknown: 1 } } }, 'INVALID_REQUEST');
    refuses({ ...negotiate, options: { limits: { response_bytes: 1 } } }, 'RESOURCE_LIMIT');
    refuses({ ...parse, options: { limits: { response_bytes: 1 } } }, 'RESOURCE_LIMIT');
    refuses({ ...parse, options: { limits: { source_bytes: 1 } } }, 'RESOURCE_LIMIT');
});

test('limit: font identity, base64, count and decoded budgets without font qualification', () => {
    const bytes = Buffer.alloc(limits.font_bytes, 65);
    assert.equal(reply(withFont(bytes)).ok, true);
    refuses(withFont(Buffer.alloc(limits.font_bytes + 1, 65)), 'RESOURCE_LIMIT');
    refuses(withFont(bytes, { total_font_bytes: limits.total_font_bytes - 1 }), 'RESOURCE_LIMIT');
    refuses(withFont(Buffer.from('xy'), { font_bytes: 1 }), 'RESOURCE_LIMIT');
    const first = font(Buffer.alloc(70_000, 65));
    const second = { ...font(Buffer.alloc(70_000, 66)), id: 'other' };
    refuses({ ...negotiate, options: { fonts: [first, second], fallback_order: ['test', 'other'] } }, 'RESOURCE_LIMIT');
});

test('malformed-request: font checks and scalar options remain per-call', () => {
    const valid = withFont(Buffer.from('identity bytes'));
    const cases = [
        [{ ...valid.options.fonts[0], sha256: '0'.repeat(64) }, 'FONT_HASH_MISMATCH'],
        [{ ...valid.options.fonts[0], bytes_base64: 'YQ' }, 'INVALID_FONT'],
        [{ ...valid.options.fonts[0], bytes_base64: 'YR==' }, 'INVALID_FONT'],
        [{ ...font(Buffer.alloc(0)) }, 'INVALID_FONT'],
        [{ ...valid.options.fonts[0], face_index: 65536 }, 'INVALID_REQUEST'],
    ];
    for (const [resource, code] of cases) refuses({ ...valid, options: { fonts: [resource], fallback_order: ['test'] } }, code);
    refuses({ ...valid, options: { ...valid.options, fonts: Array(17).fill(valid.options.fonts[0]) } }, 'INVALID_REQUEST');
    refuses({ ...valid, options: { ...valid.options, fallback_order: [] } }, 'INVALID_FONT');
    refuses({ ...valid, options: { ...valid.options, fallback_order: ['test', 'test'] } }, 'INVALID_FONT');
    refuses({ ...valid, options: { ...valid.options, fonts: [valid.options.fonts[0], valid.options.fonts[0]] } }, 'INVALID_FONT');
    for (const options of [{ font_size_px: 0 }, { font_size_px: 4097 }, { color: 'red' }, { display: 'other' }, { fonts: null }, { fallback_order: null }]) refuses({ ...negotiate, options }, 'INVALID_REQUEST');
    assert.equal(reply(negotiate).ok, true);
});

test('contract: envelope validation precedes PNG refusal and identity checks', () => {
    refuses({ ...render, output: 'png', options: { fonts: [{}] } }, 'INVALID_REQUEST');
    refuses({ ...render, output: 'png', options: { fallback_order: [1] } }, 'INVALID_REQUEST');
    const resource = { ...font(Buffer.from('bytes')), bytes_base64: 'invalid' };
    refuses({ ...render, output: 'png', options: { fonts: [resource], fallback_order: ['test'] } }, 'UNSUPPORTED_OUTPUT');
    refuses(withFont(Buffer.alloc(0)), 'INVALID_FONT');
    const reduced = reply({ ...negotiate, options: { limits: { source_bytes: 1 } } });
    assert.equal(reduced.result.limits.source_bytes, 1);
});

test('reuse: response ownership, repeated refusal/recovery and bounded memory growth', () => {
    const retained = node.request_json(wire(negotiate));
    const saved = retained.slice();
    // Warm allocator bins with the largest accepted source, byte buffer and font.
    const large = withFont(Buffer.alloc(limits.font_bytes, 65));
    const boundary = new Uint8Array(limits.request_bytes).fill(32);
    boundary.set(wire(parse));
    const dense = encoder.encode('[' + '0,'.repeat((limits.request_bytes - 3) / 2) + '0]');
    const run = () => {
        refuses(new Uint8Array(limits.request_bytes + 1), 'RESOURCE_LIMIT');
        refuses({ ...parse, source: 'x'.repeat(limits.source_bytes) }, 'NOT_IMPLEMENTED');
        refuses(boundary, 'NOT_IMPLEMENTED');
        refuses(dense, 'INVALID_REQUEST');
        assert.equal(reply(large).ok, true);
        assert.equal(reply(negotiate).ok, true);
    };
    for (let index = 0; index < 8; index++) run();
    const before = raw.__wasm.memory.buffer.byteLength;
    for (let index = 0; index < 64; index++) run();
    const after = raw.__wasm.memory.buffer.byteLength;
    assert(after <= 67_108_864);
    assert.equal(after, before, 'warm repeated calls must not grow linear memory');
    assert.deepEqual(retained, saved, 'returned bytes must survive later calls');
    assert.throws(() => node.request_json('not bytes'), TypeError);
    console.log(JSON.stringify({ measurement: 'transport budgets', request_bytes: limits.request_bytes, source_bytes: limits.source_bytes, font_bytes: limits.font_bytes, json_depth: limits.json_depth, warm_memory_bytes: before, after_64_cycles_bytes: after, node: process.version }));
});

test('web recipe: generated ES module initializes and returns the same transport envelopes in Node', () => {
    for (const input of [negotiate, parse, render, { ...render, output: 'png' }, '{', Uint8Array.of(0xff)]) {
        assert.deepEqual(reply(input, web), reply(input));
    }
    assert(!readFileSync(join(root, 'web/index.js'), 'utf8').includes('node:'));
});

test('artifacts: exports, declarations, private packages and encoded memory maximum', () => {
    for (const target of ['nodejs', 'web']) {
        const bytes = readFileSync(join(root, target, 'logifos_bg.wasm'));
        const module = new WebAssembly.Module(bytes);
        const exports = WebAssembly.Module.exports(module).map(item => item.name);
        for (const name of ['request_json', 'max_request_bytes', 'memory']) assert(exports.includes(name));
        let offset = 8;
        const read = () => {
            let value = 0;
            for (let shift = 0; shift < 35; shift += 7) {
                const byte = bytes[offset++];
                assert.notEqual(byte, undefined);
                value += (byte & 127) * 2 ** shift;
                if (!(byte & 128)) return value;
            }
            assert.fail('invalid u32 encoding');
        };
        let found = false;
        while (offset < bytes.length) {
            const section = bytes[offset++];
            const length = read();
            const end = offset + length;
            if (section === 5) {
                assert.equal(read(), 1, 'one linear memory');
                assert.equal(read(), 1, '32-bit unshared memory with explicit maximum');
                const minimum = read();
                const maximum = read();
                assert(minimum <= maximum);
                assert.equal(maximum * 65536, 67_108_864);
                found = true;
            }
            offset = end;
        }
        assert(found, 'memory section required');
        const pkg = JSON.parse(readFileSync(join(root, target, 'package.json')));
        assert.equal(pkg.private, true);
        assert.equal(pkg.main, './index.js');
        assert(readFileSync(join(root, target, 'logifos.d.ts'), 'utf8').includes('request_json(bytes: Uint8Array): Uint8Array'));
        assert(readFileSync(join(root, target, 'THIRD-PARTY-LICENSES.txt'), 'utf8').includes('UNICODE LICENSE V3'));
    }
});

test('native and WASM: identical negotiated versions, limits and typed errors', () => {
    assert(process.env.WASM_NATIVE_CONSUMER, 'native consumer path required');
    for (const input of [negotiate, parse, render, { ...render, output: 'png' }, '{', Uint8Array.of(0xff), { ...negotiate, options: { limits: { source_bytes: 1 } } }, new Uint8Array(limits.request_bytes + 1)]) {
        const native = spawnSync(process.env.WASM_NATIVE_CONSUMER, { input: wire(input), maxBuffer: 1024 * 1024 });
        assert.equal(native.status, 0, native.stderr?.toString());
        assert.deepEqual(JSON.parse(native.stdout), reply(input));
    }
});

test('receipt: rejects changed glue, WASM, versions, missing files and unlisted files', () => {
    const scratch = mkdtempSync(join(tmpdir(), 'wasm-receipts-'));
    try {
        const candidate = join(scratch, 'candidate');
        cpSync(root, candidate, { recursive: true });
        const receiptScript = resolve('scripts/wasm-receipt.py');
        const check = expected => {
            const result = spawnSync('python3', [receiptScript, 'check', candidate, process.env.WASM_CONTRACT_DIR], { encoding: 'utf8' });
            assert.equal(result.status, expected, result.stderr);
        };
        check(0);
        for (const name of ['nodejs/logifos.js', 'web/logifos_bg.wasm']) {
            const path = join(candidate, name);
            const saved = readFileSync(path);
            writeFileSync(path, Buffer.concat([saved, Buffer.from('changed')]));
            check(1);
            writeFileSync(path, saved);
        }
        const path = join(candidate, 'receipt.json');
        const original = readFileSync(path);
        const receipt = JSON.parse(original);
        receipt.tools.wasm_bindgen = '0.0.0';
        writeFileSync(path, JSON.stringify(receipt)); check(1);
        writeFileSync(path, original);
        const missing = join(candidate, 'nodejs/logifos.d.ts');
        const saved = readFileSync(missing); rmSync(missing); check(1); writeFileSync(missing, saved);
        writeFileSync(join(candidate, 'extra.txt'), 'unlisted'); check(1);
    } finally {
        rmSync(scratch, { recursive: true, force: true });
    }
});
