# surgeist-generator Repository Guide

Use `$pisct:coordination` for standalone delivery and the smallest focused
`$pisct:<skill>` for focused work. Preserve an explicit workflow selected by the
user or higher-priority instructions. Use `$pisct:plane-coordination` only when
plane coordination is explicitly selected; existing planning files do not select it.
This guide supplies repository facts, not mutation, installation, commit,
publication, or cross-repository authority.

## Authority Split

This file is the repository's committed discovery entry point. It owns the
mapping from mutable repository facts to authoritative sources, the product
boundary, configured command inventory, and the PISCT workflow selection.
PISCT skills supply reusable coordination and engineering guidance; they do not
own this crate's mutable facts. Higher-priority user and system instructions
still apply.

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
| Adoption and documentation navigation | `README.md` |
| Current behavior and crate boundary | `src/`, `docs/reference.md`, and `docs/explanation.md` |
| Setup and operating procedures | `docs/getting-started.md` and `docs/how-to.md` |
| Focused verification | tracked `#[cfg(test)]` modules in `src/` and integration tests in `tests/` |
| Verification commands and dependency policy | This command inventory, Cargo targets/features in `Cargo.toml`, and `deny.toml` |
| Project license | `LICENSE` and `Cargo.toml` |
| Third-party attribution and coverage | `NOTICE.md`, `licenses/`, and exact upstream release material; `Cargo.toml` and `Cargo.lock` own dependency identity |
| Integration MSRV, authoritative URL, and compatible pin when root integration is in scope | root `Cargo.toml`, root `.gitmodules`, and the root committed gitlink |

When these sources disagree, report the exact paths and revisions. Do not guess,
silently update another document, or widen the task to reconcile them.

## Product Boundary

`surgeist-generator` owns shared generation infrastructure, the feature-gated
CSS driver, and the caller-driven browser corpus engine. `css-corpus` exposes
the synchronous CSSTree/neutral-expectation API and `surgeist-css-generate`.
`browser-corpus` exposes explicit acquisition, source import/attestation,
browser lifecycle, generic provenance/accounting, and atomic publication.
Callers own their host executable, fixture semantics, measurement protocol, and
artifact serialization. This crate has no layout-specific API or binary.

The default value/read library remains native/wasm portable; mutation support is
Apple-Silicon macOS. `CorpusLocation` preserves contained roots; browser-specific
locations separately bind an exact browser owner and corpus. Acquisition uses
ignored owner-local `tmp/surgeist-sources` and `tmp/surgeist-browser` caches,
outside Cargo build output and corpus transactions. Existing-only operations
and corpus checks never acquire software. Manifests/caller declarations own
source pins, counts, paths, and browser settings. The documentation guides and
rustdoc define precise trust, provenance, cache, recovery, and migration contracts.

This repository owns infrastructure and focused synthetic/process evidence; it
does not commit production corpora, acquired software, generated expectations,
or sibling adoption changes.

Surgeist-to-Surgeist lowering and adapters belong to root, and sibling internals
are not this repository's surface. For work involving another repository, resolve
its ownership from current committed policy and source. Inspection does not grant
write authority there.

## Generated Artifacts

Source in this repository is authoritative. The root `surgeist` repository owns
the only API generator and all generated API audit artifacts; this leaf carries
no copies. Refresh and check them through their owning repository; never hand-edit
generated artifacts.

## Command Inventory

These commands describe local verification capability. The assigned scope and
PISCT guidance select the applicable checks. Run noninteractive checks through
`$pisct:process` with caller-authorized, already-present tooling. Mutation tests
require Apple-Silicon macOS; the no-feature wasm check covers the portable library.

```sh
cargo generate-lockfile --offline
RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --no-default-features
RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --features browser-corpus
RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --features css-corpus
RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --all-features
cargo test --locked --offline -p surgeist-generator --no-default-features
cargo test --locked --offline -p surgeist-generator --features browser-corpus
cargo test --locked --offline -p surgeist-generator --features css-corpus
cargo test --locked --offline -p surgeist-generator --all-features
cargo test --locked --offline -p surgeist-generator --all-features -- --ignored --list
cargo clippy --locked --offline -p surgeist-generator --no-default-features --all-targets -- -F unsafe-code -D warnings
cargo clippy --locked --offline -p surgeist-generator --features browser-corpus --all-targets -- -F unsafe-code -D warnings
cargo clippy --locked --offline -p surgeist-generator --features css-corpus --all-targets -- -F unsafe-code -D warnings
cargo clippy --locked --offline -p surgeist-generator --all-features --all-targets -- -F unsafe-code -D warnings
RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --target wasm32-unknown-unknown --no-default-features --lib
cargo metadata --locked --offline --no-deps --format-version 1
cargo fmt --check
cargo deny --all-features --locked --offline check licenses
cargo audit --no-fetch --stale
```

The ignored invocation is list-only inventory evidence; never remove `--list`
without separate explicit authorization to execute ignored diagnostics.
Offline/no-fetch flags prove use of already-present artifacts; they do not authorize installing or downloading
missing software.

Discovery is complete when the owning repository, public front door, dependency
and feature facts, verification sources, API-artifact owner, and applicable
command inventory are identified from the sources above.
