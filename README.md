# surgeist-generator

Shared corpus generation infrastructure for Surgeist tooling authors. The CSS
adapter imports CSSTree fixtures and creates neutral expectations; the browser
engine runs caller-prepared jobs and publishes caller-produced artifact bytes.

Version 0.2.0 uses Rust 2024 with MSRV 1.97. Default features are empty, keeping
shared value/read contracts native/wasm portable. Mutation is supported on
Apple-Silicon macOS. Callers supply corpora and browser adapters; this repository
contains no production corpus or layout-specific executable.

## Start

From this checkout, with Rust and locked dependencies already available, run:

```sh
cargo test --locked --offline -p surgeist-generator --no-default-features --test public_api shared_public_surface_has_the_exact_constructible_contracts -- --exact
```

Expected result: one passing test verifies the shared public contract surface.
See [getting started](docs/getting-started.md) for prerequisites and next steps.

## Documentation

- [Getting started](docs/getting-started.md): verify the library and choose a capability.
- [How-to](docs/how-to.md): operate CSS corpora and integrate a browser adapter.
- [Reference](docs/reference.md): features, interfaces, compatibility, and checks.
- [Explanation](docs/explanation.md): ownership, provenance, publication, and recovery.

## License

[MIT](LICENSE). See [third-party attribution](NOTICE.md) for dependency notices.
