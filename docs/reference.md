# Reference

## Package and public surface

[Cargo.toml](../Cargo.toml) owns package metadata, exact dependency pins, features,
and targets. The package is `surgeist-generator` 0.2.0, library
`surgeist_generator`, edition 2024, MSRV 1.97, licensed MIT.

| Feature | Public surface | Executable / dependencies |
| --- | --- | --- |
| Default (empty) | Shared contracts reexported by `src/lib.rs` | Serde, Serde JSON, SHA-2, TOML; target-specific Rustix |
| `css-corpus` | `surgeist_generator::css` | `surgeist-css-generate`; no additional optional dependency |
| `browser-corpus` | `surgeist_generator::browser` | Caller-owned host; Chromiumoxide, Futures, Tokio, URL |

Mutation support and Rustix are restricted to Apple-Silicon macOS. The default
value/read library is checked for native and `wasm32-unknown-unknown` targets.
Owned Rust forbids unsafe code at the [library front door](../src/lib.rs).

## Interfaces and declarations

| Surface | Authoritative source | Responsibility |
| --- | --- | --- |
| Shared contracts | [src/lib.rs](../src/lib.rs), [core](../src/core/mod.rs) | Checked paths, corpus locations, manifests, sources, digests, dispositions, and reports |
| Errors | [src/error.rs](../src/error.rs) | `GeneratorError`, `GeneratorErrorKind`, and `Result` |
| CSS | [src/css/mod.rs](../src/css/mod.rs) | `CssRequest`, `CssCommand`, `run`, `run_from_env` |
| Browser | [src/browser/mod.rs](../src/browser/mod.rs) | Generation/checking, adapter models, acquisition, import, reports, supervisor entry |
| Browser declarations | [src/browser/model.rs](../src/browser/model.rs) | Corpus, fixture/case, launch/settings, resources, requests, prior ownership, adapter errors |
| Browser reports | [src/browser/report.rs](../src/browser/report.rs) | Schema-4 executable/engine identity, inputs, attestations, settings, artifact hashes, outcome accounting |

CSS accepts `import-csstree`, `generate`, and `check-corpus`. All require explicit
`--owner-root` and `--corpus-root`; only import requires `--source-root`, and only
generation permits `--filter`. CSS report schemas and serialization remain
unchanged by the browser-engine migration.

Manifests and caller declarations own pins, counts, paths, browser settings,
and artifact formats. This crate does not prescribe a fixture-domain serializer.

## Storage locations

| Location | Role |
| --- | --- |
| `tmp/surgeist-sources/<source-id>/<full-revision>/` | Owner-local pinned source cache |
| `tmp/surgeist-browser/` | Owner-local cache with separate browser-version entries |
| `<corpus>/.surgeist-generator/` | Machine-local coordination, transactions, and durable browser-profile journals |

Caches are outside Cargo build output and corpus transactions. Repository `tmp/`
is ignored and survives `cargo clean`. Callers must ignore the corpus coordination
directory. Cache removal is separate maintenance, not profile/publication recovery.

## Verification

[AGENTS.md](../AGENTS.md#command-inventory) owns the command inventory: offline
checks, tests, and warning-denied Clippy across no features, each optional feature,
and all features; the no-feature wasm library check; metadata, formatting,
license checks, and the installed advisory database audit.

Tracked [integration tests](../tests) cover the public API, shared contracts,
package/feature policy, CSS CLI, and browser manifests/imports. Module-local tests
cover transactions, source protection, browser lifecycle, input revalidation,
retry, adapter errors, and mixed generated/unsupported outcomes.

Ignored diagnostics are inventory-only unless explicitly authorized: retain
`--list`. `cargo audit --no-fetch --stale` uses the installed advisory database
and does not establish that it is current online. All checks use already-present
tooling, lock data, and caches; offline/no-fetch flags do not authorize acquisition.

For rationale and trust limits, see [explanation](explanation.md).
