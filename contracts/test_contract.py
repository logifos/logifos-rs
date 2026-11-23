#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Fresh synthetic contract checks. No engine output is generated or qualified."""
import base64
from copy import deepcopy
import hashlib
import json
from pathlib import Path
import sys
import check

ROOT = Path(__file__).resolve().parent


def raw(value):
    return json.dumps(value, ensure_ascii=True).encode('utf-8')


def run():
    cases = []
    parse = json.loads((ROOT / 'cases/parse.json').read_text())
    negotiate = json.loads((ROOT / 'cases/negotiate.json').read_text())
    def request(name, value, code=None):
        cases.append((name, check.validate_request, value if isinstance(value, bytes) else raw(value), code))
    request('parse-unicode', parse)
    request('negotiate-envelope', negotiate)
    request('unknown-schema', {**parse, 'schema_version': 2}, 'UNSUPPORTED_SCHEMA')
    request('bool-schema', {**parse, 'schema_version': True}, 'UNSUPPORTED_SCHEMA')
    request('unknown-abi', {**parse, 'abi_version': 0}, 'UNSUPPORTED_ABI')
    request('unknown-op', {**parse, 'op': 'other'}, 'INVALID_REQUEST')
    request('unknown-field', {**parse, 'extra': True}, 'INVALID_REQUEST')
    request('unknown-option', {**parse, 'options': {'dpi': 96}}, 'UNKNOWN_OPTION')
    request('missing-source', {k:v for k,v in parse.items() if k != 'source'}, 'INVALID_REQUEST')
    request('invalid-utf8', b'\xff', 'INVALID_UTF8')
    request('surrogate', b'{"schema_version":1,"abi_version":1,"op":"parse","format":"mathml","source":"\\ud800"}', 'INVALID_UTF8')
    request('duplicate-key', b'{"op":"parse","op":"render"}', 'INVALID_JSON')
    request('nonfinite', b'{"x":NaN}', 'INVALID_JSON')
    request('numeric-overflow', b'{"x":1e999}', 'INVALID_JSON')
    request('trailing-data', raw(parse) + b'x', 'INVALID_JSON')
    request('json-depth', b'['*65+b'0'+b']'*65, 'RESOURCE_LIMIT')
    request('size-limit', b' ' * (check.LIMITS['request_bytes']+1), 'RESOURCE_LIMIT')
    request('source-utf8-bytes', {**parse,'source':'α','options':{'limits':{'source_bytes':1}}}, 'RESOURCE_LIMIT')
    request('reduced-request-bytes', {**parse,'options':{'limits':{'request_bytes':1}}}, 'RESOURCE_LIMIT')
    request('reduced-json-depth', {**parse,'options':{'limits':{'json_depth':1}}}, 'RESOURCE_LIMIT')
    request('raised-hard-ceiling', {**parse,'options':{'limits':{'request_bytes':8388609}}}, 'INVALID_REQUEST')
    request('bool-bound', {**parse,'options':{'limits':{'source_bytes':True}}}, 'INVALID_REQUEST')
    request('font-size-huge-integer', {**parse,'options':{'font_size_px':10**400}}, 'INVALID_REQUEST')
    request('zero-font-size', {**parse,'options':{'font_size_px':0}}, 'INVALID_REQUEST')
    font={'id':'fresh','sha256':hashlib.sha256(b'authored-invalid-font').hexdigest(),'bytes_base64':base64.b64encode(b'authored-invalid-font').decode(),'face_index':0}
    def withfont(**changes):
        return {**parse,'options':{'fonts':[{**font,**changes}],'fallback_order':['fresh']}}
    request('font-base64', withfont(bytes_base64='!!!!'), 'INVALID_FONT')
    request('font-empty', withfont(bytes_base64='',sha256=hashlib.sha256(b'').hexdigest()), 'INVALID_FONT')
    request('font-hash', withfont(sha256='0'*64), 'FONT_HASH_MISMATCH')
    request('font-byte-limit', {**withfont(),'options':{**withfont()['options'],'limits':{'font_bytes':1}}}, 'RESOURCE_LIMIT')
    request('font-total-limit', {**withfont(),'options':{**withfont()['options'],'limits':{'total_font_bytes':1}}}, 'RESOURCE_LIMIT')
    request('font-missing-order', {**withfont(),'options':{'fonts':[font]}}, 'INVALID_FONT')
    request('font-unknown-id', {**withfont(),'options':{'fonts':[font],'fallback_order':['other']}}, 'INVALID_FONT')
    request('font-duplicate-id', {**withfont(),'options':{'fonts':[font,font],'fallback_order':['fresh']}}, 'INVALID_FONT')
    identity={'origin':'synthetic-invalid-pin','revision':'0'*40,'schema_path':'not-a-real-schema.json','sha256':'0'*64}
    render={'schema_version':1,'abi_version':1,'op':'render','formula':{},'formula_contract':identity,'output':'png'}
    request('core-png-refusal', render, 'UNSUPPORTED_OUTPUT')
    request('render-missing-font', {**render,'output':'svg'}, 'MISSING_FONT')
    transform={'schema_version':1,'abi_version':1,'op':'transform','format':'mathml','direction':'syntax_to_formula','input':{},'input_contract':identity,'output_contract':identity}
    request('syntax-pin-unresolved', transform, 'UNPINNED_CONTRACT')
    request('formula-pin-unresolved', {**transform,'direction':'formula_to_syntax'}, 'UNPINNED_CONTRACT')
    response=(ROOT/'cases/refusal.json').read_bytes()
    cases.append(('structured-refusal',check.validate_response,response,None))
    value=json.loads(response);value['result']={}
    cases.append(('ambiguous-response',check.validate_response,raw(value),'INVALID_REQUEST'))
    # Geometry is a stand-alone authored value, never a claimed renderer output.
    response_schema=json.loads((ROOT/'response.schema.json').read_text())
    metric=response_schema['$defs']['metrics']
    geometry={'unit':'css_px','width':1,'height':2,'depth':3,'baseline':2,'ink_bounds':{'x':-1,'y':0,'width':2,'height':5}}
    def metrics(data):
        return check.matches(check.decode(data,4096),metric,response_schema)
    cases.append(('metrics-huge-integer',metrics,raw({**geometry,'width':10**400}),'INVALID_REQUEST'))
    cases.append(('metrics-css-pixels',metrics,raw(geometry),None))
    cases.append(('metrics-wrong-units',metrics,raw({**geometry,'unit':'em'}),'INVALID_REQUEST'))
    cases.append(('metrics-negative-ink-width',metrics,raw({**geometry,'ink_bounds':{**geometry['ink_bounds'],'width':-1}}),'INVALID_REQUEST'))
    cases.append(('metrics-negative-height',metrics,raw({**geometry,'height':-1}),'INVALID_REQUEST'))
    passed=0
    for name,fn,data,code in cases:
        try:
            fn(data)
            actual=None
        except check.Invalid as exc:
            actual=exc.code
        if actual != code:
            print(json.dumps({'case':name,'expected':code,'actual':actual}))
        else:
            passed += 1
    print(json.dumps({'checks':len(cases),'passed':passed,'failed':len(cases)-passed,'scope':'authored envelope/refusal and standalone metric constraints; no engine execution'}))
    return int(passed != len(cases))


if __name__=='__main__':
    sys.exit(run())
