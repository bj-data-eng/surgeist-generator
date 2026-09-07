# Getting started

Verify the shared library surface without acquiring a browser or changing a
corpus, then choose the capability your host needs.

## Prerequisites

Use a checkout of this repository, an already-installed Rust toolchain meeting
MSRV 1.97, the tracked `Cargo.lock`, and cached dependencies. The package uses
Rust 2024. Corpus mutation requires Apple-Silicon macOS; shared value/read
contracts are portable. Offline commands fail if required artifacts are absent;
they do not authorize downloading or installing replacements.

## Verify the shared contracts

1. Open a shell at the repository root.
2. Run the focused public API test:

   ```sh
   cargo test --locked --offline -p surgeist-generator --no-default-features --test public_api shared_public_surface_has_the_exact_constructible_contracts -- --exact
   ```

3. Confirm that the named test passes with one test executed. Its
   [source](../tests/public_api.rs) checks the public types, trait contracts, and
   callable entry points. This is a library smoke check, not corpus generation.

## Choose a capability

Use the default library for checked paths, source pins, provenance, and reports.
Enable `css-corpus` for synchronous CSSTree operations or `browser-corpus` for a
host that implements `BrowserCorpusAdapter`. Features and dependencies are owned
by [Cargo.toml](../Cargo.toml); exports are owned by [src/lib.rs](../src/lib.rs).

Continue with the [how-to guide](how-to.md). This checkout supplies no production
corpus, acquired source checkout, or browser installation.
