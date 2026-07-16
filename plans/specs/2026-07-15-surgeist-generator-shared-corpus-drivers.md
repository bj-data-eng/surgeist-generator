# Surgeist Generator Shared Corpus Drivers Specification

## SG-01 Outcome and acceptance

`surgeist-generator` shall become the owning leaf for reusable corpus-generation
contracts and shall expose two feature-gated command interfaces:

- `surgeist-layout-generate` for the existing Chromium measurement, constrained
  HTML handling, and layout XML generation behavior;
- `surgeist-css-generate` for pinned CSSTree fixture ingestion and deterministic
  neutral CSS expectation generation.

The crate shall provide one small shared core for manifest/version contracts,
strict paths, source revision verification, deterministic inventory, case
dispositions, hashes and provenance, generation reports and leases, atomic
artifact installation, stale-output removal, full-versus-filtered behavior, and
offline verification. Chromium measurement, async runtime, URL, HTTPS, and ZIP
dependencies shall only compile with `layout-browser`. CSS corpus code shall only
compile with `css-corpus`. The default library shall compile only the shared core.

Acceptance requires all of the following:

1. The layout generator production source is first preserved byte-for-byte from
   the verified layout source blob before any transformation.
2. The final layout driver accepts an explicitly supplied owner root and corpus
   root, reads layout-owned manifests and helper assets in place, and retains the
   current schema-2 manifest, XML, provenance, report, command, and generation
   semantics described in this specification.
3. The final CSS driver accepts an explicitly supplied owner root and corpus
   root, imports a user-supplied verified CSSTree checkout without network access,
   generates schema-1 neutral expectations, and verifies the resulting corpus
   offline.
4. Both binaries contain interface plumbing only. Domain behavior and reusable
   contracts live in the library.
5. Apart from the one audited production-prefix copy required by item 1 and
   SG-02.2, no layout or CSS manifest, fixture, generated expectation, test,
   source file, or repository configuration is copied into this repository.
   Generator tests use synthetic temporary corpora and local temporary Git
   repositories.
6. No production Surgeist crate depends on `surgeist-generator` during its normal
   build. Future layout, CSS, and root wiring are handoffs, not changes in this
   initiative.
7. The default, each individual feature, and both features together pass the
   configured offline check, test, Clippy, format, and unsafe-absence gates at
   Rust 1.97 semantics.

## SG-02 Ownership, evidence, and non-goals

### SG-02.1 Owning repository

Only `/Users/codex/Development/surgeist-generator` owns implementation changes
for this initiative. It owns its manifest, library front door, shared generator
contracts, the two binaries, focused synthetic tests, documentation, commits,
and published candidate.

`surgeist-layout` remains the owner of its layout algorithms, browser-parity
manifest, HTML fixtures, helper JavaScript and CSS, XML expectations, generation
reports, and parity tests. `surgeist-css` remains the owner of CSS parsing,
future CSS corpus manifests, imported source fixtures, neutral expectations, and
consumer tests. Root `surgeist` remains the owner of the facade, crate roster,
gitlinks, integration, and generated API audit artifacts.

### SG-02.2 Verified layout source

The source repository was observed clean at
`24fbdd097f815e19ae71029fa664de3160236e62`. Its local `main` was eleven commits
ahead of its `origin/main`, but the generator file was unchanged across that
range. The latest commit that changed the generator and is an ancestor of the
observed `origin/main` is
`92054de23b7c4d431556ef7e42e2226dd1788f1f`.

The authoritative copy source is:

`/Users/codex/Development/surgeist-layout/tests/bin/surgeist-layout-generate/generator.rs`

at commit `92054de23b7c4d431556ef7e42e2226dd1788f1f`. The complete file SHA-256 is
`5310001e3b6578fac4776b24a307cd6805157f0cae73589e9bb04f5c3d11b78b`.
The production prefix ends immediately before `#[cfg(test)] mod tests` at source
line 4627; lines 1 through 4626, including their final newline, have SHA-256
`d2f5ca87cea6b36826e9172e2d7ba7a99196c375e2ca53f8a84a075200e70a9f`.
The original ten-line binary wrapper has SHA-256
`42458c32d67fe3603ecfafb5ffbea0f199b9f687a2b3c2565d7c1c461f30a33e`.

The inline layout tests beginning at source line 4627 are not part of the copy.
They remain owned and preserved by `surgeist-layout`. Generator-owned behavior
shall receive focused synthetic tests in this repository.

Read-only migration evidence also inspected the committed schema-2 browser
manifest and the already-present local Chrome-for-Testing cache without executing
either. The manifest pins `149.0.7827.115` and the exact launch profile reproduced
in SG-05.2. The cache unit
`target/surgeist-browser/mac_arm-149.0.7827.115` contained 315 non-root
directories, 331 regular files, and the five SG-05.2 links; its SG-05.2 logical
inventory digest was
`5ef8a535ec2e28729c989886a728517681a4f30c18819e98dd2cbe018bd3070a`.
No layout file/cache byte was modified and no layout command or test was run.

Before transformation, the production prefix shall exist as the transient
tracked file `src/layout/legacy_generator.rs`. It contains exactly source lines
1 through 4626, including their final newline, and has SHA-256
`d2f5ca87cea6b36826e9172e2d7ba7a99196c375e2ca53f8a84a075200e70a9f`.
Audit evidence preserves the authoritative source revision/path, destination,
digest, and byte-comparison result. The copy remains deliberately unreferenced
until transformation, then may be split, adapted, and removed while preserving
behavior in the final modules. This exemption applies only to that identified
production prefix; all other layout and CSS source, tests, manifests, corpora,
helpers, and artifacts remain prohibited from copying.

### SG-02.3 CSS fixture evidence

The official CSSTree repository exposes JSON fixtures under `fixtures/ast`,
partitioned by parsing context. Observed examples use a top-level object whose
ordinary named entries contain `source`, optional `options`, optional `generate`,
and an `ast`, while an `error` array contains rejected inputs with `source` and
CSSTree-specific diagnostics. The generator contract shall accept that structural
shape without embedding CSSTree ASTs, offsets, or diagnostic prose in Surgeist's
neutral artifacts. The manifest, not generator source, shall pin the exact
CSSTree repository and revision.

Evidence links:

- <https://github.com/csstree/csstree/tree/master/fixtures/ast>
- <https://raw.githubusercontent.com/csstree/csstree/master/fixtures/ast/declaration/Declaration.json>
- <https://raw.githubusercontent.com/csstree/csstree/master/fixtures/ast/stylesheet/errors.json>

### SG-02.4 Explicit non-goals

This initiative does not:

- edit, format, test, fetch, commit, push, or otherwise mutate
  `surgeist-layout`;
- edit, test, or resolve the pre-existing untracked planning state in
  `surgeist-css`;
- copy layout or CSS corpora, manifests, helpers, expectations, reports, or
  domain tests into `surgeist-generator`;
- add `surgeist-generator` to the root workspace, root crate roster, or root API
  artifacts;
- remove the original layout generator or rewire layout/CSS scripts and tests;
- run Chromium, fetch Chrome-for-Testing, fetch Taffy, clone CSSTree, install
  Node, or acquire any other external software;
- make CSSTree's tolerant AST a `surgeist-css` public model or require a normal
  production dependency on generator code;
- create a generalized plugin framework, open driver trait, CI workflow, copied
  policy, corpus mirror, or synchronization service.

## SG-03 Architecture and dependency boundary

### SG-03.1 Module layout

The final source layout shall use these ownership boundaries. Small helper files
may be combined only when the resulting boundary remains identical.

```text
src/lib.rs
src/error.rs
src/core/mod.rs
src/core/artifact.rs
src/core/case.rs
src/core/corpus.rs
src/core/fs.rs
src/core/hash.rs
src/core/lease.rs
src/core/manifest.rs
src/core/report.rs
src/core/source.rs
src/layout/mod.rs
src/layout/browser.rs
src/layout/manifest.rs
src/layout/xml.rs
src/css/mod.rs
src/css/manifest.rs
src/css/neutral.rs
src/bin/surgeist-layout-generate.rs
src/bin/surgeist-css-generate.rs
```

`core` owns domain-neutral invariants and file transactions. `layout` owns the
schema-2 layout manifest interpretation, Taffy import policy, Chromium lifecycle,
HTML helper injection, measurement conversion, and XML/report compatibility.
`css` owns the schema-1 CSS corpus manifest interpretation, CSSTree structural
ingestion, and neutral expectation/report shape. No open driver trait is needed:
the two known drivers call concrete shared-core functions.

### SG-03.2 Feature and dependency matrix

The package retains edition 2024, version `0.1.0`, MIT licensing, and Rust 1.97.
The dependency matrix is:

| Dependency | Version | Default core | `layout-browser` | `css-corpus` | Reason |
| --- | --- | --- | --- | --- | --- |
| `serde` | `=1.0.228` with `derive` | yes | inherited | inherited | Strict manifest/report schemas |
| `serde_json` | `=1.0.145` | yes | inherited | inherited | Reports, layout measurements, CSSTree fixtures |
| `sha2` | `=0.10.9` | yes | inherited | inherited | Source and artifact SHA-256 |
| `toml` | `=0.9.8` | yes | inherited | inherited | Domain manifest parsing |
| `rustix` | `=1.1.4`, `fs`, Apple Silicon macOS only | yes on the supported target | inherited | inherited | Safe descriptor-relative, non-following filesystem transactions |
| `chromiumoxide` | `=0.9.1`, no defaults, `bytes` | no | yes | no | Chromium measurement without its unsafe path-based fetcher |
| `futures` | `=0.3.31` | no | yes | no | Chromium handler stream |
| `tokio` | `=1.48.0`, `fs`, `io-util`, `macros`, `rt-multi-thread`, `time` | no | yes | no | Private Chromium runtime, bounded download, handler lifecycle, and timed cleanup |
| `url` | `=2.5.7` | no | yes | no | Fixture and base URL handling |
| `reqwest` | `=0.13.4`, no defaults, `rustls` | no | yes | no | HTTPS-only pinned browser archive download |
| `zip` | package `zip`, `=8.6.0`, no defaults, `deflate-flate2-zlib-rs` | no | yes | no | Entry-level browser archive reading for rooted extraction |

All six heavy dependencies are `optional = true`. The exact feature edges are
`default = []`,
`layout-browser = ["dep:chromiumoxide", "dep:futures", "dep:tokio", "dep:url", "dep:reqwest", "dep:zip"]`,
and `css-corpus = []`. `layout-browser` also gates the layout module/binary;
`css-corpus` gates the CSS module/binary and activates no dependency. Both
features may be enabled together. The two binaries use
`required-features` so an unrequested driver cannot compile accidentally.
`rustix = { version = "=1.1.4", features = ["fs"] }` is declared under
`[target.'cfg(all(target_os = "macos", target_arch = "aarch64"))'.dependencies]`;
it is a shared lifecycle dependency, not a domain or default feature switch.

All named dependency sources, including `rustix` 1.1.4, are already present in
the local Cargo registry. `Cargo.lock` is already tracked and `.gitignore` does
not ignore it. The final lockfile resolves the exact manifest entirely from the
local cache and is committed before the locked verification matrix. No dependency
acquisition occurs; the current cycle plan owns lockfile-refresh mechanics.

### SG-03.3 Public front door

`src/lib.rs` remains `#![forbid(unsafe_code)]` and retains
`CRATE_NAME: &str = "surgeist-generator"`. The complete default-feature root
surface is this exact reexport set; `core` and `error` remain private modules:

```rust
pub use core::{
    ArtifactProvenance, CaseDisposition, CaseDispositionRecord, CorpusLocation,
    GenerationCounts, GenerationReport, ManifestVersion, PinnedSource,
    RelativePath, ReportArtifact, RunScope, Sha256Digest, SourceRevision,
    VerifiedSource, collect_regular_files, parse_manifest,
    validate_disposition_records, verify_git_source,
};
pub use error::{GeneratorError, GeneratorErrorKind, Result};
pub const CRATE_NAME: &str = "surgeist-generator";
```

The feature-gated additions are `layout::LayoutRequest`,
`layout::LayoutCommand`, `layout::run`, and `layout::run_from_env` only with
`layout-browser`, and `css::CssRequest`, `css::CssCommand`, `css::run`, and
`css::run_from_env` only with `css-corpus`. No other public module, type, free
function, constructor, field, or method is part of this cycle's surface.

All public structs have private fields and checked constructors. Public enums
whose variants may evolve are `#[non_exhaustive]`. The library does not expose
Chromiumoxide, Tokio task, Serde JSON value, filesystem lock, or child-process
types in public signatures. The public API is additive relative to the scaffold.

The feature-gated driver API is exact; private request struct fields are omitted
from this signature outline:

```rust
pub mod layout {
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct LayoutRequest {
        location: CorpusLocation,
        command: LayoutCommand,
        browser_path: Option<RelativePath>,
        filter: Option<RelativePath>,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[non_exhaustive]
    pub enum LayoutCommand {
        Generate,
        GenerateExisting,
        CheckCorpus,
        CheckTaffyCorpus,
        ImportTaffy,
    }

    impl LayoutRequest {
        pub fn new(
            location: CorpusLocation,
            command: LayoutCommand,
            browser_path: Option<RelativePath>,
            filter: Option<RelativePath>,
        ) -> Result<Self>;
        pub fn location(&self) -> &CorpusLocation;
        pub fn command(&self) -> LayoutCommand;
        pub fn browser_path(&self) -> Option<&RelativePath>;
        pub fn filter(&self) -> Option<&RelativePath>;
    }

    pub fn run(request: LayoutRequest) -> Result<()>;
    pub fn run_from_env() -> Result<()>;
}

pub mod css {
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct CssRequest {
        location: CorpusLocation,
        command: CssCommand,
        source_root: Option<std::path::PathBuf>,
        filter: Option<RelativePath>,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[non_exhaustive]
    pub enum CssCommand {
        ImportCsstree,
        Generate,
        CheckCorpus,
    }

    impl CssRequest {
        pub fn new(
            location: CorpusLocation,
            command: CssCommand,
            source_root: Option<std::path::PathBuf>,
            filter: Option<RelativePath>,
        ) -> Result<Self>;
        pub fn location(&self) -> &CorpusLocation;
        pub fn command(&self) -> CssCommand;
        pub fn source_root(&self) -> Option<&std::path::Path>;
        pub fn filter(&self) -> Option<&RelativePath>;
    }

    pub fn run(request: CssRequest) -> Result<()>;
    pub fn run_from_env() -> Result<()>;
}
```

Both request structs have private fields and exactly
`Clone + Debug + Eq + PartialEq`; they have no Serde, default, conversion, or
other explicit trait implementation and no `#[non_exhaustive]` attribute. Both
command enums have exactly the traits and attribute shown. Request constructors
enforce the complete option matrix in SG-11 before any domain I/O; the CSS
constructor canonicalizes the required import source root and rejects a missing,
non-directory, non-UTF-8, or otherwise supplied source root. The layout
constructor retains the browser path as a checked owner-relative path for later
manifest-cache containment validation.

`run_from_env` reads `std::env::args_os()` only for command-line arguments; it
does not read environment overrides. `run` and `run_from_env` return `Ok(())`
only after every authorized artifact and canonical full report is atomically
installed and verified. Check/import commands and filtered generation expose no
in-memory report; reports remain corpus-owned files. A filtered success never
writes the canonical report. Any partial or failed lifecycle returns the one
semantic `GeneratorError` that the binary prints.

The layout functions are deliberately synchronous at the public boundary; no
caller-visible future can be dropped while Chromium or a lease remains active.
`layout::run` moves the checked request into one named private OS worker thread,
builds and drives the Tokio runtime only on that thread, and synchronously joins
it. It is therefore safe to call from either synchronous code or from a thread
that already participates in another Tokio runtime. Thread spawn or runtime build
failure maps to `Generation` before authority/resources exist. A normal join
returns the worker's semantic result; an outer worker panic is resumed rather
than mislabeled as completed cleanup. The async operation body is wrapped in
`AssertUnwindSafe(...).catch_unwind()` *inside* the still-live private runtime,
with its lease and resource registry owned by the enclosing supervisor; an
operation panic therefore becomes cleanup input rather than unwinding those
handles. The function returns only after the supervisor's lease and every domain
resource have reached the terminal cleanup state in SG-10. No handler task or
browser cleanup is detached.

### SG-03.4 Exact shared-core API

The shared-core contract is closed. The following table lists every explicit
public trait implementation; compiler-derived auto traits are not additional API
commitments.

| Type | Explicit public traits and attributes |
| --- | --- |
| `GeneratorErrorKind` | `Clone + Copy + Debug + Eq + PartialEq`, `#[non_exhaustive]` |
| `GeneratorError` | `Debug + Display + std::error::Error` |
| `RelativePath` | `Clone + Debug + Eq + Ord + PartialEq + PartialOrd + Hash + Serialize + Deserialize` |
| `CorpusLocation` | `Clone + Debug + Eq + PartialEq` |
| `RunScope` | `Clone + Debug + Eq + PartialEq`, `#[non_exhaustive]` |
| `ManifestVersion` | `Clone + Copy + Debug + Eq + Hash + Ord + PartialEq + PartialOrd + Serialize + Deserialize` |
| `SourceRevision` | `Clone + Debug + Display + Eq + Hash + Ord + PartialEq + PartialOrd + Serialize + Deserialize` |
| `PinnedSource` | `Clone + Debug + Eq + PartialEq + Serialize + Deserialize` |
| `VerifiedSource` | `Clone + Debug + Eq + PartialEq` |
| `CaseDisposition` | `Clone + Copy + Debug + Eq + PartialEq + Serialize + Deserialize`, `#[non_exhaustive]` |
| `CaseDispositionRecord` | `Clone + Debug + Eq + PartialEq + Serialize + Deserialize` |
| `Sha256Digest` | `Clone + Debug + Display + Eq + Hash + Ord + PartialEq + PartialOrd + Serialize + Deserialize` |
| `ArtifactProvenance` | `Clone + Debug + Eq + PartialEq + Serialize + Deserialize` |
| `ReportArtifact` | `Clone + Debug + Eq + PartialEq + Serialize + Deserialize` |
| `GenerationCounts` | `Clone + Copy + Debug + Eq + PartialEq + Serialize + Deserialize` |
| `GenerationReport` | `Clone + Debug + Eq + PartialEq + Serialize + Deserialize` |

The enum variants and error observations are exact:

```rust
#[non_exhaustive]
pub enum GeneratorErrorKind {
    Cli,
    InvalidPath,
    InvalidManifest,
    InvalidInventory,
    SourceVerification,
    UnsupportedPlatform,
    LeaseActive,
    Process,
    Io,
    ArtifactTransaction,
    Generation,
    Verification,
}

impl GeneratorError {
    pub const fn kind(&self) -> GeneratorErrorKind;
    pub const fn exit_code(&self) -> u8;
}

#[non_exhaustive]
pub enum RunScope {
    Full,
    Filtered(RelativePath),
}

#[non_exhaustive]
pub enum CaseDisposition {
    Active,
    ExpectedFail,
    Unsupported,
    Quarantined,
}
```

`GeneratorError` has no public constructor; checked public operations return it
through `Result<T>`. Path, corpus, scope, manifest, source, disposition, and hash
operations are exactly:

```rust
impl RelativePath {
    pub fn new(value: impl AsRef<str>) -> Result<Self>;
    pub fn with_extension(value: impl AsRef<str>, expected: &str) -> Result<Self>;
    pub fn as_str(&self) -> &str;
    pub fn join(&self, root: impl AsRef<std::path::Path>) -> std::path::PathBuf;
    pub fn resolve_existing(
        &self,
        root: impl AsRef<std::path::Path>,
    ) -> Result<std::path::PathBuf>;
    pub fn resolve_output(
        &self,
        root: impl AsRef<std::path::Path>,
    ) -> Result<std::path::PathBuf>;
}

impl CorpusLocation {
    pub fn new(
        owner_root: impl AsRef<std::path::Path>,
        corpus_root: impl AsRef<std::path::Path>,
    ) -> Result<Self>;
    pub fn owner_root(&self) -> &std::path::Path;
    pub fn corpus_root(&self) -> &std::path::Path;
}

impl RunScope {
    pub const fn may_write_report(&self) -> bool;
    pub const fn may_remove_stale(&self) -> bool;
    pub fn includes(&self, source: &RelativePath) -> bool;
    pub fn require_match(&self, sources: &[RelativePath]) -> Result<()>;
}

impl ManifestVersion {
    pub fn new(value: u64) -> Result<Self>;
    pub const fn get(self) -> u64;
    pub fn require(
        self,
        expected: Self,
        manifest_path: impl AsRef<std::path::Path>,
    ) -> Result<()>;
}

pub fn parse_manifest<T: serde::de::DeserializeOwned>(
    text: &str,
    manifest_path: impl AsRef<std::path::Path>,
) -> Result<T>;

impl SourceRevision {
    pub fn new(value: impl AsRef<str>) -> Result<Self>;
    pub fn as_str(&self) -> &str;
}

impl PinnedSource {
    pub fn new(
        label: impl Into<String>,
        repository_url: impl Into<String>,
        revision: SourceRevision,
        source_subdirectory: RelativePath,
    ) -> Result<Self>;
    pub fn label(&self) -> &str;
    pub fn repository_url(&self) -> &str;
    pub const fn revision(&self) -> &SourceRevision;
    pub const fn source_subdirectory(&self) -> &RelativePath;
}

impl VerifiedSource {
    pub fn canonical_root(&self) -> &std::path::Path;
    pub fn canonical_source_root(&self) -> &std::path::Path;
    pub const fn revision(&self) -> &SourceRevision;
}

pub fn verify_git_source(
    checkout: impl AsRef<std::path::Path>,
    pin: &PinnedSource,
) -> Result<VerifiedSource>;

pub fn collect_regular_files(
    root: impl AsRef<std::path::Path>,
    extension: &str,
) -> Result<Vec<RelativePath>>;

impl CaseDispositionRecord {
    pub fn new(
        case_id: impl Into<String>,
        source_path: RelativePath,
        disposition: CaseDisposition,
        reason: Option<impl Into<String>>,
    ) -> Result<Self>;
    pub fn case_id(&self) -> &str;
    pub const fn source_path(&self) -> &RelativePath;
    pub const fn disposition(&self) -> CaseDisposition;
    pub fn reason(&self) -> Option<&str>;
}

pub fn validate_disposition_records(
    records: Vec<CaseDispositionRecord>,
) -> Result<Vec<CaseDispositionRecord>>;

impl Sha256Digest {
    pub fn from_bytes(bytes: impl AsRef<[u8]>) -> Self;
    pub fn from_text(value: impl AsRef<str>) -> Result<Self>;
    pub fn from_file(path: impl AsRef<std::path::Path>) -> Result<Self>;
    pub fn as_str(&self) -> &str;
}
```

`validate_disposition_records` rejects duplicate case IDs and sorts the returned
records by case ID. It deliberately permits repeated `source_path` values:
multiple independently identified cases may derive from one source file. A
domain that requires one case per source performs that stricter check in its
manifest validator.

Provenance, report, transaction, and lease operations are exactly:

```rust
impl ArtifactProvenance {
    pub fn new(
        source_path: RelativePath,
        source_digest: Sha256Digest,
        generator: impl Into<String>,
        schema_version: ManifestVersion,
        domain_provenance: std::collections::BTreeMap<String, Sha256Digest>,
    ) -> Result<Self>;
    pub const fn source_path(&self) -> &RelativePath;
    pub const fn source_digest(&self) -> &Sha256Digest;
    pub fn generator(&self) -> &str;
    pub const fn schema_version(&self) -> ManifestVersion;
    pub const fn domain_provenance(
        &self,
    ) -> &std::collections::BTreeMap<String, Sha256Digest>;
}

impl ReportArtifact {
    pub fn new(
        provenance: ArtifactProvenance,
        output_path: RelativePath,
        output_digest: Sha256Digest,
        case_count: usize,
    ) -> Result<Self>;
    pub const fn provenance(&self) -> &ArtifactProvenance;
    pub const fn output_path(&self) -> &RelativePath;
    pub const fn output_digest(&self) -> &Sha256Digest;
    pub const fn case_count(&self) -> usize;
}

impl GenerationCounts {
    pub fn new(
        active: usize,
        expected_fail: usize,
        unsupported: usize,
        quarantined: usize,
        failed_to_generate: usize,
    ) -> Result<Self>;
    pub fn total(self) -> Result<usize>;
    pub const fn active(self) -> usize;
    pub const fn expected_fail(self) -> usize;
    pub const fn unsupported(self) -> usize;
    pub const fn quarantined(self) -> usize;
    pub const fn failed_to_generate(self) -> usize;
}

impl GenerationReport {
    pub fn new(
        manifest_digest: Sha256Digest,
        source_repository: impl Into<String>,
        source_revision: SourceRevision,
        counts: GenerationCounts,
        artifacts: Vec<ReportArtifact>,
    ) -> Result<Self>;
    pub const fn manifest_digest(&self) -> &Sha256Digest;
    pub fn source_repository(&self) -> &str;
    pub const fn source_revision(&self) -> &SourceRevision;
    pub const fn counts(&self) -> GenerationCounts;
    pub fn artifacts(&self) -> &[ReportArtifact];
    pub fn verify_files(
        &self,
        corpus_root: impl AsRef<std::path::Path>,
        manifest_path: &RelativePath,
    ) -> Result<()>;
}

```

Mutation authority is deliberately not public. Crate-private `ArtifactPlan`,
composite publication plans, and `GenerationLease` accept only domain-validated
paths and metadata. Every plan stores the exact `CorpusLocation` identity and
domain; install requires a still-live lease whose private corpus/domain identity
matches. A released, absent, foreign-domain, or foreign-corpus lease cannot reach
capability probing or writes. Domain drivers are the only constructors, so a
generic library caller cannot select `corpus.toml`, helpers, fixtures, or another
domain's publication root as a mutation target.

Those internal mutation types are active only on the single SG-09 supported
target. Rooted descriptors, capability probes, Git snapshot objects and bytes,
failure-injection hooks, and error constructors also remain crate-private.
`ReportArtifact::new` rejects a zero or greater-than-`u32::MAX` case count and
`GenerationCounts::new` rejects any field or checked aggregate above
`u32::MAX`; `GenerationReport::new`
performs duplicate, inventory, and header validation, and
`GenerationCounts::total` repeats that same portable checked sum for
deserialized values.

`GenerationReport::new` validates count overflow, sorted unique output paths, and
report header fields, but does not require summed `ReportArtifact::case_count` to
equal `GenerationCounts::total()`. Layout can have failed/unsupported cases with
no artifact, while a CSS expectation artifact can document non-active cases.
Each domain report validator owns its exact artifact-to-case/count relationship.

Domain cache acquisition outside the corpus uses the same crate-private rooted
transaction machinery. No public plan or lease type exists.

Every semantic string accepted by this public surface has one exact grammar:

- an `identifier` is 1 through 64 ASCII bytes, starts with `[a-z0-9]`, and has
  only `[a-z0-9._-]` thereafter; it has no `..` substring. Source labels,
  generator names, lease domains/commands, artifact-plan domains, and domain-
  provenance keys use this grammar;
- a case ID is 1 through 4,096 UTF-8 bytes. Its part before an optional single
  `#` is a valid `RelativePath`; when present, the suffix is empty or an RFC 6901
  JSON Pointer beginning `/`, and every `~` escape is exactly `~0` or `~1`.
  ASCII controls, backslashes, and leading/trailing Unicode whitespace are
  therefore rejected;
- a reason is 1 through 2,048 UTF-8 bytes, equals its Unicode-trimmed form, and
  contains no control character. `Active` forbids one; every other disposition
  requires one;
- a repository URL is ASCII and has exactly the form `https://<dns>/<path>.git`:
  the lowercase DNS authority has two or more nonempty `[a-z0-9-]` labels, no
  userinfo or port, and the nonempty slash-separated path uses visible ASCII
  other than `%`, `?`, `#`, or backslash and has no empty, `.` or `..` segment;
- a source revision is exactly 40 or 64 lowercase hexadecimal bytes; a generated
  extension is 1 through 16 lowercase ASCII alphanumeric bytes without a dot.

The public Serde wire contract is also exact. JSON object fields are serialized
in the listed order, every listed field except the conditional `reason` below is
required on input, and no alias, flattened form, implicit default, or additional
field is accepted:

| Type | Exact JSON representation |
| --- | --- |
| `RelativePath` | one JSON string containing the checked relative-path spelling |
| `ManifestVersion` | one positive unsigned JSON integer |
| `SourceRevision` | one JSON string containing the checked full object ID |
| `PinnedSource` | object fields `label`, `repository_url`, `revision`, `source_subdirectory` |
| `CaseDisposition` | exactly one of the strings `active`, `expected-fail`, `unsupported`, `quarantined` |
| `CaseDispositionRecord` | object fields `case_id`, `source_path`, `disposition`, then conditional `reason`; canonical `active` output omits `reason` and input accepts omitted or JSON `null`, while every non-active value requires one string and canonical output includes it |
| `Sha256Digest` | one JSON string containing exactly 64 lowercase hexadecimal bytes |
| `ArtifactProvenance` | object fields `source_path`, `source_digest`, `generator`, `schema_version`, `domain_provenance` |
| `ReportArtifact` | object fields `provenance`, `output_path`, `output_digest`, `case_count` |
| `GenerationCounts` | object fields `active`, `expected_fail`, `unsupported`, `quarantined`, `failed_to_generate` |
| `GenerationReport` | object fields `manifest_digest`, `source_repository`, `source_revision`, `counts`, `artifacts` |

`ManifestVersion` accepts 1 through `u64::MAX`. Every public count accepts 0
through `u32::MAX`, except `ReportArtifact.case_count`, which accepts 1 through
`u32::MAX`; constructors enforce this portable wire bound even when `usize` is
wider. The checked sum of all five `GenerationCounts` fields must also be at most
`u32::MAX` on every target; construction, deserialization, and `total()` enforce
the same bound. Canonical JSON serialization emits unsigned base-10 digits with no leading
zero except `0`. JSON fractions, exponent spellings, negatives, strings, and
overflow fail. `domain_provenance` is an object whose decoded keys are unique,
checked identifiers and whose serialization uses `BTreeMap` key order.
`GenerationReport::new` sorts artifacts by output path before duplicate checks,
and deserialization repeats that normalization, so `artifacts` is canonically
sorted on output. Serialization uses these canonical scalar forms and field
visits, giving compact Serde JSON one byte-exact golden representation for every
public type.

`ManifestVersion::new` additionally requires a nonzero value.
`PinnedSource::new` validates its label, URL, revision, and subdirectory.
`ArtifactProvenance::new` validates its generator and every unique `BTreeMap`
key. `GenerationReport::new` validates its repository URL. Lease acquisition
validates domain, generator, and command; plan construction validates domain,
extension, unique paths, scope/retained-inventory agreement, and content.

Checked-constructor error kinds are fixed: path/location failures are
`InvalidPath`; manifest-version failures are `InvalidManifest`; source revision,
source label, and repository failures are `SourceVerification`; case/reason/count
failures are `InvalidInventory`; digest/provenance/report failures are
`Verification`; lease metadata failures are `InvalidInventory`; and plan domain,
extension, or inventory failures are `ArtifactTransaction` (while a nested
`RelativePath` failure remains `InvalidPath`). Domain manifest validators remap a
well-formed but semantically invalid manifest field to `InvalidManifest` before
constructing these generic values, so public-constructor kinds do not blur CLI
schema diagnostics.

Every public type in the trait table with `Deserialize` uses a private checked
visitor or a `#[serde(deny_unknown_fields)]` raw representation and calls the
same checked constructor; it never derives field-wise deserialization that can
bypass an invariant. This explicitly includes the scalar `RelativePath`,
`ManifestVersion`, `SourceRevision`, `CaseDisposition`, and `Sha256Digest`
visitors as well as the raw representations for `PinnedSource`,
`CaseDispositionRecord`, `ArtifactProvenance`, `ReportArtifact`,
`GenerationCounts`, and `GenerationReport`. `GenerationReport` recursively
revalidates its nested values and aggregate inventory. Raw object visitors reject
repeated or unknown fields, and provenance maps use a custom map visitor that
rejects a repeated decoded key instead of allowing `BTreeMap`
last-value-wins behavior. Wrong scalar kinds and noncanonical enum spellings
fail. Constructor failures become deterministic `serde::de::Error::custom`
messages prefixed by the semantic kind. Domain manifest parsing maps any such
error to `InvalidManifest`; generated report/expectation parsing maps it to
`Verification`. Direct callers of Serde receive the Serde error, as required by
that trait rather than a `GeneratorError`.

## SG-04 Semantic core types

### SG-04.1 Corpus location

`CorpusLocation` owns two canonical absolute paths:

- an owner root, used for repository-relative acquisition caches and provenance;
- a corpus root, which must exist as a directory at construction and must be the
  owner root itself or a descendant of it.

Construction rejects missing roots, non-directories, non-UTF-8 CLI inputs,
canonicalization failures, and corpus roots that escape the owner root through
lexical components or symlinks. It also rejects an owner or corpus root whose
canonical components contain the reserved exact name `.surgeist-generator`, so
a new location cannot be rooted at or beneath another corpus's coordination
directory. Callers cannot mutate either path after construction.

Every binary invocation requires explicit `--owner-root <path>` and
`--corpus-root <path>`. There is no default corpus, `CARGO_MANIFEST_DIR` fallback,
current-directory inference, or corpus-root environment override. A consumer may
pass relative CLI paths, but construction canonicalizes them before use.

The owner root is intentionally not lease identity: two different canonical
owner ancestors may validly describe one corpus. The reserved coordination root
is always `<canonical-corpus-root>/.surgeist-generator/`, so every
`CorpusLocation` for the same canonical corpus converges on one filesystem
namespace. Domain manifests, artifacts, reports, caches, and imports may not use
`.surgeist-generator` as any relative-path component at any depth. Coordination
files are generator state, have no generated extension, and are excluded from
domain artifact/report inventories; recursive domain walks do not enter any
directory with that exact component name.

Those namespace rules use filesystem equivalence, not Rust string equality, and
the proof is local to the actual held parent directory. No result is inferred
from another directory merely because it has the same mount identity. Before a
planned component is opened, created, compared with a sibling namespace, or used
as a transaction destination, the rooted capability establishes the exact pair
relationships needed in that component's parent descriptor:

1. it opens every existing spelling without following, reads the directory
   entry's exact on-disk name, and compares descriptor identity;
2. when exactly one spelling is absent it records a synced private probe journal
   and attempts that exact missing spelling with `NOREPLACE`; when both are absent
   it journal-creates the first exact spelling and then attempts the second.
   Lookup and descriptor identity distinguish an alias from two entries;
3. it removes only its verified single-link probes, then rechecks the parent
   identity and mount before accepting the pair result.

The result applies only to that candidate pair in that parent. A newly created
directory is reopened, mount-checked, and receives its own direct-parent probes
before any descendant is planned; differing policies in parent and child are
therefore not conflated. Final `NOREPLACE` creation remains authoritative and a
surprising collision fails closed without replacing the encountered object. An
inconclusive probe, cleanup failure, parent-identity change, or inability to
represent either exact pair is `UnsupportedPlatform`. A crash leaves a journaled
single-link probe identity that the next gate holder removes before continuing;
an unjournaled or identity-mismatched entry is never treated as a probe.

Thus `.SURGEIST-GENERATOR` or a normalization variant is reserved only when the
actual relevant parent aliases it to `.surgeist-generator`, and two case-only
manifest roots collide only where their actual shared parent aliases them. An
exact-text conflict remains `InvalidManifest` at semantic validation; a
filesystem-only alias is `InvalidPath` after verified private-probe cleanup and
before mutation authority is returned or a publication root is written. The
lease-held probe journal is the only permitted prior coordination mutation.

Coordination bootstrap avoids a journal circularity. First, a read-only
descriptor walk of the already existing owner/corpus ancestry tries the exact
reserved name in each component's actual parent and compares identity plus the
entry's on-disk spelling with the actual component. If an existing alias places
either root at or beneath coordination, preflight returns `InvalidPath` without
creating it. At the corpus root, opening the exact reserved spelling either finds
an exact existing directory or exposes an aliased on-disk spelling, which is
rejected. When it is absent, the implementation atomically creates and syncs the
exact persistent coordination directory with exclusive descriptor-relative
`mkdirat`, reopens it, requires the exact on-disk spelling/identity, and syncs its
held parent. This directory is deliberate reusable scaffolding rather than a
temporary stage. Only then can it publish a probe journal
inside that directory and test any candidate pair directly in the corpus parent.
A bootstrap failure may therefore leave only an exact empty generator-owned
coordination directory; it never leaves an unjournaled alias or domain file.
Every deeper coordination parent is independently checked before use.

### SG-04.2 Strict relative paths

`RelativePath` is a normalized UTF-8, forward-slash representation. Its checked
constructor rejects:

- empty text, leading or trailing whitespace, NUL, backslashes, absolute paths,
  Windows prefixes, root components, empty segments, `.` segments, and `..`
  segments;
- a caller-requested file path with an unexpected extension;
- an existing filesystem object whose canonical target escapes its declared
  canonical root;
- output targets whose nearest existing canonical ancestor escapes the output
  root.

Ordering and hashing use the normalized forward-slash text. Joining a
`RelativePath` to a root is the only shared-core path-construction mechanism.
Domain schemas may impose stricter extension, component-count, or prefix rules.

### SG-04.3 Run scope

`RunScope` is a closed enum:

- `Full` is verification-capable. On clean success it may install the complete
  artifact set, write the canonical report, remove stale generated artifacts,
  and remove non-manifest reports. The recoverable layout-failure exception in
  SG-09 may install successful artifacts plus a diagnostic canonical report but
  must preserve stale artifacts and non-manifest reports;
- `Filtered(RelativePath)` is iteration-only and may install only matching
  artifacts. It must not write or prune reports, remove stale nonmatching
  artifacts, or count as final verification evidence.

Filters name an exact source fixture or a directory prefix. Construction proves
that at least one source matches before a lease is acquired or any output is
written. Layout permits filters only for `generate-existing`; CSS permits them
only for `generate`. Empty filters are invalid rather than aliases for full runs.

Here and in SG-09, “does not write/prune” is a logical content-and-inventory
guarantee. A one-root atomic swap may replace the inode of an unchanged report,
stale artifact, or authored fixture with a staged regular file having identical
bytes and mode; inode number and modification time are not corpus contracts.
Digest, path, type, mode, inclusion, and exclusion are contracts, and filtered
runs preserve all report bytes exactly.

## SG-05 Manifest and version contracts

### SG-05.1 Shared rules

Every manifest uses `serde(deny_unknown_fields)`, a positive integer
`schema_version`, normalized paths, unique inventories, and exact count checks.
Parsing and semantic validation are separate operations so diagnostics distinguish
malformed TOML from a well-formed but invalid contract. Duplicate TOML keys fail
parsing. Unknown versions fail closed; there is no best-effort downgrade.

`ManifestVersion` is a semantic newtype. Domain code compares it to one exact
supported version and reports the manifest path, actual value, and expected
value on mismatch.

### SG-05.2 Layout schema 2

The layout driver continues to read the existing `corpus.toml` schema 2 without
requiring edits in `surgeist-layout`. Existing sections and generated artifact
formats remain compatible:

- `[browser]` and `[browser.launch]` own the browser source, pin, expected version
  output, cache path, provenance template, and deterministic launch lifecycle;
- `[generation_reports]` owns the full and scoped report inventory and counts;
- `[source_roots]` and `[imports.taffy]` own the Taffy repository, exact revision,
  source directory, destination, expected inventory, exclusions, and descriptions;
- `[[cases]]` owns explicit Surgeist cases and their generator/disposition.

The refactored driver removes hard-coded Taffy repository, revision, directory,
and count values. It validates agreement between the existing source-root and
import sections, then derives the pin from the manifest. Cache placement is
fully specified rather than invented by a worker:

- `browser.source` is exactly `chrome-for-testing`;
- `browser.version` is exactly four dot-separated ASCII decimal components.
  Each component is `0` or starts with `[1-9]`, contains digits only, and parses
  as `u32`; leading zeroes, signs, whitespace, separators, URL characters, and
  additional components are rejected. The currently evidenced pin
  `149.0.7827.115` satisfies this grammar without becoming a hard-coded future
  manifest value;
- `browser.version_output` must equal the byte string
  `Google Chrome for Testing <browser.version>` with no added whitespace. These
  three checks occur during manifest validation, so neither a download URL nor
  a process argument can be formed from unchecked browser text;

Schema 2's remaining browser fields are required and exact. Unknown/missing
fields or a wrong TOML scalar kind are `InvalidManifest`:

| Field | Required value |
| --- | --- |
| `browser.provenance_format` | `chrome-for-testing/{version} ({repository_relative_executable})` |
| `browser.launch.batch_size` | integer `50` |
| `browser.launch.navigation_timeout_ms` | integer `10000` |
| `browser.launch.dom_poll_interval_ms` | integer `25` |
| `browser.launch.retry_count` | integer `1` |
| `browser.launch.job_order` | `sorted-sequential` |
| `browser.launch.retry_error_class` | `open-load-reset-timeout` |
| `browser.launch.profile_scope` | `per-batch-and-retry` |
| `browser.launch.page_scope` | `per-job` |
| `browser.launch.disable_default_args` | boolean `true` |
| `browser.launch.disable_cache` | boolean `true` |

`browser.launch.arguments` is an array of exactly these 28 strings in this
order; duplicates, added switches, leading `--`, NUL/control characters, or a
different order fail manifest validation and the ordered array feeds the launch-
profile digest:

```text
headless=new
mute-audio
disable-background-networking
disable-background-timer-throttling
disable-backgrounding-occluded-windows
disable-breakpad
disable-client-side-phishing-detection
disable-component-extensions-with-background-pages
disable-component-update
disable-default-apps
disable-dev-shm-usage
disable-domain-reliability
disable-features=TranslateUI,MediaRouter,OptimizationHints,AutofillServerCommunication
disable-hang-monitor
disable-ipc-flooding-protection
disable-popup-blocking
disable-prompt-on-repost
disable-renderer-backgrounding
disable-sync
enable-automation
enable-blink-features=IdleDetection,CSSGridLanesLayout
enable-features=NetworkService,NetworkServiceInProcess
force-color-profile=srgb
lang=en_US
metrics-recording-only
no-default-browser-check
no-first-run
use-mock-keychain
```

- schema 2 requires the existing `browser.cache_root` value
  `target/surgeist-browser`; its canonical root is
  `<owner-root>/target/surgeist-browser`. The one browser publication unit is its
  existing platform/version child
  `mac_arm-<browser.version>`;
- the Taffy object cache is
  `<owner-root>/target/surgeist-sources/taffy/<exact-revision>`, preserving the
  copied generator's fixed prefix while deriving the terminal component from the
  manifest pin. That exact revision directory is the one Taffy publication unit
  and is a generator-owned bare object repository, not a checkout;
- `owner_root/target` must already be a same-mount ordinary directory. Browser
  and Taffy stages are unique siblings of their respective final version/revision
  unit, never stages for the whole cache family. Per-run browser profiles live
  outside every immutable cache unit at
  `<owner-root>/target/.surgeist-generator-cache/runtime/browser/<token>`.
  Tokens are registered before creation.

Cache coordination is independent of every corpus. The exact namespace is
`<canonical-owner-target>/.surgeist-generator-cache/`, with a target-wide
`acquisition.lock`, immutable keyed mutexes under `locks/`, journals under
`transactions/`, and the runtime subtree above. A cache key hashes the held
target directory's device/inode/fsid identity plus the normalized final relative
version/revision path. Thus all corpora whose owner resolves to the same target
and cache unit converge on one lock and journal even before the final directory
exists; pathname/mount aliases converge by descriptor identity or fail
`InvalidPath`.

The cache-key preimage is byte-exact:
`b"surgeist-cache-key-v1\0"`, followed by `st_dev` and `st_ino` as unsigned
64-bit big-endian values, the two macOS `fsid_t` words as signed 32-bit big-
endian values, the final-relative-path byte length as unsigned 32-bit big-endian,
and its normalized UTF-8 bytes. The key is the 64-character lowercase SHA-256 of
that preimage. Its immutable mutex is `locks/<key>.lock` with exact contents
`surgeist-generator-cache-lock-v1\n<key>\n`; the target-wide immutable gate has
exact contents `surgeist-generator-cache-gate-v1\n`. Both use SG-10's journaled
atomic lock bootstrap and are never truncated or rewritten.

The `.surgeist-generator-cache` directory itself uses SG-04.1's exact-name,
exclusive-`mkdirat`, reopen, identity/mount, and parent-sync bootstrap against the
held canonical target. A crash may leave that one valid empty persistent
scaffold; no temporary cache-coordination spelling is created.

Under the target gate, `locks`, `transactions`, `runtime`, `runtime/browser`, and
`runtime/runs` are created/adopted with the same persistent-scaffold checks. A
per-key `transactions/<key>` and `runtime/runs/<key>` directory is created only
while that key is held exclusive. These containers may survive empty; unknown or
aliased entries are never adopted as state.

Only `<owner-root>/target` is required beforehand. Under the target-wide gate,
the fixed cache-family scaffolds `surgeist-browser`, `surgeist-sources`, and
`surgeist-sources/taffy` are created one component at a time by exclusive
descriptor-relative creation, parent sync, reopen, exact-name/identity/mount
validation, and a second parent sync. They are persistent container scaffolding,
not replaceable publication units. A crash may leave only one of these complete,
empty, validated prefixes; subsequent acquisition adopts it under the gate. A
pre-existing nonempty scaffold is accepted only when every child is a valid
immutable version/revision unit or recognized registered transaction name under
the applicable keyed journal; unknown content fails closed. The per-key intent
is not created until the exact final-parent descriptor is held.

Managed and existing-browser `generate` plus `import-taffy` take their exact
cache key exclusively; generation needs exclusivity for the run/profile journal
even when the immutable browser unit already exists. Read-only `check-corpus`
and `check-taffy-corpus` take only shared cache guards when they inspect a cache
and fail `Verification` on unresolved keyed state. All lock attempts are
nonblocking; contention returns generic `LeaseActive` context naming only the
cache key and never a stale owner. The browser cache lock is retained until
the child is reaped and the external runtime profile is removed or remains under
its durable recovery intent; the
Taffy cache lock is retained until its immutable snapshot is no longer needed by
import/check. Cache coordination bootstrap/recovery holds only the target-wide
gate briefly. Different cache keys may run concurrently.

Before creating a profile or launching Chrome, the exclusive browser cache guard
creates and syncs
`runtime/runs/<cache-key>/active-<token>/intent.json`. It records the cache key,
unit and target identities, token, and exact `runtime/browser/<token>` profile
name. The profile directory is registered with the same reservation-before-move
protocol as SG-09.1. An immutable `launching` marker is durable before spawn;
`child.json` records the returned PID immediately afterward, and `reaped` is
published only after `wait` proves that exact owned child handle terminated and
the child-created process-group probe returns `ESRCH`.
Profile removal is descriptor-rooted and the run intent is tombstoned/removed
only after durable reap proof and profile removal. If a process dies after
`launching` but before recording the PID, a later acquirer cannot prove that no
child survived and preserves the intent/profile as disputed
`ArtifactTransaction`. With a recorded PID/process-group ID but no `reaped`,
cleanup proceeds only when the safe process-group probe returns `ESRCH`; an
existing/reused group or any
inconclusive result is preserved. Merely mentioning residue in an error is never
terminal accounting. An identity-matching profile cleanup failure retains the
run intent for the next exclusive cache-key acquisition and returns `Generation`;
wrong identity/type/mount returns `ArtifactTransaction` without removal.

The global lock order is: cache keys in normalized key order, then the corpus
gate/domain mutex, then browser/task resources; release is the reverse. No path
acquires a cache lock while holding a corpus lease. A command may therefore
commit one cache transaction and later one corpus transaction, but never has two
transactions in their commit phase together. A successfully published cache is
durable reusable state: if later corpus lease, browser, generation, or corpus
publication fails, the cache remains committed while corpus outputs follow their
own pre/post-commit state. There is no cross-root rollback claim.

The fixed domain names (`chrome-for-testing`, `taffy`, `surgeist`, and
`constrained-html`) remain layout schema semantics. The two source roots and
`imports.taffy.destination` must continue to resolve to the one corpus-relative
`html` tree; XML and every generation report remain beneath the fixed `xml`
publication root, with reports beneath `xml/generation-reports`.

The helper JavaScript and base CSS are loaded from
`scripts/gentest/test_helper.js` and
`scripts/gentest/test_base_style.css` under the supplied corpus root. The helper
directory must contain exactly those two regular files. Their bytes remain
layout-owned and feed the same hashes, browser document, XML provenance, and
report metadata as before.

Layout applies one complete namespace matrix before cache or corpus mutation.
Exact-text relationships are rejected in manifest/location validation; SG-04's
per-parent equivalence and SG-09 mount/identity checks run while the acquisition
gate is held and before a cache/domain write:

- both owner-relative cache families, cache coordination/runtime, and their fixed
  `target` parent must remain canonically beneath `owner_root`; cache families are
  pairwise disjoint and every cache/stage/profile/coordination namespace is
  disjoint in both ancestor directions from the whole canonical corpus root;
- corpus `html`, `xml`, `corpus.toml`, `scripts/gentest`, and coordination roots
  are pairwise disjoint. The two named helper files are the only permitted helper
  children, and `xml/generation-reports` is the intentional report child of
  `xml`; neither cache may alias any of these namespaces;
- browser stages/executables may occur only beneath the browser-cache parent and
  its exact version unit; profiles may occur only beneath cache coordination's
  runtime subtree. Taffy stages, bare Git administration/object paths, and final
  revision may occur only beneath the Taffy-cache parent. Neither family/runtime
  may alias another;
- the Taffy cache's verified protection set may overlap its own acquisition root
  only. Before snapshot bytes can feed publication it must be disjoint from the
  browser cache, corpus `html`/`xml`, manifest, helpers, and coordination roots;
- layout generation mutates only the complete `xml` publication root. Taffy
  import mutates the complete `html` publication root while copying every
  validated Surgeist-authored fixture unchanged and replacing/pruning only the
  manifest-classified Taffy portion. No helper or manifest byte enters either
  transaction.

Equality, either ancestor direction, per-parent case/normalization equivalence,
descriptor-ancestry identity, or a mount crossing outside an explicitly allowed
containment is `InvalidPath`. The planned roots are known before acquisition, so
a conflict cannot be deferred until after fetch. Acquisition-private stages are
cleaned or journaled residues; no manifest/cache/helper/HTML/XML/report final is
changed on a failed matrix check.

Managed Chromium does not call `chromiumoxide_fetcher`: its path-based ZIP
extractor is outside the safety contract. The driver constructs only
`https://storage.googleapis.com/chrome-for-testing-public/<browser.version>/mac-arm64/chrome-mac-arm64.zip`.
A direct Reqwest client is HTTPS-only, disables environment/system proxies and
redirects, uses Rustls, has a 30-second connect and 10-minute request deadline,
and accepts only status 200 at that exact URL. It streams at most 1 GiB into an
already-open exclusive rooted stage file, then flushes and syncs it.

The driver has a closed generator-owned content-pin table. This cycle contains
exactly the evidenced row:

```text
version          149.0.7827.115
platform         mac-arm64
entry counts     315 directories, 331 regular files, 5 symlinks
logical tree sha 5ef8a535ec2e28729c989886a728517681a4f30c18819e98dd2cbe018bd3070a
```

Another manifest version is `InvalidManifest` until a reviewed pin row is added.
The logical tree SHA-256 starts with
`b"surgeist-browser-tree-v1\0"` and, for every non-root entry sorted by UTF-8
relative-path bytes, hashes: a big-endian `u32` path length, path bytes, one type
byte (`D`, `F`, or `L`), and a big-endian `u16` sanitized mode. A regular file
then contributes its big-endian `u64` length and raw 32-byte SHA-256; a symlink
contributes a big-endian `u32` target-byte length and target bytes. The final
version directory retains the archive's one `chrome-mac-arm64` top-level tree.
This complete logical inventory digest—not mutable URL/version text—is the
trusted content pin; harmless ZIP metadata/recompression may differ only when it
extracts to exactly the same pinned bytes, modes, paths, and links. The observed
download SHA-256 is computed for transaction diagnostics but does not add a field
to layout-owned schema-2 XML or reports; their existing browser-version field is
bound to this one reviewed tree pin by the generator contract.

The registered version-unit stage initially contains only a mode-`0600`
`._surgeist-download.zip`. A declared `Content-Length` above 1 GiB is rejected;
streaming enforces the same limit when absent or inaccurate. The stream's SHA-256 is
finished before ZIP parsing. Extraction retains the top-level directory in that
same stage, removes and syncs the archive file only after extraction succeeds,
and prepares the cache transaction only when the root contains exactly the
pinned tree. Every entry's declared/computed size and CRC must verify during a
full read; trailing archive data and a multi-disk archive are rejected.

The ZIP reader performs a complete validation pass before extraction: at most
10,000 entries, 1 GiB per entry, and 2 GiB total uncompressed bytes; UTF-8 strict
relative forward-slash paths; one `chrome-mac-arm64` top-level tree; no duplicate
decoded path, file/directory conflict, unsupported compression, encrypted entry,
device, FIFO, socket, or hard-link representation; and modes exactly `0644` or
`0755` for regular files, `0755` for directories, and `0120777` for the five
symlink entries. Inferred parent directories
receive the same checks. Symlink entry bodies are read during this pass and must
be relative UTF-8 targets that contain no NUL/backslash, normalize within the
archive root, form an acyclic graph, and resolve to another validated entry.

For the pinned Apple Silicon Chrome shape the only permitted symlinks are the
following five beneath
`chrome-mac-arm64/Google Chrome for Testing.app/Contents/Frameworks/Google Chrome for Testing Framework.framework`
(denoted `F`):

```text
F/Versions/Current -> <browser.version>
F/Google Chrome for Testing Framework -> Versions/Current/Google Chrome for Testing Framework
F/Helpers -> Versions/Current/Helpers
F/Libraries -> Versions/Current/Libraries
F/Resources -> Versions/Current/Resources
```

Any missing, extra, differently targeted, broken, escaping, or cyclic symlink is
`SourceVerification`. Extraction then reopens the archive and creates all
directories and regular files first using only held directory descriptors,
exclusive create, no-follow component traversal, exact size accounting, flush,
sync, and sanitized modes. It creates the five already-validated symlinks last
with descriptor-relative `symlinkat`; no later archive write can traverse one.
The extractor never calls `create_dir_all`, `File::create`, `ZipArchive::extract`,
or another pathname-based write.

A final rooted inventory repeats the exact type/mode/link graph and mount checks.
The platform executable is exactly
`chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing`.
It must be a regular single-link `0755` file. Its version probe uses only that
held descriptor-validated path, argv `--version`, trusted working directory `/`,
and a cleared environment with `LC_ALL=C`; it has a ten-second deadline and
4-KiB independent stdout/stderr caps. Timeout/overflow kills and reaps the child.
Success requires status zero and empty stderr. Stdout must be UTF-8 with no NUL
or control byte except exactly one terminal LF (with an optional immediately
preceding CR); removing that one line ending must yield byte-exact
`browser.version_output`. No whitespace splitting or trimming occurs.

Measurement uses a private direct launcher and then connects Chromiumoxide to
the reported DevTools WebSocket; it does not use Chromiumoxide's ambient-
environment/process launcher. The exact Chrome argv is the 28 manifest strings
rendered as `--<string>` in manifest order, followed by
`--remote-debugging-port=0`, `--disable-extensions`, and
`--user-data-dir=<absolute registered profile>`. No default or platform-detected
argument is added. The child uses cwd `/`, clears its environment and sets only
`HOME=<profile>`, `TMPDIR=<profile>/tmp`, `LC_ALL=C`, and `LANG=C`; both private
directories are rooted/registered first. The direct launcher places the browser
and descendants in a new process group whose ID equals the child PID and records
both in the run intent only after a safe `getpgid` check confirms that equality;
a mismatch kills/reaps the direct child without group signalling and fails
`Process`. Stdout is null and stderr is drained by
an owned task. Within 20 seconds and 64 KiB of startup stderr there must be
exactly one syntactically valid
`DevTools listening on ws://127.0.0.1:<ephemeral-port>/devtools/browser/<uuid>`
line; otherwise the child is killed/reaped. The drain remains owned after
connection and a one-MiB total cap is fatal. After connection, the driver
explicitly disables the browser cache through CDP before opening a job page.
Navigation uses the exact 10-second/25-millisecond manifest intervals, sorted
sequential batches of 50, one retry only for the named retry class, one page per
job, and one registered profile per batch/retry.

That version directory alone is the SG-09 cache publication unit.
`generate-existing` requires its owner-relative browser path to equal the exact
cache-root/version/executable path above, applies the same inventory, five-link
graph, containment, executable, tree-pin, and version validation to the already
published version unit, and never fetches. Synthetic tests encode this exact target-evidenced Chrome 149
shape and malicious archives whose early symlinks, later descendants, absolute/
parent targets, cycles, duplicates, and size limits prove no outside sentinel is
created; no test reads the real layout cache.

### SG-05.3 CSS schema 1

The CSS driver reads `corpus.toml` schema 1 with exactly these sections:

```toml
schema_version = 1

[source]
kind = "csstree"
repository = "https://github.com/csstree/csstree.git"
revision = "<exact 40- or 64-lowercase-hex Git object id>"
fixture_root = "fixtures/ast"
import_root = "source"
expected_files = 1
expected_cases = 1

[artifacts]
expectation_root = "expectations"
report_file = "expectations/generation-reports/all.json"

[[cases]]
id = "declaration/Declaration.json#/error/0"
status = "unsupported"
reason = "Surgeist intentionally rejects this legacy parser hack."
```

The values in the example illustrate the schema and are not repository facts or
a pin to publish. A future CSS-owned manifest supplies its exact revision and
counts.

`source.kind` must be `csstree`. `repository` must be a nonempty absolute HTTPS
URL ending in `.git`; it is data used for verification, not an acquisition
instruction. `revision` is an exact lowercase 40- or 64-hex object ID.
`fixture_root`, `import_root`, `expectation_root`, and `report_file` are strict
relative paths. `import_root` and `expectation_root` are each exactly one
component, so their complete trees can be swapped from the existing corpus-root
parent without pre-commit parent creation. The report path must equal
`<expectation_root>/generation-reports/all.json`; placing it inside the one
generator-owned expectation publication tree gives a full generation one atomic
root commit. File and case counts are positive and exact.

Manifest semantic validation treats namespace relationships as part of schema 1.
Two paths overlap when they are equal or either is a component-wise ancestor of
the other; string-prefix matches inside one component do not overlap. Exact-text
overlap is rejected during manifest validation, and SG-04 filesystem-equivalent
component overlap is rejected during lease-integrated capability preflight. The
corpus-absolute `import_root` and `expectation_root` must be pairwise
non-overlapping under both comparisons, and neither generated root may overlap
the protected `corpus.toml` manifest path. `report_file` has only the one required
containment above and may not equal/contain `expectation_root` or escape into any
other namespace. Other equal, ancestor, and descendant configurations fail with
`InvalidManifest` before source verification, lease acquisition, directory
creation, or writes.

After `CorpusLocation` construction, the driver also forms the canonical
corpus-absolute coordination namespace
`<corpus-root>/.surgeist-generator/`. Each manifest-declared writable path
must be component-wise disjoint from that coordination namespace. A conflict at
this canonical/text boundary fails with `InvalidPath` before capability
preflight, lease acquisition, or writes; a filesystem-only alias fails during
the cleaned lease-held private preflight before mutation authority is returned or
an import/expectation/report root is written.

`[[cases]]` entries are disposition overrides keyed by a derived case ID. IDs
are unique. An override must resolve to one collected case. Active is the
default for a collected case without an override.

## SG-06 Pinned source and inventory verification

### SG-06.1 Source pin model

`PinnedSource` contains a nonempty label, repository URL, exact `SourceRevision`,
and source subdirectory. `verify_git_source` runs installed `git` read-only and
returns `VerifiedSource` only when:

1. `git rev-parse --is-inside-work-tree` succeeds, including ordinary checkouts
   and linked worktrees;
2. `git cat-file -t <revision>` returns exactly `commit`,
   `git rev-parse --verify <revision>^{commit}` returns exactly that same full
   object ID, and `git rev-parse --verify HEAD^{commit}` returns exactly the
   manifest revision, not merely a prefix. Tree, blob, annotated-tag, and tag-to-
   commit pins are rejected rather than peeled into acceptance;
3. the crate-private raw cleanliness proof below establishes that the index
   equals the exact HEAD tree, every materialized tracked entry equals its index
   blob and type, and no nonignored untracked path exists;
4. the sanitized raw local-config inventory below contains exactly one
   `remote.origin.url` whose unrewritten value equals the manifest repository URL;
5. the requested source subdirectory is a directory inside the canonical
   checkout without a symlink escape.

The raw cleanliness proof never invokes `status`, `diff`, `add`, `hash-object`,
checkout conversion, text conversion, or any clean/smudge/process filter. Using
the sanitized runner, it obtains the exact HEAD inventory with
`git ls-tree -r -z --full-tree <revision>`, the index inventory with
`git ls-files --stage -z`, index visibility flags with `git ls-files -v -z`, and
the nonignored untracked inventory with
`git ls-files --others --exclude-standard -z`. It then enforces all of these
conditions without asking Git to inspect tracked worktree contents:

- HEAD and stage-zero index path, mode, and object-ID inventories are identical;
  unmerged, intent-to-add, sparse-directory, gitlink, and non-UTF-8 entries fail
  `SourceVerification`;
- every visibility record is exactly the ordinary uppercase `H` tag for the
  corresponding index path; skip-worktree, assume-unchanged, unmerged, removed,
  modified-status, or unknown tags fail closed;
- the untracked inventory is empty; repository-owned `.gitignore` and
  `$GIT_DIR/info/exclude` rules remain honored, while the runner overrides
  `core.excludesFile` to the platform null device so no external global exclude
  file participates;
- Rust opens each indexed worktree path directly beneath the canonical worktree
  without following a component symlink. Regular-file bytes must exactly equal
  the original index blob read by sanitized `cat-file`; on Unix the executable
  bit class must also match `100644` versus `100755`. A `120000` entry must be a
  filesystem symlink whose UTF-8 link-target bytes exactly equal its blob.
  Missing, special, mismatched, or unsupported materialization fails
  `SourceVerification`.

This is deliberately stricter than Git's configurable conversion view. A
checkout materialized through EOL, ident, clean, smudge, or process conversion
is rejected when raw bytes differ; no configured program is executed to make it
appear clean. Ignored untracked files and empty directories remain consistent
with normal clean-worktree semantics.

Verification performs no fetch, checkout, remote mutation, configuration change,
or network access. Every Git subprocess used by shared verification or snapshot
construction uses one crate-private sanitized read-only runner. Layout
acquisition uses the separate sanitized bare-fetch runner defined below; neither
path invokes ambient Git configuration.

The runner never resolves `git` or a helper through `PATH`. On the one supported
host it opens and identity-checks the exact `/usr/bin/git` entry before entering
any checkout, corpus, owner, cache, or temporary directory. It must be a
root-owned regular executable whose mode has no group/other write bit; hard-link
count is not an integrity signal for the platform binary. From the trusted `/`
working directory and an otherwise empty environment it invokes only
`/usr/bin/git --exec-path`, requires one absolute canonical directory, and
descriptor-checks every component as root-owned and not group/other writable.
That directory is the trusted Git installation unit: a no-follow recursive
inventory requires every directory, regular entry, and symlink to be root-owned
and not group/other writable, and every symlink target to remain inside the unit.
Any Git-dispatched executable actually used by read-only commands or fetch,
including HTTPS and pack helpers, must resolve to a regular executable inside
that inventory; no helper outside it may execute. The checked canonical
directory is then fixed as `GIT_EXEC_PATH`. Executable/unit identities are
rechecked immediately before and after each spawn. Any mismatch is
`SourceVerification`.

Every subsequent invocation uses the absolute `/usr/bin/git`, trusted working
directory `/`, and either `-C <canonical-checkout>` or an absolute
`--git-dir=<held-stage-path>`; no child process has an untrusted current working
directory. It clears the inherited environment and `PATH` entirely, sets the
validated `GIT_EXEC_PATH`, `LC_ALL=C`, and
`GIT_OPTIONAL_LOCKS=0`, `GIT_NO_REPLACE_OBJECTS=1`,
`GIT_NO_LAZY_FETCH=1`, `GIT_LITERAL_PATHSPECS=1`,
`GIT_TERMINAL_PROMPT=0`, and `GIT_CONFIG_NOSYSTEM=1`. Consequently no inherited
Git directory, worktree, object, alternate, namespace, replacement, config,
credential, transport, prompt, or trace override survives; with no `HOME` or
`XDG_CONFIG_HOME`, no global user config is read. No process-loader or temporary-
path variable is preserved on the supported host.

Every invocation also supplies the equivalent explicit global options
`--no-pager --no-optional-locks --no-replace-objects --no-lazy-fetch
--literal-pathspecs`, followed by `-c core.fsmonitor=false`,
`-c core.untrackedCache=false`, `-c core.attributesFile=<platform-null>`,
`-c core.excludesFile=<platform-null>`, and `-c submodule.recurse=false`, before
the built-in subcommand. The runner permits only the required `rev-parse`,
`ls-tree`, `ls-files`, `cat-file`, and
`config --local --null --list --show-origin --no-includes` operations.
Repository-local configuration remains readable only where Git needs repository
identity, but it cannot enable a filesystem-monitor helper, external attributes
or excludes, optional index update, replacement object, pathspec magic,
submodule recursion, or promisor fetch. Because none of the permitted operations
requests Git worktree conversion, repository-local filter, diff, textconv, and
credential commands are inert. A Git version that does not accept the required
global options fails closed as `SourceVerification`; the driver does not retry
with a weaker invocation.

Before accepting a checkout or bare repository, the runner requires
`rev-parse --show-object-format=storage` to return exactly `sha1` for a 40-byte
pin or `sha256` for a 64-byte pin. A pin length/object-storage-format mismatch is
`SourceVerification` even when another algorithm could spell an object prefix.

The NUL-delimited local-config inventory is read without includes. Every reported
origin must be the already protected canonical common config or per-worktree
config file. Any `include.*`, `includeIf.*`, `url.*`, custom remote-helper,
remote upload/receive-pack, credential helper, `core.sshCommand`, or protocol/ext
configuration fails `SourceVerification`. Exactly one raw
`remote.origin.url` must equal the pin; no `remote get-url` expansion is used.
Filter/diff/textconv entries may remain because the closed read-only command set
never invokes them, and the sentinel tests prove that property.

The acquisition runner begins with the same cleared environment, locale,
no-prompt, no-replacement, no-lazy-fetch, and no-system/global-config baseline.
It additionally clears proxy, askpass, SSH, template, hook, config-include,
worktree, object-directory, alternate-object, and credential environment
variables; sets `GIT_CONFIG_GLOBAL=<platform-null>`; and supplies command-line
configuration that fixes `core.hooksPath` and `init.templateDir` to one verified
empty run-owned directory, empties `credential.helper`, disables HTTP redirects,
and permits only the HTTPS protocol. The manifest repository URL has the SG-03.4
grammar and is passed literally, never through a remote name. There is no SCP,
SSH, `git://`, HTTP, file, ext, or custom remote-helper production path.

Each acquisition starts in a new descriptor-rooted unique Taffy stage and runs
only this closed command sequence:

```text
/usr/bin/git <sanitized-global-options> init --bare --template=<verified-empty-dir> \
    --object-format=<sha1-for-40-bytes|sha256-for-64-bytes> <stage>
/usr/bin/git <sanitized-global-options> --git-dir=<stage> fetch \
    --no-tags --no-auto-maintenance --no-write-fetch-head \
    --recurse-submodules=no --refmap= \
    <literal-https-url> \
    <exact-revision>:refs/surgeist-acquire/<lease-token>
```

Rust reads the newly written bare config without following links and accepts
only Git's known non-executable `core.repositoryformatversion`, `core.filemode`,
`core.bare`, `core.logallrefupdates`, `core.ignorecase`, and
`core.precomposeunicode` keys plus the revision-derived
`extensions.objectformat` value with their expected scalar forms. Any include,
remote, URL, protocol, hook, filter, diff, credential, fsmonitor, extension, or
unknown config is `SourceVerification`; repository config is never reused from a
prior cache. The command-line empty hooks directory suppresses reference-
transaction and every other hook even while fetch writes the private ref.

The fetched ref must resolve to the exact full object ID. Before any tree walk,
the sanitized runner also requires `cat-file -t <revision>` to return exactly
`commit` and `rev-parse --verify <revision>^{commit}` to return the unchanged
full ID; a tree, blob, or tag object is `SourceVerification`. The sanitized
read-only runner then enumerates the pinned commit tree and reads raw blobs directly; no
worktree, index, checkout, submodule, smudge/clean/process filter, LFS program, or
post-checkout hook exists in this acquisition. Only after snapshot verification
does the rooted publication transaction make the complete bare cache visible.
An existing cache is verification input, never an acquisition stage to update in
place.

Focused tests use a crate-private test-only local-file transport capability with
an exact canonical source directory; production has no such branch. With
sentinel system/global/home/local includes, templates, hooks, URL rewrites,
credential/filter helpers, askpass, proxies, and Git environment variables
configured, the local fetch succeeds while no sentinel executes or target is
rewritten. The tests need no network and separately assert the production runner
rejects a file URL and emits the exact HTTPS-only argv/environment.

`VerifiedSource` retains the canonical worktree root and exact revision and
cannot be constructed publicly without verification. It also privately retains
a deduplicated source-protection set resolved by sanitized Git commands:

- the canonical worktree root from `rev-parse --show-toplevel`;
- the canonical per-worktree administrative directory from
  `rev-parse --absolute-git-dir`;
- the canonical common administrative directory from
  `rev-parse --path-format=absolute --git-common-dir`;
- the canonical primary object directory from
  `rev-parse --path-format=absolute --git-path objects`;
- every canonical alternate object directory recursively reachable through each
  local `objects/info/alternates` file, resolving relative entries against the
  object directory that contains the file, deduplicating repeated directory
  identities, and failing a recursion cycle as `SourceVerification`.

Each protection entry must be an existing UTF-8 directory. A missing, malformed,
non-UTF-8, non-directory, or uncanonicalizable administrative/object/alternate
path fails `SourceVerification`. Inherited object and alternate directories
cannot affect this set because the runner cleared them. Worktree administration,
the index, refs, packed objects, and loose objects are therefore all covered for
ordinary repositories, linked worktrees, and local alternates without making
these internal paths public.

Imports never reread fixture bytes from mutable checkout pathnames after this
verification. The shared source module immediately builds an internal immutable
`VerifiedSourceSnapshot` from the pinned commit tree: `git ls-tree -r -z
--full-tree <revision> -- <source-subdirectory>` must enumerate only regular blob
modes beneath the declared subdirectory, and `git cat-file blob <object-id>`
supplies each file's bytes. Paths are normalized and sorted; blob object IDs,
bytes, and SHA-256 digests are retained in memory. Symlink, submodule, tree,
escaped, duplicate, non-UTF-8, and wrong-extension entries fail. The literal
pathspec global option makes metacharacters in `source_subdirectory` ordinary
filename bytes. The no-replacement option binds enumeration and reads to the
commit's original object graph; the no-lazy-fetch option makes a locally absent
promisor object fail `SourceVerification` without contacting its remote or
writing an object. Output paths must have the exact declared subdirectory as a
component prefix before that prefix is stripped. The snapshot type, protection
set, and Git object details are crate-private; public provenance exposes only the
exact source revision, normalized path, and SHA-256 digest.

CSS constructs this snapshot after the clean worktree/origin/HEAD proof and the
read-only existing-path overlap checks, but before the corpus lease and every
writable capability probe. It imports only retained snapshot bytes after the
lease. A checkout content change after snapshot creation cannot alter imported
bytes; a protected-directory identity change is caught by the under-lease
revalidation below. Layout performs any fresh bare fetch as an SG-05 cache-key
transaction before taking a corpus lease, proves the fetched object is a commit,
and builds the same commit-tree snapshot for Taffy import. Expected file and case
counts are checked against the in-memory snapshot before any corpus publication.

Every snapshot consumer validates the complete source-protection set against its
downstream writable namespaces. CSS performs SG-07.2's read-only comparison
before snapshotting and repeats identity/disjointness validation after the lease
has run its journaled probes. Layout checks its planned cache namespace before
cache acquisition; once a bare cache exists, its verified Git/object protection
set must be disjoint from the Taffy import destination, layout artifact/report
roots, browser cache, and both coordination namespaces before snapshot bytes can
feed a corpus publication. It repeats held/reopened identities under the later
corpus lease. A conflict is `InvalidPath`. The exact Taffy cache unit and its
registered private stage are the only source-side namespaces layout may mutate.

The layout `import-taffy` command remains explicitly acquisition-capable through
the SG-10 cache phase, but uses the closed bare-fetch sequence above instead of
the copied checkout workflow, then passes raw snapshot bytes through this
verifier before the corpus phase. No test or verification gate contacts the real
remote or invokes the real acquisition path.

CSS `import-csstree` never acquires a source: it requires
`--source-root <path>` and verifies that user-supplied checkout.

### SG-06.2 Deterministic collection

Shared collection walks regular files recursively without following directory or
file symlinks. It rejects non-UTF-8 names, special filesystem entries, root
escapes, and duplicate normalized paths. Callers supply the allowed extension.
Results are sorted by `RelativePath`; filesystem enumeration order never affects
artifacts or reports.

Layout HTML and XML collection remains lexicographically deterministic. CSS
import collects only JSON beneath the verified `fixture_root`; generation
collects only JSON beneath the corpus-owned `import_root`. Exact manifest counts
are checked before mutation.

## SG-07 Case dispositions and neutral CSS expectations

### SG-07.1 Disposition model

`CaseDisposition` is the closed set `Active`, `ExpectedFail`, `Unsupported`, and
`Quarantined`. `CaseDispositionRecord` couples a unique normalized case ID and
source path to one disposition and optional reason.

The invariant is:

- `Active` has no reason;
- every non-active disposition has a nonempty trimmed reason;
- duplicate case IDs fail shared validation, while repeated source paths are
  valid when distinct case IDs identify multiple cases in one fixture;
- `FailedToGenerate` is a runtime report outcome, not a manifest disposition.

Layout semantics remain: active cases generate normally; expected-fail cases run
and are accounted separately; unsupported and quarantined cases do not run and
their old outputs are removed only during a successful full run. CSS applies the
same accounting to derived neutral cases. Layout's schema-2 validator retains
its domain-specific unique source-fixture rule; CSS intentionally permits any
number of ordinary and error-array case IDs to reference one imported JSON file.

### SG-07.2 CSSTree ingestion

`import-csstree` has one closed phase order:

1. validate CLI/request, `CorpusLocation`, manifest, filter, and source-root text;
2. apply the read-only compile-target support check;
3. verify the user-supplied Git checkout and resolve its complete protection set;
4. perform text, canonical, and descriptor-ancestry overlap checks for every
   existing protected/writable path and nearest existing writable parent;
5. build and validate the immutable in-memory commit-tree JSON snapshot;
6. enter corpus-domain lease acquisition, lock its mutex, recover its own
   journals, and run the journaled actual-parent alias/mount/rename capability
   probes;
7. while that mutex is held but before the lease is returned, reopen every protected directory without following links,
   require its recorded device/inode/fsid identity, and repeat the complete
   disjointness matrix against the probed writable descriptors; and
8. finish lease acquisition and atomically mirror only the retained snapshot bytes beneath the CSS-owned
   `import_root`.

Steps 1 through 5 are read-only. No private probe, coordination bootstrap, lease,
import path, expectation, or report is created before step 6. The driver never
rereads checkout fixture or object bytes after step 5. Content-only source
changes therefore cannot change the import; a path/directory identity swap fails
step 7 with `InvalidPath`. The import preserves relative paths, rejects all
non-JSON and special snapshot entries, checks the exact source-file count, and
removes stale imported JSON only as part of a successful complete transaction.
It writes no expectations or generation report.

Source verification first yields the complete crate-private SG-06 protection
set, not only the canonical checkout root. Before the commit-tree snapshot is
built, every protected worktree, per-worktree Git directory, common Git
directory, primary object directory, and recursive local alternate object
directory must be component-wise disjoint in both directions from every
prospective CSS mutation namespace: the absolute import root, expectation root,
report path, and corpus coordination root. Any equality, protected-ancestor, or
protected-descendant relationship fails with `InvalidPath` without a write. The comparison uses
canonical protected directories and owner/corpus roots plus checked relative-path
joins. It also compares descriptor `(device, inode)` ancestry sets for every
existing protected and nearest-existing writable directory in both directions;
this detects case aliases, bind/null mounts, firmlinks, and other pathname aliases
that canonical text does not collapse. SG-04 exact-pair probes run independently
in each actual parent needed by a not-yet-created writable suffix. Thus an external ordinary or
linked checkout and every object store it uses remain strictly read-only even
when the user supplies an aliased owner or corpus nested within one of those
locations. The only checks that need creating candidate spellings or exercising
filesystem flags are the lease-held, journaled probes in steps 6 and 7.

### SG-07.3 Neutral expectation shape

For each imported JSON file, CSS generation writes one expectation JSON at the
same relative path beneath `expectation_root`. Its object has:

- `schema_version: 1`;
- `generator: "surgeist-css-generate"`;
- `source`, the corpus-relative imported JSON path;
- `source_sha256` and exact `source_revision`;
- `cases`, sorted by derived case ID.

Each case has:

- `id`, formed as `<fixture-relative-json>#/<JSON-pointer-escaped-label>` for an
  ordinary case or `<fixture-relative-json>#/error/<zero-based-index>` for an
  entry in the top-level `error` array;
- `context`, the first fixture-relative path component;
- `label`, omitted only for indexed error entries;
- `input`, copied from the fixture's `source` string;
- `options`, preserved only when it is a JSON object;
- `upstream_outcome`, either `parsed` for an ordinary object containing an `ast`
  or `rejected` for an `error` entry;
- `canonical_css`, copied only from an optional string `generate` field;
- `status` and optional `reason` after manifest override resolution.

JSON Pointer escaping replaces `~` with `~0` and `/` with `~1`. The driver does
not copy AST values, CSSTree error messages, offsets, comments, or recovery ASTs.
Those are upstream implementation details, not Surgeist expectations.

Preserved `options` are recursively canonicalized: object keys use decoded
Unicode scalar lexicographic order at every depth, arrays retain source order,
strings retain decoded values, and booleans/null/numbers use Serde JSON's one
deterministic serialization. Source member order and insignificant whitespace
therefore cannot change expectation bytes.

Before typed interpretation, a crate-private streaming JSON visitor consumes the
entire raw byte slice and rejects a duplicate member name in every object at any
depth. This applies to top-level fixture labels, ordinary-case members such as
`source`, `options`, `generate`, and `ast`, each object inside `error`, and nested
`options` objects. Member identity is the decoded JSON string, so escaped and
literal spellings of the same key collide. The visitor rejects trailing values
and malformed UTF-8/JSON and only then permits the typed CSSTree conversion; no
last-member-wins parser behavior is observable. Generated CSS expectations and
reports use the same duplicate-rejecting reader during offline verification.

Malformed top-level shapes, duplicate object members, nonobject ordinary cases,
missing/nonstring `source`, ordinary cases without `ast`, nonarray `error`,
nonobject error entries, invalid `options`, invalid `generate`, duplicate derived
IDs, and unmatched manifest overrides fail `InvalidInventory` before any
expectation or report write. During full or filtered generation, each imported
fixture JSON must derive at least one ordinary or error case; a syntactically
valid zero-case object is also `InvalidInventory` (and the persisted equivalent
is `Verification` during checking). Import itself validates JSON bytes/counts but
does not interpret this shape. Consequently every CSS expectation report artifact has the
positive `case_count` required by `ReportArtifact::new`; no empty expectation
file/report-artifact exception exists.

## SG-08 Hashes and provenance

`Sha256Digest` stores exactly 64 lowercase hexadecimal characters and is created
from bytes or checked text. Shared helpers hash files without lossy conversion.
Every generated artifact records its source path, source digest, generator name,
domain schema version, and domain provenance. Reports record artifact digests so
offline checks detect both source and output drift.

Layout preserves the current XML generated-by comment and schema-2 report
metadata, including source, linked-resource, helper, base-style, browser,
launch-profile, manifest, and Taffy revision hashes. Equivalent corpus bytes and
browser measurements must render byte-identical XML and report JSON.

CSS expectation JSON uses pretty serialization with two-space Serde formatting
and exactly one final newline. Its full report schema 1 records:

- manifest digest, source repository/revision, and generator identity;
- filter `null` for the canonical full report;
- sorted artifact entries with source path/digest, output path/digest, and case
  count;
- active, expected-fail, unsupported, quarantined, and failed-to-generate case
  counts and sorted case records with reasons where required.

Report validation recomputes every digest and count. Generic `skipped` buckets
are rejected. The shared `GenerationReport` validates structural counts without
assuming every counted case has an artifact; layout and CSS validators enforce
their distinct schema relationships described above.

## SG-09 Atomic installation and stale-output behavior

`ArtifactPlan` owns the supplied `CorpusLocation`'s canonical corpus-root
identity, validated domain, a `BTreeMap` of unique corpus-relative output paths to
bytes, and the exact retained output inventory for a full run. Construction
hashes content and rejects path collisions before touching disk. There is no
public plan rooted at an independently supplied path, and internal installation
requires its matching live lease.

The only mutation target claimed and verified in this cycle is
`aarch64-apple-darwin`. On that target an internal rooted-filesystem capability
opens either the plan's bound corpus root under its live domain lease or the
canonical owner-target/cache-unit parent under its matching live cache-key guard.
It performs every traversal, create, open, rename, sync, and remove relative to
held directory descriptors with non-following safe `rustix` operations. It opens
each component separately, refuses symbolic links, and compares the opened root
identity with the authority's recorded canonical path before adoption. There is
no generic "owner-cache root" capability and no independently supplied writable
root. Pathname validation alone is never mutation authority and no OS descriptor
is public.

Every opened or newly created component must have both the root's
`fstat().st_dev` and `fstatfs().f_fsid`; a missing value or change is
`UnsupportedPlatform`. Newly created directories are reopened and checked before
use. Collection, clone, and stale walks do not enter a mount point, and
source/writable aliases use descriptor ancestry plus the per-parent SG-04 proof.
This prevents a held corpus/cache descriptor from authorizing a mounted or
firmlinked alias to another tree.

Every other target is outside this cycle's mutation and binary-support claim.
When its default shared core is built, each mutation-capable entry point returns
`UnsupportedPlatform` before coordination, cache, import, artifact, or report
writes; read-only value, manifest, inventory, hash, provenance, and corpus logic
remains available. The already-installed `wasm32-unknown-unknown` standard
library provides the explicit nonmutation compile check in SG-13.3. Linux and
other native driver support are a bounded future handoff, not an unverified
promise in this candidate.

Supported macOS requires runtime `renameatx_np` `RENAME_EXCL` and `RENAME_SWAP`.
While the applicable coordination gate and exclusive corpus-domain or cache-key
mutex are held and before that mutation authority is returned, a descriptor-
rooted journaled probe beneath that authority's coordination root exercises both
flags, syncs the parent, removes only its verified objects, and syncs again.
`ENOSYS`, `EINVAL`, `EOPNOTSUPP`, an identity change, or uncleanable probe is
`UnsupportedPlatform`; no cache/import/artifact/report mutation follows. Lease
or cache-guard acquisition records successful recovery/probing, so internal plan
installation rejects unmatched authority rather than probing outside exclusivity.

The transaction namespace has an explicit exclusivity contract. The publication
and coordination roots are generator-managed for the duration of a live lease;
cooperating generator processes obey it. Pre-existing names, hard links,
symlinks, special entries, mounts, and unjournaled residue are untrusted and
rejected. A non-cooperating same-user process that changes those namespaces while
leased is outside the supported contract, while descriptor rooting still
prevents escape or overwrite of an unexpected object.

### SG-09.1 One-root publication unit

Each transaction has exactly one final publication root. Layout generation uses
`xml`, including `xml/generation-reports`; layout import uses the complete `html`
tree; CSS import uses `import_root`; and CSS generation uses `expectation_root`,
including its required report child. A browser version or Taffy revision
directory is likewise one cache transaction unit. A command may finish one cache
transaction and later one corpus transaction, but those are separate journals,
authorities, and commit points and are never simultaneously in commit cleanup.

Before creating transaction state, a rooted walk inventories any current unit in
normalized path order. Corpus units accept only ordinary single-link files and
same-mount directories. A browser cache unit additionally accepts exactly SG-
05.2's five validated symlinks; its inventory stores the uninterpreted link
target bytes and their SHA-256 and every recovery operation unlinks them without
following. Taffy cache units contain only the verified bare-Git regular-file/
directory inventory. All policies reject hard links, other symlinks, special
entries, reserved/transaction aliases, and mount crossings.

A clean full corpus run builds only the complete retained state plus explicitly
domain-classified preserved files; an unknown regular file is `InvalidInventory`,
not silently retained or deleted. Layout `xml` classifies only generated XML and
structurally valid generation-report paths (including nonmanifest stale
candidates), layout `html` additionally classifies validated Surgeist-authored
fixtures, and both CSS publication roots classify only their generated JSON. A
filtered or recoverable-diagnostic run clones the validated current tree into its
stage, overlays successful artifacts, and deliberately retains stale/nonmanifest
output. Layout HTML import clones every validated Surgeist-authored fixture byte-
for-byte and replaces or prunes only manifest-classified Taffy files. No path is
mutated in place.

Journal authority exists before an external stage can exist. Under the matching
exclusive corpus-domain mutex or cache-key mutex, construction follows this
exact durable protocol:

The external stage name is derived rather than caller-selected:
`._surgeist-<domain>-stage-<transaction-id>` for a corpus unit and
`._surgeist-cache-stage-<cache-key>-<transaction-id>` for a cache unit. These
ASCII names and the final one-component name are stored in the intent. A cache-
family scaffold can therefore map a stage to exactly one key journal; an
unparseable or unreferenced child is disputed.

1. Inventory the old final and compute its canonical inventory digest or
   `absent`. Cache publication requires `absent`: a valid existing immutable
   version/revision unit is verified and reused without a transaction, while an
   invalid existing unit fails closed and is never replaced or repaired.
2. Exclusively `mkdirat` an `active-<transaction-id>` directory in the applicable
   transaction parent and sync that parent. For corpus this is
   `<corpus>/.surgeist-generator/transactions/<domain>/`; for cache it is
   `<owner-target>/.surgeist-generator-cache/transactions/<cache-key>/`. The
   atomic directory creation is the intent reservation. An empty or partially
   populated active directory after a crash owns no external name and is a
   recoverable internal state, not a publication stage.
3. Inside that active directory, write/sync a complete `intent.json` and
   `old-inventory.json` through unique private temporary names and `RENAME_EXCL`,
   then sync the active directory. The schema-1 intent records coordination,
   root/mount, authority-key, final-parent, final/stage, and transaction-parent
   identities; domain/cache key; publication mode; expected old digest; token;
   and the old-sidecar digest. All recorded relative paths are revalidated on
   read. No stage registration begins unless both immutable files are complete.
4. Create an empty `stage-reservation` directory inside the active directory,
   record and sync its device/inode/fsid and external stage name in an immutable
   `stage-registration.json`, then atomically move that exact empty directory to
   the final root's held parent with `RENAME_EXCL`. Sync both parents before
   populating it. Thus a crash leaves the reservation internally, externally
   under its recorded identity, or absent before population; it can never leave
   populated external state without durable intent and registration evidence.
5. Populate only that registered stage descriptor. Construction writes are
   exclusive, descriptor-relative, same-mount, and no-follow. An in-progress
   stage may contain only the domain's allowed types/modes; its registered root
   identity is sufficient for fail-closed descriptor cleanup under the SG-09
   cooperating-namespace contract. Chrome creates its five prevalidated links
   last; no later write traverses them.
6. Sync the completed tree bottom-up, reopen it, and publish immutable
   `new-inventory.json` and `prepared.json` files. The new sidecar records each
   normalized path, type, mode, device/inode identity, link count, length/content
   digest for regular files, and target bytes/digest for an allowed symlink.
   `prepared.json` binds the old/new sidecar digests and the complete new-tree
   digest. No commit is attempted before the prepared marker is durable.

Every metadata/marker/receipt publication uses a unique regular single-link
temporary file inside the active directory, flush/sync, `RENAME_EXCL` to its one
fixed final name, and an active-directory sync. A crash-partial temporary file
therefore remains inside already-reserved intent state; recovery never interprets
it as a final marker. Unknown names or wrong identities/types are disputed.

On the supported target, every lease/stage/profile/transaction token is an exact
16-byte read from `/dev/urandom`, rendered as 32 lowercase hex bytes. Failure to
open/read the device is `UnsupportedPlatform`; there is no clock/PID fallback.
An exclusive-create collision retries with a fresh token at most 16 times, then
fails without adopting the colliding name.

The commit operation depends on the unit policy:

- an absent corpus final or every cache unit publishes the complete stage with
  `RENAME_EXCL`;
- a present corpus final atomically exchanges the complete stage and final root
  with `RENAME_SWAP`.

The held parent is then synced. At no instant does a cooperating reader observe a
partially renamed file set: it sees the complete old tree or complete new tree.
For a corpus exchange, the stage name now designates the complete old tree. The
transaction publishes and syncs an immutable `committed` marker only after the
final path is reopened and its digest equals `prepared.json`'s new-tree digest.

### SG-09.2 Recovery and cleanup states

Corpus lease acquisition recovers only that corpus/domain's journals in
transaction-ID order while holding its exclusive mutex. Cache acquisition
recovers only the exact cache key's journals while holding that key exclusively,
before any corpus lease is requested. All corpora using the same canonical
owner-target identity and final relative cache-unit path therefore converge on
the same recovery authority. A read-only check or other shared cache guard never
recovers; it reports any unresolved cache journal as `Verification`.

Recovery opens all names descriptor-relatively, validates the intent's recorded
coordination and authority identities against its currently held authority, and
recomputes inventories. It follows this closed state machine; `old` includes the
cache-required absent state:

| Observed durable state | Classification and action |
| --- | --- |
| active directory empty or containing only recognized partial metadata temporaries; no complete intent | abandoned internal intent; because registration was forbidden, remove only the active directory's verified internal entries |
| complete intent/old sidecar; registration absent or its exact reservation remains internal; final = old | unregistered intent; remove only internal reservation/metadata |
| complete registration; internal reservation absent; external registered stage is absent or a construction-policy subset; no durable `prepared`; final = old | interrupted construction; publish `aborted`, keep final, descriptor-clean the recorded stage if present |
| durable `prepared`; final = old; external stage = complete new; no outcome marker | prepared pre-commit; publish `aborted`, keep final, clean stage |
| durable `prepared`; final = complete new; external stage = complete old or absent; no outcome marker | commit occurred; publish `committed`, keep final, clean old stage if present |
| final = old; external stage = permitted new-stage subset or absent; durable `aborted` | aborted cleanup; keep final, continue stage cleanup |
| final = complete new; external stage = old-stage subset or absent; durable `committed` | committed cleanup; keep final, continue old-stage cleanup |
| active journal has valid `cleanup-complete`; terminal final digest matches and external stage is absent | rename active journal to its completed name and continue metadata cleanup |
| completed journal has valid receipt and remaining members are an exact receipt-listed subset | remove remaining metadata in receipt order, with receipt last |
| completed journal is empty | descriptor-`rmdir` it and sync the transaction parent |
| internal and external registration both exist, a prepared tree is incomplete, an outcome conflicts with final digest, or any identity/type/name/digest differs | disputed; return `ArtifactTransaction` without removing or renaming final or disputed external state |

For an interrupted construction without a new sidecar, cleanup first rechecks
the registered stage-root identity and permits only same-mount entries allowed by
that domain's construction policy; it never follows a link, crosses a mount, or
removes a hard-linked/special entry. After `prepared`, removal is descriptor-
rooted and postorder and each remaining entry's mount, identity, type, mode, link
count, length/content digest, or allowed symlink target is rechecked against the
applicable sidecar immediately before unlink. Once an `aborted` or `committed`
marker is durable, a crash-partially-deleted stage is accepted only when its
remaining names and identities are an exact subset of that sidecar. A mismatch
leaves the residue and journal untouched.

After the stage is gone, recovery publishes a synced immutable `cleanup-complete`
receipt that binds the terminal outcome/final digest and the exact immutable
journal-member inventory/removal order, renames the validated `active-<id>` directory with `RENAME_EXCL` to its
unique `completed-<id>` name, and syncs the transactions parent. Completed-state
cleanup removes only the closed set of verified sidecars, markers, and recognized
partial metadata temporaries while retaining that receipt until last. A crash
therefore leaves a completed directory with its valid receipt or an empty one;
recovery continues the former and removes the latter only with descriptor-
relative `rmdir`, which fails if any entry appeared. Any receipt-less nonempty or
identity-mismatched completed directory is disputed. The parent is synced after
final `rmdir`. An external stage is removable only through its durable
registration; an unregistered stage or completed name is a collision, never a
cleanup candidate.

A construction or pre-commit failure publishes `aborted`, leaves the old final
unchanged, and runs the same journaled cleanup. Inability to complete cleanup
returns `ArtifactTransaction`; the next matching authority resumes it. Process
death at any intent, sidecar, registration, stage-population, prepared-marker,
swap, outcome-marker, or completed-directory transition is therefore one of the
states above. Death before commit leaves the old final; death after commit leaves
the new final. Recovery distinguishes them by full-tree digest even if death
occurred before the committed marker write.

The swap is the irreversible commit point. A failure to sync, mark, or delete the
old stage after it returns successfully does **not** attempt an impossible
rollback: the complete new final remains visible, the durable journal identifies
it as committed, and the call returns `ArtifactTransaction`. Such a result is not
final verification evidence; read-only checks report the unresolved journal as
`Verification`, and the next mutation lease retries cleanup. Once post-commit
cleanup succeeds, the same coherent new final remains and the journal disappears.

Every internal artifact plan rejects any component filesystem-equivalent to the
reserved coordination or transaction names in a generated root, artifact,
retained path, or report path at any depth. Exact spellings fail construction; SG-04 probes fail
aliases before staging. Together with its location-bound root, callers cannot
select, contain, or reach current or nested coordination state. Stale selection
is limited to the declared generated extension and full retained inventory; root
cloning preserves every other authorized file.

### SG-09.3 Domain outcomes

Outcomes are classified before staging:

- A clean full success has no `failed_to_generate` cases. It atomically installs
  the complete artifact set, prunes stale generated artifacts from the complete
  retained inventory, installs the canonical report, prunes nonmanifest reports,
  and returns `Ok(())`.
- A recoverable layout job failure is a page/measurement/batch failure assigned
  to one or more validated case IDs after acquisition succeeded and terminal
  browser/task/profile cleanup itself succeeded. Successful case bytes are
  installed as one coherent group. A full run atomically writes a canonical
  diagnostic report containing those artifacts and the exact
  `failed_to_generate` cases/reasons, preserves every stale artifact and
  nonmanifest report, then returns `Generation`. A filtered run may install only
  successful matched artifacts, writes/prunes no report or stale output, and
  returns `Generation`. This diagnostic state is not final verification evidence.
- A fatal lifecycle failure is any manifest/source/capability/lease/acquisition,
  invariant/serialization, unassigned generation, browser/task/profile cleanup,
  or pre-commit publication failure. It publishes no artifact or report and
  removes no stale output. It never emits a diagnostic report merely because the
  fatal path failed. A post-commit cleanup failure is the explicit SG-09.2
  exception: it returns `ArtifactTransaction` with the complete new tree and
  recovery journal intact, never a mixed tree.

CSS has no recoverable per-case execution in this cycle: malformed or
unconvertible neutral input is a fatal pre-publication error. Layout's
recoverable behavior preserves the copied generator's externally useful
partial-success/failed-case report contract, but deliberately buffers successful
XML until resource cleanup instead of writing it incrementally.

## SG-10 Generation leases and lifecycle

Both cache and corpus coordination use standard-library advisory file locks and
the descriptor-rooted filesystem capability in SG-09. Lock acquisition is
nonblocking. The global order is: cache target gate (released after key
acquisition), required cache keys in normalized-key order, corpus gate (released
after domain acquisition), corpus domain mutex, then browser/task resources.
Release is reverse; no code takes a cache lock while holding a corpus mutex.

Before the first gate exists, only the exact persistent `bootstrap` and
`bootstrap/locks` directories may be created inside an already validated
coordination root. Each component uses exclusive `mkdirat`, reopen, identity/
mount/exact-name checks, and parent sync; concurrent `EEXIST` adopts only the
same validated directory. A crash may leave either empty prefix, which is
reusable scaffolding rather than an unjournaled stage.

Lock-file bootstrap itself is journaled. Before creating a lock-file stage, a
bootstrapper exclusively creates/syncs
`bootstrap/locks/active-<decimal-pid>-<token>/` beneath the applicable
coordination root; the exact name is the first durable liveness/ownership record
even if death precedes `intent.json`. Recovery probes that PID without signalling
it and never touches the directory while the PID exists or the probe is
inconclusive; PID reuse is conservatively live. Once `ESRCH` proves the creating
process is absent, only recognized internal bootstrap residue may be cleaned.
The bootstrapper publishes `intent.json`, creates the empty regular stage inside
that intent, publishes its identity in `stage-created`, writes/syncs the exact
header, and acquires the advisory lock on that still-open stage. It then attempts
`RENAME_EXCL` directly from the intent to the exact final lock name and syncs both
directories. A winner keeps that same descriptor; on `EEXIST`, a loser validates
and locks the complete final inode before journal-cleaning its own stage. A live
bootstrap intent's stage lock is never cleaned. Recovery of an unlocked intent
accepts only: final absent plus the registered internal stage (abort); a valid
matching final plus absent stage (winner cleanup); or a valid final plus the
registered loser stage (loser cleanup). Other identities are disputed. Intent
cleanup uses SG-09.2's receipt/tombstone order, so no unpublished unjournaled lock
stage exists. Final lock files are exact-name, regular, single-link, immutable,
never truncated, and rejected on an unknown header or identity change.

Cache users first open
`<canonical-owner-target>/.surgeist-generator-cache/acquisition.lock` with the
header in SG-05.2. Mutation-capable users take it exclusive, bootstrap/open all
needed `locks/<key>.lock` files, and try their exclusive key locks in normalized
order before releasing the gate. They then recover only those keys' SG-09
transactions and runtime/run intents and perform cache-parent alias/mount/rename
probes. Read-only checks create nothing: when coordination exists they take the
gate and relevant key locks shared, fail `Verification` on any journal/run
intent, and hold the key guard through cache inspection; when it is absent they
require it and the inspected final identities to remain absent/unchanged at a
final recheck. A cache contender returns `LeaseActive` with canonical target and
key only. Cache coordination has no owner record.

`GenerationLease` is corpus-only. The fixed coordination domain is `layout` for
every layout import/generate/check command and `css` for every CSS
import/generate/check command. Mutating commands take their domain mutex
exclusive; checks take it shared. Therefore import, full/filtered generation,
and checking cannot race within one domain/corpus. Layout and CSS may use distinct
domains in a deliberately shared corpus; distinct corpora are independent.

The corpus gate and mutex live beneath
`<canonical-corpus-root>/.surgeist-generator/`: `acquisition.lock`,
`leases/<domain>/mutex.lock`, `leases/<domain>/owner.json`, and
`transactions/<domain>/`. Gate and mutex contents are exactly
`surgeist-generator-lock-v1\n`. Their placement depends only on canonical corpus
and fixed domain, so distinct valid owner ancestors converge. Existing
coordination is adopted only with the exact on-disk spelling, expected
descriptor identity/mount, valid immutable lock headers, and no unknown entry.

A mutator takes the corpus gate exclusive. While it is held, missing `leases`,
`leases/<domain>`, `transactions`,
and `transactions/<domain>` parents are created/adopted as exact persistent
scaffolds with the same reopen/sync checks; a crash may leave only validated
empty prefixes. It then bootstraps/opens the mutex and tries it exclusive.
Contention returns `LeaseActive` with only domain and canonical corpus; it never
reads or presents `owner.json` as the current holder.
After locking, acquisition recovers only coordination-bootstrap/probe journals,
the domain's owner-record journal, and that domain's corpus transactions—never a
cache journal. It then runs actual-parent namespace/mount/rename probes and final
protected-source identity/disjointness revalidation. A complete probe is removed
only by recorded descriptor identities; partial cleanup resumes from its
sidecar. A dispute releases the locks and returns `ArtifactTransaction`.

`owner.json` is historical audit metadata for the last successfully acquired
lease, not liveness evidence. It contains generator, PID, owner/corpus roots,
scope, command, and Unix start time. Replacement is the single-regular-file
specialization of SG-09's exact `intent -> stage-registration -> prepared ->
RENAME_EXCL/RENAME_SWAP -> committed/aborted -> cleanup-complete` protocol; the
intent is durable before the file stage exists. While holding the gate,
acquisition completes that journal, installs the new owner, then releases the
gate and returns the mutex-backed lease. Checks never update owner metadata.
Dropping a lease releases only its held mutex. No lease is returned while an
owner/probe/corpus journal is unresolved.

On every target other than `aarch64-apple-darwin`, a mutation-capable operation
returns `UnsupportedPlatform` during the static read-only prefix, before
coordination or source subprocesses. On the supported target, exact-text,
canonical, and existing descriptor-ancestry conflicts fail before lock
acquisition. Filesystem-only case/normalization aliases, runtime rename/mount
capability failures, and a protected-directory identity change found by final
revalidation fail while the exclusive mutex is held but before a lease is
returned or any cache/import/artifact/report root is mutated. Only exact
coordination bootstrap plus a cleaned or durably journaled private probe may
remain.

Layout's synchronous worker owns one private resource registry. Thread-spawn and
Tokio-runtime-build failure occurs before cache/corpus authority and maps to
`Generation`. After supervisor construction, the registry lives outside the
panic-contained operation future. A browser child, page, CDP/stderr task `JoinHandle`,
profile/run intent, and generated bytes are registered immediately; no task
handle is dropped. Normal completion, semantic error, and contained operation
panic use this exact monotonic-clock terminal sequence:

1. Each page receives one close attempt with a two-second `tokio::time::timeout`.
   Failure/timeout is recorded; that page is not retried.
2. Browser close receives one five-second attempt. Whether it succeeds or fails,
   the owned child then receives one five-second wait. If not reaped, the
   supervisor sends kill to the recorded process group exactly once and performs
   an intentionally unbounded blocking wait for the owned child plus process-
   group `ESRCH`. A browser is terminal only when wait returns its exit status
   and no member of its group remains; there is no return/detach deadline after
   kill.
3. Each registered CDP/stderr task receives one five-second normal join interval. If still pending,
   abort is requested exactly once and cancellation join receives five seconds.
   If still pending, the supervisor keeps the handle and waits without a terminal
   deadline until it resolves completed, cancelled, or panicked. Abort request
   alone is not terminal. The public call may therefore remain blocked forever
   for a noncooperative task rather than detach it.
4. Only after child reap publishes the run intent's `reaped` proof does rooted
   profile cleanup run once. Success completes the run journal. An identity-
   matching removal failure keeps the durable pending intent/profile and returns
   `Generation`; a disputed identity returns `ArtifactTransaction` without
   removal. Cache transaction stages are handled only by SG-09 recovery, not an
   ad-hoc recursive cleanup.
5. The corpus lease is released only after every child/task is terminal and the
   profile is removed or durably pending; the cache guard is released afterward.

Timeouts use Tokio's clock and focused tests use a paused injected clock; retry
counts and the unbounded forced states above are not configurable. Layout buffers
all XML/report bytes in memory and begins the synchronous corpus transaction only
after browser/task cleanup and profile terminal accounting. Fatal launch,
handler, unassigned measurement, profile, or pre-commit errors leave the old
artifact/report/stale tree. Only SG-09.3's case-assigned recoverable outcome may
publish diagnostics. Once publication begins there is no async cancellation
point; post-commit cleanup follows SG-09.2.

Each cleanup await is separately panic-contained. A close panic falls through to
the same direct kill-and-unbounded-wait path; a handler panic is a resolved
terminal join outcome and is error-accounted. Cleanup contains no assertion,
`unwrap`, or `expect` over external state. A contained operation panic returns
`Generation` only after the same terminal protocol. If the outer supervisor
itself nevertheless panics, `layout::run` joins and resumes that panic; it does
not misreport cleanup-complete `Generation`. Chromiumoxide drop behavior is only
last-resort unwind protection, not the claimed lifecycle.

Mutation lifecycles are command-specific and closed:

- Layout validates request/location/manifest/filter and the static target, then
  takes required cache guard(s), recovers/acquires or verifies each immutable
  cache unit, and builds any Taffy snapshot. Only after cache transaction commit
  does it take the corpus lease, repeat cache/source identity disjointness, create
  a browser run/profile when needed, generate/clean resources, and perform one
  corpus transaction. Release is corpus then cache. A cache remains durable if a
  later corpus phase fails.
- `import-csstree` follows SG-07.2's eight phases. The snapshot precedes the
  exclusive `css` mutex; probe/source-identity failures after mutex acquisition
  publish no corpus unit.
- CSS `generate` may use a read-only preliminary inventory only to reject an
  obvious filter miss. After taking the exclusive `css` mutex it authoritatively
  reopens/recollects the complete import tree, rechecks `expected_files` and the
  filter, duplicate-checks and interprets every JSON file, requires at least one
  derived case per file, applies every override, and requires the complete case
  count to equal `expected_cases`. Full and filtered generation validate the same
  complete inventory; filtering selects outputs only afterward. A mismatch is
  `InvalidInventory` before transaction intent.
- Layout/CSS checks take shared locks in global order and remain read-only.
  Persisted corpus defects map to `Verification`; `check-taffy-corpus` takes its
  cache key shared before its corpus-domain shared lock.

`import-csstree` validates the pinned Git/JSON snapshot and exact file count but
does not interpret CSSTree cases or enforce `expected_cases`; that belongs to
generation and offline corpus verification. Generation and install errors retain
failure provenance and follow SG-09's exact construction, commit, recovery, and
cleanup states.

## SG-11 Domain commands and thin binaries

### SG-11.1 Layout interface

The layout binary syntax is:

```text
surgeist-layout-generate --owner-root <path> --corpus-root <path> \
  <generate|generate-existing|check-corpus|check-taffy-corpus|import-taffy> \
  [--browser-path <owner-relative-path>] [--filter <corpus-html-relative-path>]
```

`generate` rejects `--browser-path` and `--filter`, resolves the managed manifest
browser pin, and is acquisition-capable. `generate-existing` requires a nonempty
owner-relative `--browser-path` under the manifest cache, verifies executable type
and exact `--version`, and alone accepts `--filter`. Check commands reject browser
and filter arguments and perform no browser resolution. `import-taffy` rejects
browser and filter arguments and retains its explicit acquisition-capable behavior.

Environment variables formerly used for roots, filters, browser paths, cache, or
version are neither read nor accepted as overrides. Browser cache and version are
manifest-owned. A future layout wiring change supplies the explicit arguments.

### SG-11.2 CSS interface

The CSS binary syntax is:

```text
surgeist-css-generate --owner-root <path> --corpus-root <path> \
  <import-csstree|generate|check-corpus> \
  [--source-root <path>] [--filter <import-relative-json-or-prefix>]
```

`import-csstree` requires `--source-root` and rejects `--filter`. `generate`
rejects `--source-root` and optionally accepts a validated filter. `check-corpus`
rejects both optional arguments and performs no process execution or network
access.

### SG-11.3 Binary boundary

Each binary file is at most fifteen physical lines. It selects its feature-gated
library driver, prints `surgeist-*-generate: <error>` once on failure, and exits
only through `GeneratorError::exit_code()`. It contains no manifest, path, source,
generation, artifact, or report behavior.

## SG-12 Error model

`GeneratorError` is a public struct with private data, a nonexhaustive semantic
`GeneratorErrorKind`, `Display`, `std::error::Error`, `kind()`, and `exit_code()`.
Kinds are:

- `Cli`;
- `InvalidPath`;
- `InvalidManifest`;
- `InvalidInventory`;
- `SourceVerification`;
- `UnsupportedPlatform`;
- `LeaseActive`;
- `Process`;
- `Io`;
- `ArtifactTransaction`;
- `Generation`;
- `Verification`.

Diagnostics identify the operation and relevant normalized or display path.
I/O errors preserve their safe source. Process errors record program, status,
stdout, and stderr without panicking. External input returns an error rather than
asserting. Assertions remain only for internal states already made unrepresentable
by a checked constructor.

`GeneratorError::exit_code() -> u8` has this exhaustive mapping:

| Kind | Exit code |
| --- | --- |
| `Cli` | 64 |
| `InvalidPath` | 1 |
| `InvalidManifest` | 1 |
| `InvalidInventory` | 1 |
| `SourceVerification` | 1 |
| `UnsupportedPlatform` | 1 |
| `LeaseActive` | 1 |
| `Process` | 1 |
| `Io` | 1 |
| `ArtifactTransaction` | 1 |
| `Generation` | 1 |
| `Verification` | 1 |

No binary remaps a semantic error independently.

| Failure | Required kind | Mutation rule |
| --- | --- | --- |
| Missing/duplicate/unknown CLI argument | `Cli` | No filesystem mutation |
| Root, relative path, symlink/mount escape, or textual/filesystem-equivalent protected/writable namespace overlap | `InvalidPath` | No domain mutation; private probes cleaned, exact coordination bootstrap may remain |
| TOML parse, version, or field contract | `InvalidManifest` | No corpus mutation |
| Count, duplicate, unmatched override, or malformed CSSTree case | `InvalidInventory` | No artifact/report mutation |
| Wrong/dirty Git source or origin, malformed Git storage, unsupported sanitized Git invocation, or missing promised object | `SourceVerification` | No import mutation |
| Non-Apple-Silicon-macOS mutation request, or missing required rename, mount-identity, or name-equivalence capability | `UnsupportedPlatform` | No cache/import/artifact/report mutation; supported-host probes cleaned and exact coordination bootstrap may remain |
| Contended generation lease | `LeaseActive` | No corpus mutation |
| Git/browser subprocess failure | `Process` | Domain cleanup; no stale pruning |
| Read/write/canonicalize failure | `Io` | Pre-commit final unchanged; post-commit state follows journal |
| Journal/stage/swap/recovery/cleanup failure | `ArtifactTransaction` | Pre-commit final unchanged, or post-commit complete new final plus durable recovery journal |
| Recoverable case-assigned layout page/measurement/batch failure after successful resource cleanup | `Generation` | SG-09 diagnostic publication; no stale/nonmanifest pruning |
| Unassigned layout generation/resource cleanup or CSS neutral conversion failure | `Generation` | No publication commit; final artifact/report/stale state unchanged |
| Offline hash/provenance/report mismatch or unresolved probe/owner/transaction journal | `Verification` | Check commands remain read-only |

## SG-13 Verification behavior

### SG-13.1 Offline checks

Layout `check-corpus` remains browser-free and validates the schema-2 manifest,
helper-only asset directory, case/source inventory, report inventory and counts,
XML inventory, and every provenance hash. `check-taffy-corpus` verifies the
manifest-derived bare object cache and imported baseline without network access.

CSS `check-corpus` reads only the CSS-owned corpus. It validates schema 1, source
and expectation inventories, exact counts, derived case IDs, disposition reasons,
source-to-expectation one-to-one paths, expectation provenance, report inventory,
all source/artifact hashes, case/report counts, and absence of stale generated
JSON. It never requires the original CSSTree checkout after import.

All check commands are read-only. They also inspect their domain's coordination
inventory and fail on any unresolved probe, owner, transaction, or completed-
tombstone state; they never recover it. A failing check reports the first
deterministic violation and writes nothing.

When a check inspects a browser/Taffy cache it first applies SG-10's read-only
target gate/cache-key protocol, then the corpus shared-lock protocol below. It
fails on any cache transaction, run intent, or completed tombstone and never
creates/recovers cache coordination. Locks are released in reverse order.

For a pre-existing exact coordination namespace, a check opens the immutable
gate/mutex files read-only, validates their headers/identities, briefly takes the
gate's shared advisory lock, then takes and holds the domain mutex's shared lock
for the complete check before releasing the gate. Both acquisitions are
nonblocking; contention releases what was acquired and returns `LeaseActive`.
Mutation leases use exclusive locks, so verification observes one publication
state without updating owner metadata. An existing `owner.json` is validated as
historical audit only and is never described as the current contender. If
coordination was absent at start, the check creates nothing and
requires the exact name to remain absent at its final root-identity recheck; its
later appearance returns `Verification`. If a valid coordination/gate exists but
this domain has no mutex yet, the check holds the gate's shared lock for its whole
run, preventing bootstrap; an existing but invalid mutex fails read-only rather
than being repaired.

### SG-13.2 Focused test outlines

Shared-core tests shall prove:

1. strict paths reject absolute, dot, dotdot, backslash, empty-component,
   non-UTF-8-at-CLI, and symlink escapes;
2. corpus locations reject roots outside the owner and roots whose canonical
   components contain the exact reserved coordination name;
3. collection is sorted and rejects symlinks, special entries, and mount
   crossings;
4. local Git verification accepts the exact clean revision and rejects prefixes,
   wrong revisions, tree/blob/annotated-tag/peeled-tag pins, storage-format/pin-
   length mismatch, dirty/untracked state, wrong origins, and escaped source
   roots; absolute executable/exec-unit identity replacement and a PATH shadow
   sentinel (including empty and relative PATH components) fail without running
   the sentinel; its recursively enumerated commit-tree snapshot includes fixtures below
   nested directories, treats pathspec metacharacters literally, ignores a
   replacement ref, and retains pinned original blob bytes when checkout paths
   change afterward; a synthetic promisor repository with a locally missing blob
   fails without reading its available local promisor remote or repopulating the
   object store, and linked-worktree/common/object plus recursive alternate
   protection paths are resolved canonically; a tracked `.gitattributes` plus
   repository-configured clean/process sentinel filter is never executed by the
   raw HEAD/index/worktree cleanliness proof, verification leaves the complete
   source/Git tree unchanged, and raw filtered-byte mismatch, skip-worktree,
   assume-unchanged, index/tree drift, and nonignored untracked paths fail closed;
   the test-only local acquisition transport fetches an exact object into a fresh
   bare stage while contaminated system/global/home/local config, templates,
   hooks, includes, URL rewrites, credentials, filters, askpass, proxies, and Git
   environment sentinel programs remain unexecuted, auto-maintenance does not
   run, and production rejects the same file URL without network access;
5. dispositions require reasons exactly as SG-07 specifies, reject duplicate
   case IDs, accept repeated source paths for distinct case IDs, and return case-ID
   order;
6. filtered scope cannot authorize report or stale-output operations;
7. full and filtered requests contend on one corpus lease, including two
   `CorpusLocation` values that use distinct valid canonical owner ancestors for
   the same canonical corpus; owner metadata records the selected owner
   coherently as historical audit without ever being reported as the current
   holder; a mutator contending with a check returns generic `LeaseActive`,
   dropping releases the lease, and symlink, hard-link,
   unknown-header, owner-exchange collision, and coordination-component swaps
   observed before an atomic transition cannot redirect, overwrite, or truncate
   a lock or owner file; concurrent first acquisition publishes only complete
   locked gate/mutex inodes and journaled bootstrap intents, a race loser adopts
   the winner, and injected crashes before/after stage identity, header sync, and
   `RENAME_EXCL` recover only through the bootstrap intent or a complete adoptable
   final—never an unjournaled/empty/partial poisoned final; plan install
   accepts only a still-live matching corpus/domain lease and rejects a released,
   foreign-domain, or foreign-corpus lease before probes or writes; read-only
   verification takes/contends on the shared gate/mutex without creating state,
   and holds a pre-bootstrap shared gate for the complete check. Two distinct
   corpora sharing one canonical owner-target and cache unit derive the same
   byte-exact cache key, contend on one key mutex, and recover only that key's
   journal before either corpus mutex; alias target identities converge or fail;
8. artifact publication creates/syncs intent and old sidecar before registering
   an external stage, then deterministically builds and syncs one complete staged
   root, removes stale files only in a clean full staged state, and changes the
   final namespace with exactly one root transition; injected process death after
   active-directory creation, every metadata temporary write/sync/rename, stage
   registration, mid-file/tree construction, prepared publication, and before the swap
   leave the complete old final, process termination immediately before/after the
   swap recovers respectively to old/new, and termination during aborted-stage,
   committed-old-stage, or completed-journal-tombstone cleanup resumes from the
   exact recorded subset. Active `cleanup-complete`, completed rename, every
   metadata deletion, receipt deletion, and final `rmdir` are independently
   resumed. Marker or old-stage cleanup failure after the swap
   preserves the complete new final plus a read-only-detectable journal that the
   next lease clears; no test expects rollback after commit and no observed state
   is a mixed generation. Descriptor-bound tests replace roots,
   components, stage/final names, journals, and destination identities before
   each transition and prove no escape, overwrite, or disputed-object removal;
   no public plan or lease exists, internal plans have no arbitrary output-root
   constructor, a `CorpusLocation` rooted at/below coordination is rejected, and generated, retained, artifact,
   report, clone, and stale paths reject filesystem-equivalent coordination or
   transaction names at all depths. Exact-parent case/normalization probes use
   different injected policies in a parent and child and reject only where the
   actual parent aliases the pair; injected macOS device/fsid changes stop
   traversal. Cache tests repeat the state matrix under the matching key guard,
   verify immutable cache units publish only with `RENAME_EXCL`, reuse valid
   units, reject invalid units without replacement, accept only the browser's
   exact five link targets, and prove a corpus lease never recovers cache state;
9. hash text validation and report counts/provenance detect drift, shared reports
   accept structurally valid failed/unsupported counts without fictitious
   artifacts while domain validators enforce their own artifact/count mapping,
   and every `GeneratorErrorKind` has the exact SG-12 exit code;
10. an unregistered external stage/tombstone is rejected even when its final target is
    absent; compile-time mutation selection is exactly
    `aarch64-apple-darwin`; every mutation entry point in the already-installed
    WASM nonmutation build fails before coordination/cache/import/artifact/report
    mutation; supported-host rename probe failure leaves no domain mutation and
    reports any private residue; test documentation states that non-cooperating
    namespace mutation while leased is unsupported; missing macOS device/fsid or
    an inconclusive exact-parent name probe fails `UnsupportedPlatform` after
    private-probe cleanup;
11. `tests/public_api.rs` type-checks the exact SG-03.4 root reexports,
    constructors, getters, free functions, operation signatures, enum variants,
    and explicit traits. It asserts exact compact JSON bytes for every public
    Serde type, including field order, `expected-fail`, omitted versus null
    `reason`, sorted artifacts/provenance keys, and unsigned integer grammar. It
    rejects repeated/unknown fields, wrong scalar kinds, exponent/fraction
    spellings, per-field/aggregate overflow, zero `case_count`, noncanonical enum
    spellings, and every SG-03.4 identifier, case-ID, reason, URL, revision,
    extension, provenance, report, and constructor kind/context violation,
    proving deserialization cannot bypass one;
    the layout,
    CSS, and combined feature test builds type-check the exact SG-03.3 driver
    additions, including request structs' private construction boundary and
    `Clone + Debug + Eq + PartialEq` commitments. No alternative public module
    or compatibility alias is added.

Layout tests shall use synthetic temporary manifests, helpers, HTML, JSON
measurements, and artifacts to prove:

1. the explicit root CLI and closed command/argument matrix;
2. the exact public request constructor/getter and synchronous `run` signatures,
   including a call made from a thread already inside a Tokio runtime without a
   nested-runtime panic;
3. schema-2 parsing, unknown/duplicate and reserved-coordination overlap
   rejection, the exact browser/launch fields and 28 arguments, content-pin row,
   manifest-derived Taffy pin, exact owner-relative browser/Taffy cache derivation,
   first-run persistent cache-family scaffolds, byte-exact cache keys, and the
   complete cache/corpus/helper/HTML/XML namespace matrix before acquisition;
4. helper/base-style loading and hashes from the supplied corpus;
5. managed/existing browser validation through injected download/ZIP/version
   boundaries: exact retained top-level tree, logical content digest/counts,
   executable path/mode, five links/modes/targets, archive CRC/size/trailing-data
   rules, strict version stdout/stderr, and direct launch argv/environment/
   DevTools parsing. Synthetic archives cover early links, descendants after
   links, path escapes, duplicates/conflicts, cycles, special/hard-link modes,
   corrupt CRC, limits, and a byte change that misses the content pin without
   creating an outside sentinel. Sanitized fresh-bare Taffy acquisition crosses
   only the local sentinel boundary, without launching a browser, using the
   network, or checking out a worktree;
6. representative XML shape, four variants, numeric formatting, layout fields,
   and unchanged generated-by provenance;
7. disposition accounting, full/filtered isolation, and offline drift rejection;
   recoverable case-assigned failures publish successful artifacts plus the
   full-only failed-case diagnostic report, preserve stale/nonmanifest outputs,
   and return `Generation`, while fatal launch/acquisition/cleanup/serialization
   and pre-commit publication failures leave the old complete XML tree; a forced
   post-commit cleanup failure returns `ArtifactTransaction` with the complete
   new XML/report tree and recoverable journal, never mixed state;
8. the planned/fetched Taffy bare cache and object store cannot overlap browser
   cache, import, artifact/report, helper/manifest, or coordination writes;
9. injected fatal launch, unassigned-generation, browser-close, handler-join,
   profile, cache-transaction, and acquisition-stage failures, plus an injected operation panic
   after all synthetic resources are registered, a successful close with delayed
   child exit, a hung page close, and a hung handler exercise the exact 2/5-second
   graceful deadlines, one kill/abort, and unbounded terminal wait. The test
   proves the public call remains blocked past the forced grace interval, then
   releases the synthetic resource and observes child reap and a resolved join.
   Every handler is completed or aborted-and-joined, the profile is removed or
   retained only with its durable run intent, and a
   distinct-owner contender can acquire the released same-corpus lease; the
   panic maps to `Generation`, no final artifact, stale output, or report changes,
   and no detached-task counter remains. Death after run intent, profile
   registration, `launching`, child PID, `reaped`, profile cleanup, and run-
   tombstone transitions proves that uncertain child state is preserved while
   proven-reaped state resumes cleanup. Item 7 separately covers recoverable
   case-assigned page/measurement failures.

CSS tests shall use official-shaped synthetic fixture JSON and local temporary Git
repositories to prove:

1. exact-pin snapshot import, deterministic JSON-only copying, a same-directory-
   identity content change after snapshotting that cannot change imported bytes,
   a protected-directory identity replacement before final revalidation that
   returns `InvalidPath` and publishes nothing, count validation, and stale source
   removal;
2. the exact public request constructor/getter and synchronous `run` signatures;
3. multiple ordinary and error-array cases from one JSON file, multiple
   disposition overrides for that same source, JSON Pointer IDs, sorted cases,
   options, canonical CSS, and omission of AST/error/offset data;
4. malformed source structures, unmatched overrides, and duplicate decoded JSON
   members at the top-level ordinary label, ordinary `source`, `options`,
   `generate`, or `ast`, error-entry, and nested-options levels fail
   `InvalidInventory` before writes; escaped and literal duplicate-key spellings
   collide. A zero-case fixture is accepted by import but rejected by full and
   filtered generation as `InvalidInventory` and by offline checking as
   `Verification`;
5. equal and component-wise ancestor/descendant overlaps among import,
   expectation, manifest, and coordination namespaces fail with the specified
   `InvalidManifest` or `InvalidPath`, while the report accepts only its exact
   required child position within expectation. Exact-text, canonical, and
   existing-descriptor conflicts fail in the read-only prefix before lease/
   coordination creation; case-folded or Unicode-normalized aliases discovered
   only by actual-parent probing fail while the exclusive `css` mutex is held and
   leave only cleaned or durably journaled private probe state. Injected sibling
   directories with different policies prove the split. A verified checkout
   equal to, above, or below every CSS
   writable/coordination namespace fails with `InvalidPath` while leaving both
   the checkout and owner/corpus trees unchanged; linked-worktree administrative
   and common directories, primary object storage, and a recursively configured
   local alternate object store receive the same overlap matrix even when the
   canonical worktree root itself is textually disjoint, including an injected
   mount/path alias with matching descriptor ancestry;
6. active/default and non-active reason accounting, and one fixed `css` mutex on
   which import/full/filtered generation contend exclusively while checking
   contends shared;
7. full generation publishes the complete expectation/report root and removes
   stale outputs at its single root-swap commit; pre-commit failures retain the
   old tree and post-commit cleanup failures retain the new tree plus journal;
8. filtered generation updates only matches and writes/prunes no report;
9. offline verification detects imported-source, expectation, report, hash,
   provenance, count, and stale-inventory drift.

No focused test reads or executes the real layout or CSS repository corpus.

### SG-13.3 Final command matrix

The final implementation is verified with already-present tooling only:

```sh
cargo check --locked --offline -p surgeist-generator --no-default-features
cargo test --locked --offline -p surgeist-generator --no-default-features
cargo clippy --locked --offline -p surgeist-generator --no-default-features --all-targets -- -F unsafe-code -D warnings
cargo check --locked --offline -p surgeist-generator --target wasm32-unknown-unknown --no-default-features --lib
cargo check --locked --offline -p surgeist-generator --no-default-features --features layout-browser --all-targets
cargo test --locked --offline -p surgeist-generator --no-default-features --features layout-browser --all-targets
cargo clippy --locked --offline -p surgeist-generator --no-default-features --features layout-browser --all-targets -- -F unsafe-code -D warnings
cargo check --locked --offline -p surgeist-generator --no-default-features --features css-corpus --all-targets
cargo test --locked --offline -p surgeist-generator --no-default-features --features css-corpus --all-targets
cargo clippy --locked --offline -p surgeist-generator --no-default-features --features css-corpus --all-targets -- -F unsafe-code -D warnings
cargo check --locked --offline -p surgeist-generator --no-default-features --features layout-browser,css-corpus --all-targets
cargo test --locked --offline -p surgeist-generator --no-default-features --features layout-browser,css-corpus --all-targets
cargo clippy --locked --offline -p surgeist-generator --no-default-features --features layout-browser,css-corpus --all-targets -- -F unsafe-code -D warnings
cargo fmt --check
```

The native matrix runs on and records `rustc -vV` evidence for
`aarch64-apple-darwin`; a compile-time target assertion in focused tests prevents
a different host from being presented as supported evidence. The WASM check uses
the already-installed standard library and compiles the explicit nonmutation
shared-core branch. No target is installed. The final gate also builds the
tracked/nonignored Surgeist-owned Rust manifest and runs the canonical
repository-wide unsafe scan. It does not run either binary's acquisition or
real-corpus generation paths and does not run commands in `surgeist-layout` or
`surgeist-css`.

## SG-14 Documentation, compatibility, and handoff

`README.md` shall describe the shared-core ownership, feature matrix, exact CLI
syntax, acquisition-capable commands, offline checks, the
Apple-Silicon-macOS-only mutation-capability boundary, the already-verified WASM
nonmutation core, and the fact that consumer corpora remain in layout/CSS.
`AGENTS.md` shall cease describing an empty scaffold and shall point
discovery at the new modules, features, binaries, tests, and offline command
matrix. No copied workflow is added.

Compatibility classification:

- Rust public API: additive relative to the scaffold; `CRATE_NAME` remains;
- Cargo features and binary targets: additive;
- layout generator CLI: intentionally breaking at the yet-unwired generator-crate
  boundary because roots and optional inputs become explicit arguments instead
  of environment/default state;
- layout manifest/XML/report schema: compatible schema 2;
- CSS manifest/expectation/report schema: new schema 1;
- generator mutation lifecycle: descriptor-confined and runtime-verified on
  `aarch64-apple-darwin`; Linux and other native targets are explicitly deferred,
  while the already-present WASM target verifies the nonmutation core branch;
- MSRV: unchanged at 1.97;
- production dependency graph: unchanged because no production crate is wired.

After publication, the candidate handoff shall require separate owning-repository
cycles to:

1. add `surgeist-generator` to root's crate roster/gitlinks/tooling topology as
   appropriate;
2. rewire `surgeist-layout` scripts/tests to invoke the published layout binary,
   prove real corpus compatibility there, and only then remove its duplicate
   generator source;
3. create the CSS-owned schema-1 manifest/corpus, invoke the CSS binary, and add
   CSS consumer tests;
4. refresh root-owned API audit artifacts only after root integrates the
   published source;
5. add Linux or other native mutation support only in an environment that already
   has the relevant target/tooling, with its own descriptor, mount-identity,
   atomic-swap, crash-recovery, and full feature-matrix evidence.

Those handoffs do not block this leaf candidate when its synthetic contract and
feature matrix are clean.
