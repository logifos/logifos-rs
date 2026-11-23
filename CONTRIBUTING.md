# Contributing to logifos-rs

logifos-rs is an unpublished Rust workspace for a math formula renderer. The
current code provides JSON request handling and part of the formula model;
parsing, layout and SVG rendering are not implemented end to end. Read the
[architecture plan](docs/PLAN.adoc) for the intended pipeline and the
[CI guide](docs/ci-phase-zero.md) for the current checks.

Use Rust 1.95 or newer. The workspace declares 1.95 as its minimum Rust version;
the toolchain file selects stable. Rustfmt and Clippy are needed for the checks.
CI uses typos-cli 1.52.2, cargo-deny 0.22.0 and cargo-machete 0.10.1. Install any
missing tools in a local environment and put their executables on PATH.

Run `cargo fmt --all --check` and `typos --config typos.toml --hidden .` from the
repository root. Spelling checks include hidden configuration files and the
current working tree, with Git metadata, build output and the
local virtual environment excluded. Keep word exceptions in typos.toml limited
to project names or terms that would otherwise be incorrectly flagged.

For a code change, run the affected tests with `cargo test --locked -p CRATE`,
replacing CRATE with the affected workspace package. Build with
`cargo build --workspace --all-targets --locked -j 2` and lint with
`cargo clippy --workspace --all-targets --locked -j 2 -- -D warnings` as needed
for the change. The CI guide lists the remaining documentation, feature and
dependency checks and explains the bootstrap limits. A passing bootstrap check
does not establish that the renderer or a runtime package is complete.

The repository provides an optional Git hook. Enable it for this repository with
`git config --local core.hooksPath .githooks`. Run `.githooks/pre-commit` to check
the tree without committing. It checks formatting and spelling without changing
files, stops on a failed command and needs Cargo, Rustfmt and typos on PATH. It
checks unstaged edits too, so a failure may come from work outside the commit.

Alternatively, if pre-commit is available in a local environment, run
`pre-commit install` and `pre-commit run --all-files`. The configuration invokes
the same repository hook and uses the tools on PATH. Choose one installation
method. To switch from core.hooksPath to pre-commit, first remove the local
setting with `git config --local --unset core.hooksPath`. Neither method runs the
complete CI pipeline.

Keep pull requests focused on the behavior being changed. Describe the problem,
the resulting behavior and the checks you ran, including checks that could not
run. Record user-visible changes under Unreleased in [CHANGELOG.md](CHANGELOG.md).
There is no supported published API baseline to migrate from at present; the
current limitations are described above. For a suspected vulnerability, read
[SECURITY.md](SECURITY.md) before sharing details.
