# Phase-zero CI gates

This ledger describes the bootstrap workspace. Passing its tests does not
qualify a renderer, a runtime package, or a release.

The workspace declares Rust 1.95 as its minimum tested compiler. This is the
compiler available for local validation, not a claim that older Rust cannot
compile the code. Every crate inherits that value. The reusable setup workflow
checks this agreement and exports the manifest value to the MSRV job; CI also
tracks stable on Linux, macOS and Windows. Each job installs its own toolchain
and restores its own cache because reusable workflow jobs have separate runners.

| Gate | Command or coverage |
| --- | --- |
| Formatting | `cargo fmt --all --check` |
| Build | `cargo build --workspace --all-targets --locked -j 2` |
| Tests | `cargo test --workspace --locked -j 2`, including doctests |
| Clippy | `cargo clippy --workspace --all-targets --locked -j 2 -- -D warnings` |
| Documentation | `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked -j 2` |
| Features | Stable Linux repeats build, test and clippy with `--no-default-features` and `--all-features` |
| MSRV | Linux repeats the default build, test and clippy on the manifest's Rust version |
| Unused dependencies | `cargo machete --with-metadata` |
| Dependency policy | `cargo deny --workspace --locked check` |
| Spelling | `typos --hidden .github crates Cargo.toml README.md docs/ci-phase-zero.md` |

There are currently no workspace feature declarations. The two additional
profiles keep CI prepared for future features without inventing feature names.
Five existing path dependencies reserve the bootstrap pipeline boundaries.
Their two manifests list them explicitly as cargo-machete exemptions; remove
each exemption when that boundary is used. These exemptions do not exclude any
crate from the license or advisory checks.
Dependency checks cover the complete workspace graph, including optional,
transitive and unpublished packages, without a platform filter. The license
allowlist accepts the MIT terms offered by the current resolved graph; new
licenses require a deliberate policy change. Known vulnerabilities, yanked
versions, duplicate versions, registry wildcard requirements and unapproved
registry or Git sources fail the gate. Unversioned local paths are allowed for
the unpublished workspace crates. No advisory exceptions are currently recorded.

CI runs for every pull request and every push to main, without path filters, so
future corpus and policy changes also trigger the gates. Tool versions for
dependency and spelling checks are pinned in CI. Semver comparison is deferred
until a supported, published API baseline exists; all six crates are currently
unpublished. There are no release or publication jobs.

Local command results and CI execution are separate evidence. GitHub execution
and macOS/Windows results must be checked after an authorized push; adding the
workflows alone does not establish those results.
