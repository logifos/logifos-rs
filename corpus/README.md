# Shared conformance foundation (format 1)

This is fresh MIT test input and source metadata, not a rendering baseline.
The validator uses only the Python standard library. It validates all entries
before selecting exact names, so a named case cannot hide a corrupt corpus.

```sh
python3 tools/conformance.py
python3 tools/conformance.py --case bootstrap-unknown-op
python3 tools/conformance.py --stage P1
python3 tools/conformance.py --self-test
python3 tools/conformance.py --check-p1
python3 tools/conformance.py --replay
```

The last two commands deliberately exit 2. Validation exits 0; malformed data,
unknown names and empty selections exit 1. `engine_executed: false` is always
reported. No command in this card executes, compiles or qualifies an engine.

`manifest.schema.json` is the format authority. The checker implements exactly
the subset of JSON Schema used in that file, not a general JSON Schema library.
Every case has a stable stage/rule id, exact input-file SHA-256, nullable Formula
revision and font SHA-256, origin declaration, and expected result kind. A font
hash is null because these cases use no font. A future font-bearing case must
supply a separately pinned resource receipt and an adapter before replay.
Paths stay inside the corpus; symlinks, missing files, duplicate names, unknown
fields/versions, empty input and hash mismatches fail. JSON is strict UTF-8.

P0 cases target the current bootstrap facade, with its existing `op`/`format`
envelope. Their `{ "ok": false }` assertions express the refusal contract;
they are not recorded engine observations. They make no claim about the future
versioned facade from card 09. P1 cases are newly authored examples based on
MathML 3.0 Second Edition (10 April 2014), section 2.1.2 (namespaces) and 2.1.6
(shared presentation attributes). Their expected kind is `syntax`, status
`pending`, and output is null. No successful output, Formula tree, SVG or
geometry is invented. New v1 wire adapters and real observations belong to the
later implementation cards.

Formula is a separate upstream contract. Its revision, schema path and hash
remain null until card 12 pins the approved authority. This format neither
defines Formula fields nor changes shared model code.

`source-inputs.json` pins the direct original testsuite repository at
`27e5493203290b7910aaccf4efc2230540660cea` and inventories generation 3's 27
payload paths. It records 260 seed case ids with their reported original spec
file/line origins. These are metadata, not imported fixture bytes. The seed
list is not proof of complete stage coverage: the stage also contains rejection,
model-roundtrip and revised cases. All original case origins remain unresolved
until direct original input bytes, exact revisions and licenses are screened.
No upstream source is enabled for required replay. The recorded coverage gaps
block P1 closure even when this foundation validator passes.

Inventory generation consumes only the tracked, fresh metadata transcript:

```sh
python3 corpus/inventory.py > corpus/source-inputs.json
```

`source-metadata.json` independently enumerates the original seed case ids,
reported source paths/lines, and generation directory paths. No fixture inputs
or outputs are included. Its upstream metadata URLs/revisions/hashes record the
source trail; they are not input-eligibility clearance. The generator receipt
pins this tracked transcript and generator. Original case URLs and revisions
remain null until direct original-source identity, bytes and license are checked.

`provenance.json` uses the card-05 manifest format and covers every card file.
The generated inventory has a reproducible command and tracked generator/input/
output hashes. Run the separately pinned card-05 checker against committed
base/head; this focused validator does not replace that check.
