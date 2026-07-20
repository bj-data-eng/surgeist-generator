# surgeist-generator

`surgeist-generator` owns Surgeist generator tooling, shared generation
contracts, and two feature-gated corpus drivers. Root `surgeist` owns cross-crate
composition, this leaf's gitlink, root integration tests/tools, and generated API
audit artifacts. Production Surgeist crates do not normally depend on this
tooling crate.

The default feature set is empty and exposes only the shared value, rooted-path,
source-proof, provenance, and report contracts. Driver dependencies
and binaries are opt-in:

| Feature | Public module | Binary | Commands |
| --- | --- | --- | --- |
| `css-corpus` | `surgeist_generator::css` | `surgeist-css-generate` | `import-csstree`, `generate`, `check-corpus` |
| `layout-browser` | `surgeist_generator::layout` | `surgeist-layout-generate` | `import-taffy`, `check-taffy-corpus`, `generate`, `check-corpus` |

`css-corpus` adds no dependency edge. `layout-browser` alone enables the optional
Chromiumoxide, Futures, Tokio, and URL graph. Each binary has the corresponding
`required-features` setting, so default and CSS-only builds do not build the
browser graph.

## Roots and corpus ownership

Every invocation receives an existing owner root and an existing corpus root.
`CorpusLocation` requires the corpus root to be contained by the owner root and
rejects the reserved `.surgeist-generator` component in either supplied root.
The generator creates no owner-global or system-temporary mutation authority.

The corpus's `corpus.toml` is authoritative for mutable source pins, expected
counts, artifact roots, and domain settings. These values are deliberately not
hard-coded in the generator:

- A CSS schema-1 manifest owns the CSSTree revision, expected file/case counts,
  import root, expectation root, and report path. Import verifies the explicit
  existing checkout's `fixtures/ast` tree, publishes a source sidecar under the
  import root, and never executes CSSTree. Generation writes neutral
  expectations and its report under the manifest-owned expectation root.
- A layout schema-2 manifest owns the duplicated Taffy revision fields, the
  pre-exclusion source count, manifest-authored HTML, browser cache/version/launch
  pins, and report expectations. Taffy import verifies the explicit existing
  checkout's `test_fixtures` tree and atomically publishes Taffy HTML plus
  `html/.surgeist-taffy-source.json` while preserving manifest-authored HTML.
  Layout generation reads corpus-owned HTML and helper assets and publishes XML
  and reports beneath `xml`.

The source checkouts and browser cache are protected read-only inputs. Driver
coordination, transactions, and layout profile journals live beneath the corpus
root's `.surgeist-generator`; they are not production corpus artifacts. This
repository commits no production corpus, source checkout, browser, generated XML,
or generated expectation tree.

When a corpus owner changes a source pin or expected count, it supplies the newly
pinned existing checkout and reruns the applicable import. The resulting sidecar
binds the exact revision, count, and file bytes. No generator source or release
change is required.

## Driver use

Both binaries read only command-line arguments for operator configuration.
Owner, corpus, and source roots retain OS-native path bytes; browser paths and
filters are checked UTF-8 relative paths.

```sh
surgeist-css-generate --owner-root <owner> --corpus-root <corpus> import-csstree --source-root <checkout>
surgeist-css-generate --owner-root <owner> --corpus-root <corpus> generate [--filter <fixture>]
surgeist-css-generate --owner-root <owner> --corpus-root <corpus> check-corpus

surgeist-layout-generate --owner-root <owner> --corpus-root <corpus> import-taffy --source-root <checkout>
surgeist-layout-generate --owner-root <owner> --corpus-root <corpus> check-taffy-corpus --source-root <checkout>
surgeist-layout-generate --owner-root <owner> --corpus-root <corpus> generate --browser-path <owner-relative-executable> [--filter <html>]
surgeist-layout-generate --owner-root <owner> --corpus-root <corpus> check-corpus
```

The crate has no browser or source downloader, installer, repair path, or archive
extractor. Imports and source checks use an already-present Git executable and an
explicit existing checkout without network access. CSS and layout `check-corpus`
use only persisted corpus attestations and do not require a source checkout.
Layout `check-corpus` also does not open or authenticate a browser cache: it treats
browser provenance in XML/reports as historical data and verifies that the
attestation is internally consistent.

### Trusted browser boundary

Layout generation accepts one existing executable selected by an owner-relative
path below the manifest-declared browser cache root; that cache root must be
outside the corpus root. Before launch and again at close, the driver verifies
the cache/executable identity, regular executable shape, single-link status, raw
SHA-256 digest, manifest version output, and pinned launch switch set.

Those checks authenticate the selected capability; they do not sandbox it or
prove it benign. The browser and manifest-pinned switches can write or spawn
outside generator-owned filesystem namespaces. Lifecycle guarantees cover the
recorded inherited process group, not work a malicious executable deliberately
detaches, and macOS path-based spawn is not atomic execution from the held file
descriptor. Operators must trust the exact executable and pinned configuration
and must prevent concurrent cache replacement.

Version and measurement processes receive a cleared, fixed environment. `HOME`,
`TMPDIR`/`TMP`/`TEMP`, and the XDG homes point to precreated directories beneath
the private attempt profile; the profile is the working directory;
`PATH=/usr/bin:/bin`, `TZ=UTC`, `LANG=C`, and `LC_ALL=C`; proxy variables are
cleared and `NO_PROXY=*`. No inherited, manifest-supplied, or operator-supplied
environment entry reaches the browser.

Each attempt has a durable journal beneath
`.surgeist-generator/profiles/layout`. Normal completion, ordinary errors, and
panics terminalize the recorded process group before artifact planning. At the
next mutation, a provably dead journal is recovered only after protected inputs
are revalidated. Recovery never signals a process. A live, reused, permission-
inconclusive, or transition-locked recorded group returns `LeaseActive` and
preserves all evidence; the operator must terminate the orphaned trusted-browser
group and retry. Corrupt or identity-drifted cleanup evidence is also preserved
and reported instead of guessed away.

### Taffy corpus adoption

A valid sidecar-free schema-2 layout corpus is adopted by one reviewed
`import-taffy --source-root <checkout>` run. Import derives ownership from the
manifest and exact pinned checkout, adds the canonical Taffy sidecar atomically,
preserves manifest-authored HTML byte-for-byte, and rejects unknown inventory.
Thereafter `check-taffy-corpus` requires the sidecar. `check-corpus` verifies that
sidecar offline; if a pin/count update changes it, downstream XML/reports remain
stale until a clean full generation refreshes them.

The adopting layout repository owns that corpus change. It must run its schema-2
compatibility fixtures and review any normalization required by the documented
strict parser before switching scripts. This leaf never mutates a sibling corpus,
gitlink, or root API artifact.

## Platform and checks

Mutation is supported on Apple-Silicon macOS. The no-feature value/read library
remains portable across native targets and `wasm32-unknown-unknown`. The package
MSRV is Rust 1.97. All verification uses already-installed tools, lock data, and
caches; these commands do not authorize acquisition:

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

The ignored command above is inventory-only. It must retain `--list`; ordinary
verification does not execute ignored diagnostic bodies. `cargo audit --no-fetch
--stale` reports against the installed advisory database and is not a claim that
the database is current online.
