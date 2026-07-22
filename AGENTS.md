# surgeist-generator Repository Guide

Use the installed `surgeist-agent` plugin for every task in this repository.
Select the task-appropriate focused skill.

## Authority Split

This file is the leaf repository's committed discovery entry point. It owns the
mapping from mutable leaf facts to authoritative sources, the intended crate and
architecture boundary, and the configured local command inventory. The sources
named below own their current values.

The installed `surgeist-agent` plugin is the sole Surgeist workflow authority.
Its selected skill owns scope control, planning, debugging and TDD,
worker/reviewer gates, external-software permission,
the absolute unsafe prohibition, Git landing and publication, and cross-repository
handoffs. This file does not redefine those workflows or grant authority to
mutate, install, commit, or publish.

Resolve an apparent conflict by domain: use this file and the sources below for
mutable repository facts; use the selected plugin skill for workflow.
Higher-priority user and system instructions still apply. Do not import another
workflow.

## Repository Identity And Ownership

`surgeist-generator` is an independent leaf repository. It owns its manifest,
domain implementation, public front door, focused tests and docs, commits, and
published `main` candidate.

The root `surgeist` repository owns the facade and public composition surface,
cross-crate adapters, root integration tests and tools, this leaf's gitlink, and
the API generator and generated audit artifacts. A parent workspace, Codex
project, task, branch, or worktree does not change repository ownership.

## Discover The Current Structure

Read these sources instead of relying on cached descriptions.

| Fact | Authoritative source |
| --- | --- |
| Package identity, edition, MSRV, dependencies, features, and targets | `Cargo.toml` |
| Public front door | `src/lib.rs` and its reexports |
| Current behavior and crate boundary | `README.md` and `src/` |
| Focused verification | tracked `#[cfg(test)]` modules in `src/` and integration tests in `tests/` |
| Additional configured commands | Cargo targets and features in `Cargo.toml`, `README.md`, and tracked task-runner or CI configuration when present |
| Integration MSRV, authoritative URL, and compatible pin when root integration is in scope | root `Cargo.toml`, root `.gitmodules`, and the root committed gitlink |

When these sources disagree, report the exact paths and revisions. Do not guess,
silently update another document, or widen the task to reconcile them.

## Crate Boundary

`surgeist-generator` owns the shared generation core and the two completed,
feature-gated CSS and layout drivers. `css-corpus` exposes the synchronous
CSSTree/neutral-expectation API and `surgeist-css-generate`; `layout-browser`
exposes Taffy maintenance, trusted-browser XML/report generation, and
`surgeist-layout-generate`. The default feature set exposes only shared value and
read contracts. `Cargo.toml`, `src/lib.rs`, and the public module rustdoc own the
exact current target and API facts.

Callers supply explicit owner/corpus roots and, for imports/source checks, an
existing source checkout. Corpus manifests own mutable source pins, counts,
artifact roots, and browser settings. This repository owns the parser,
verification, transaction, generation, and focused synthetic/process evidence;
it does not own or commit production corpora, source checkouts, browsers, XML,
expectation trees, or sibling adoption changes.

Mutation support is Apple-Silicon macOS. The default value/read library remains
native/wasm portable. Source/browser acquisition is not a crate capability:
imports verify existing checkouts and generation authenticates one existing
trusted browser executable. README and rustdoc define the operator-facing trust,
offline-attestation, profile-recovery, and Taffy-adoption boundaries.

The root `surgeist` repository continues to own cross-crate integration, this
leaf's gitlink, and the root API generator and generated API audit artifacts.
Surgeist-to-Surgeist lowering and adapters belong to root, and sibling internals
are not this repository's surface.

## API Artifacts

Source in this repository is authoritative. The root `surgeist` repository owns
the only API generator and all generated API audit artifacts; this leaf carries
no copies.

## Command Inventory

These commands describe local verification capability. The selected plugin skill
determines the exact gate, order, feature matrix, and whether already-present
tooling can run without unauthorized acquisition.

```sh
cargo generate-lockfile --offline
RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --no-default-features
RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --features layout-browser
RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --features css-corpus
RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --all-features
cargo test --locked --offline -p surgeist-generator --no-default-features
cargo test --locked --offline -p surgeist-generator --features layout-browser
cargo test --locked --offline -p surgeist-generator --features css-corpus
cargo test --locked --offline -p surgeist-generator --all-features
cargo test --locked --offline -p surgeist-generator --all-features -- --ignored --list
cargo clippy --locked --offline -p surgeist-generator --no-default-features --all-targets -- -F unsafe-code -D warnings
cargo clippy --locked --offline -p surgeist-generator --features layout-browser --all-targets -- -F unsafe-code -D warnings
cargo clippy --locked --offline -p surgeist-generator --features css-corpus --all-targets -- -F unsafe-code -D warnings
cargo clippy --locked --offline -p surgeist-generator --all-features --all-targets -- -F unsafe-code -D warnings
RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --target wasm32-unknown-unknown --no-default-features --lib
cargo metadata --locked --offline --no-deps --format-version 1
cargo fmt --check
cargo deny --all-features --locked --offline check licenses
cargo audit --no-fetch --stale
```

The ignored invocation is list-only inventory evidence; never remove `--list`
without the separate explicit authorization required by the selected plugin
skill and the active cycle contract. Offline/no-fetch flags prove use of
already-present artifacts; they do not authorize installing or downloading
missing software.

Discovery is complete when the owning repository, public front door, dependency
and feature facts, verification sources, API-artifact owner, and applicable
command inventory are identified from the sources above.
