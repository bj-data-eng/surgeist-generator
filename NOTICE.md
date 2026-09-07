# surgeist-generator

## Dependencies

This notice covers the nine direct dependencies declared by
[Cargo.toml](Cargo.toml) and pinned in [Cargo.lock](Cargo.lock) for version 0.2.0.
It includes all optional features and the Apple-Silicon macOS dependency.
The default library and `css-corpus` use Serde, Serde JSON, SHA-2, and TOML;
Rustix is target-specific. Chromiumoxide, Futures, Tokio, and URL are enabled by
`browser-corpus`.

This is a direct-dependency attribution set for the repository source
distribution, not a complete transitive binary-release inventory. Dependency
source code is not vendored here. Transitive runtime dependencies, build tools,
other-target packages, separately acquired browsers/source corpora, and
caller-produced artifacts are outside this inventory. Their inclusion and
attribution must be established for the particular artifact being distributed.
The project's own [MIT license](LICENSE) remains separate.

License files below are verbatim copies from the exact cached upstream crate
releases. License alternatives are preserved; this notice does not select one.

### chromiumoxide

This product depends on chromiumoxide 0.9.1, distributed by Matthias Seitz:

* License: [MIT](licenses/chromiumoxide/LICENSE-MIT) OR [Apache-2.0](licenses/chromiumoxide/LICENSE-APACHE)
* Homepage: [chromiumoxide](https://github.com/mattsse/chromiumoxide)

### futures

This product depends on futures 0.3.31:

* License: [MIT](licenses/futures-rs/LICENSE-MIT) OR [Apache-2.0](licenses/futures-rs/LICENSE-APACHE)
* Homepage: [futures](https://rust-lang.github.io/futures-rs)

### rustix

This product depends on rustix 1.1.4, distributed by Dan Gohman, Jakub Konka:

* License: [Apache-2.0 WITH LLVM-exception](licenses/rustix/LICENSE-Apache-2.0_WITH_LLVM-exception) OR [Apache-2.0](licenses/rustix/LICENSE-APACHE) OR [MIT](licenses/rustix/LICENSE-MIT)
* Homepage: [rustix](https://github.com/bytecodealliance/rustix)
* Additional material: [COPYRIGHT](licenses/rustix/COPYRIGHT)

### serde

This product depends on serde 1.0.228, distributed by Erick Tryzelaar, David Tolnay:

* License: [MIT](licenses/serde/LICENSE-MIT) OR [Apache-2.0](licenses/serde/LICENSE-APACHE)
* Homepage: [serde](https://serde.rs)

### serde_json

This product depends on serde_json 1.0.145, distributed by Erick Tryzelaar, David Tolnay:

* License: [MIT](licenses/serde-json/LICENSE-MIT) OR [Apache-2.0](licenses/serde-json/LICENSE-APACHE)
* Homepage: [serde_json](https://github.com/serde-rs/json)

### sha2

This product depends on sha2 0.10.9, distributed by RustCrypto Developers:

* License: [MIT](licenses/rustcrypto-hashes/LICENSE-MIT) OR [Apache-2.0](licenses/rustcrypto-hashes/LICENSE-APACHE)
* Homepage: [sha2](https://github.com/RustCrypto/hashes)

### tokio

This product depends on tokio 1.48.0, distributed by Tokio Contributors:

* License: [MIT](licenses/tokio/LICENSE)
* Homepage: [tokio](https://tokio.rs)

### toml

This product depends on toml 0.9.8:

* License: [MIT](licenses/toml/LICENSE-MIT) OR [Apache-2.0](licenses/toml/LICENSE-APACHE)
* Homepage: [toml](https://github.com/toml-rs/toml)

### url

This product depends on url 2.5.7, distributed by The rust-url developers:

* License: [MIT](licenses/rust-url/LICENSE-MIT) OR [Apache-2.0](licenses/rust-url/LICENSE-APACHE)
* Homepage: [url](https://github.com/servo/rust-url)

## Maintaining and distributing this set

Keep `NOTICE.md` and `licenses/` together with these relative paths. On a
dependency version, feature, target, or distribution change, compare the affected
upstream license and attribution material before reusing it. Preserve upstream
copyrights, license alternatives, exception text, and applicable notices.
For a binary release, establish the actual transitive and bundled-material
inventory before treating its attribution set as complete. Builds must use the
committed notice material without acquiring license files.
