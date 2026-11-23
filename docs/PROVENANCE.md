# MIT provenance procedure

Logifos implementation, tests, tables, tooling and documentation must be written
fresh under the existing MIT license. Do not import renderer source, source
fragments, data, fixtures, generated outputs, fonts or geometry baselines.
Architectural inspiration is not permission to copy. Read public specifications
directly and cite their original source, edition and section. Parsanol supplies
patterns only; its files are not authorized inputs.

The existing `LICENSE.txt` is Ronald's MIT license, copyright 2026 Ribose Inc.
Its SHA-256 is
`2d0f8a412f45f64d0e2bba4f9592907371613f219fe01f2e7a88c0381a672d07`.
Keep that license and the existing package names. Ordinary declared dependencies
are distinct from copied source; record their exact versions, upstream sources
and licenses. Dependencies may have their own licenses; the manifest's `license`
field describes the changed Logifos file.

## Per-PR check

1. Inspect the actual changes between explicit base and head commits. Update
   `provenance.json` for every added or modified tracked file, including cases,
   tables, generated artifacts and infrastructure. Review existing entries when
   changing a file again. An entry does not grant continuing permission to import.
2. Have the author provide the explicit no-copy declaration and accurate origins.
   Do not assert provenance for historical unchanged files without evidence.
3. Run the same pinned Logifos checker against the target repository:

   ```sh
   python3 tools/provenance.py --repo PATH --base BASE --head HEAD --manifest provenance.json
   ```

   Both revisions must resolve to commits. They are compared directly, rather
   than choosing a merge base. `provenance.json` and every receipt input are read
   from the target HEAD's Git objects; uncommitted edits cannot repair a failure.
   For sibling repositories, invoke an absolute path to the checker from a
   reviewed Logifos commit, and record that checker commit alongside the explicit
   target base/head in the PR evidence. No installation or repository code
   execution is involved. Python 3.9+ and Git are sufficient; no Rust build runs.
4. Inspect the focused diff, origin trails and every signal acknowledgement.
   Keep the author declaration and command's JSON counts in the PR evidence.
   A failed check exits 1; a successful check exits 0. Commit the manifest before
   checking HEAD. Later CI wiring belongs to card 11; this card runs no workflows.

## Manifest format (schema version 1)

`declaration` contains `author`, a nonempty `statement` and
`no_renderer_source_data_output_copied: true`. The statement explicitly declares
that no renderer source, data or output was copied.

`files` maps exact normalized repository-relative POSIX paths to entries. Every
entry requires `author`, `description`, `origin`, `kind`, `license: "MIT"` and
`specification_refs`. Use an empty reference list for fresh infrastructure with
no specification inputs; do not invent a specification citation.

Allowed origins:

- `fresh-logifos`: independently authored Logifos work.
- `public-specification`: direct original specification sources. Requires at
  least one reference with an HTTPS `url`, `edition` and `section`; the author
  must verify the original source and its compatibility with MIT use.
- `declared-dependency`: dependency manifests only (`kind: "dependency"`).
  Requires a nonempty `dependencies` list with `name`, exact `version`, `source`
  and upstream `license` for each dependency. It does not authorize source imports.
- `generated-logifos`: output from fresh Logifos code (`kind: "generated"`).

All other origins, including renderer-derived and unknown origins, fail.
Kinds are `implementation`, `infrastructure`, `documentation`, `fixture`,
`table`, `generated` and `dependency`. A fixture or table also requires a
`source_trail` explaining how the author derived it from the contract and/or
listed specification. Hand-authored synthetic tests must say so. Formula trees
and geometry expectations must be independently derived; do not use another
implementation's bytes as their specification. Generate SVG baselines only with
fresh Logifos code and review them against independently derived assertions.

A generated entry's `receipt` has:

- `command`: the actual reproducible generation command, recorded but never run
  by this checker.
- `generator`: an object with its tracked `path` and lowercase `sha256`.
  That path needs its own fresh Logifos implementation entry, even if unchanged.
- `inputs`: a nonempty list of tracked `path`/`sha256` objects. Their provenance
  entries are required and checked even when unchanged from the base. Generated
  inputs are checked recursively; cycles and chains deeper than 64 are rejected.
- `output_sha256`: the hash of the generated file's exact HEAD bytes.

All receipt hashes are compared to Git blobs from HEAD, including binary output.
The output cannot be its own generator or input. External sources must first be
pinned, eligible tracked inputs with their original non-renderer provenance.
A hash identifies bytes; it does not prove a generator ran or its output is
correct. Inspect the command, generator and independent assertions separately.

## Review signals and limits

Added text in diffs is scanned for foreign renderer names, machine-local/source
URLs and attribution headers. A signal fails unless that file's `review_signals`
maps the signal name (`foreign-reference`, `source-path` or `attribution`) to a
nonempty explanation. This checker's own detection rules and synthetic negative
tests explain those signals in their entries. An explanation is review evidence,
not an exception allowing prohibited imports. Review it before accepting the PR.

All added/modified regular tracked files are checked, including binary files.
Rename detection is disabled, so renames and copies require destination entries.
Deleted files need no new byte provenance. Symlinks, submodules, unsafe paths and
non-UTF-8 changed paths are rejected. Binary content still needs an entry and,
when generated, a receipt; text signal scanning cannot inspect binary content.
Untracked files are outside the committed revision range. The checker cannot
infer whether an author mislabeled generated content as fresh, recognize every
foreign module, establish independent authorship, or prove specification accuracy.
The author's explicit declaration and focused human diff inspection are required.

Run the fresh standard-library synthetic self-tests:

```sh
python3 -m unittest discover -s tools/tests -p 'test_provenance.py'
```

Tests create temporary Git repositories, author their own bytes and exercise
positive and negative cases. They never read external renderer trees or fixtures.
