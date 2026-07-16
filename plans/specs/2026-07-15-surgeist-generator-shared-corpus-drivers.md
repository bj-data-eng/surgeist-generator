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
offline verification. Chromium, Tokio, URL, and fetcher dependencies shall only
compile with `layout-browser`. CSS corpus code shall only compile with
`css-corpus`. The default library shall compile only the shared core.

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

Copy-first is a distinct, auditable implementation step. Before changing any
copied byte or introducing the refactored generator modules, one standalone
implementation commit shall add only the production prefix as the tracked file
`src/layout/legacy_generator.rs`. That file shall contain exactly source lines
1 through 4626, including their final newline, and shall have SHA-256
`d2f5ca87cea6b36826e9172e2d7ba7a99196c375e2ca53f8a84a075200e70a9f`.
The cycle evidence shall record the copy commit SHA, its parent SHA, the source
commit and path above, the destination path, a byte-for-byte comparison result,
and a name-status diff proving that the destination is the commit's only file
change. The file remains deliberately unreferenced at this step, so the copy is
not transformed as a side effect of making it compile. A later reviewed task may
split, adapt, and remove the transient legacy file while preserving behavior in
the final modules. This exemption applies only to that identified production
prefix; all other layout and CSS source, tests, manifests, corpora, helpers, and
artifacts remain prohibited from copying.

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
| `chromiumoxide` | `=0.9.1`, no defaults, `fetcher`, `rustls`, `zip8` | no | yes | no | Managed pinned Chromium measurement |
| `futures` | `=0.3.31` | no | yes | no | Chromium handler stream |
| `tokio` | `=1.48.0`, `fs`, `macros`, `rt-multi-thread` | no | yes | no | Async layout driver and thin binary |
| `url` | `=2.5.7` | no | yes | no | Fixture and base URL handling |

`default = []`. `layout-browser` activates the four heavy optional dependencies
and the layout module/binary. `css-corpus` activates the CSS module/binary and no
heavy dependency. Both features may be enabled together. The two binaries use
`required-features` so an unrequested driver cannot compile accidentally.

All named dependency sources are already present in the local Cargo registry.
The now binary-bearing tool package shall track `Cargo.lock`; `.gitignore` shall
stop ignoring that one file. All implementation and verification commands use
`--locked --offline`, so no dependency acquisition occurs.

### SG-03.3 Public front door

`src/lib.rs` remains `#![forbid(unsafe_code)]` and retains
`CRATE_NAME: &str = "surgeist-generator"`. It intentionally exposes:

- `GeneratorError`, `GeneratorErrorKind`, and the crate `Result<T>` alias;
- `core` contract types named in SG-04 through SG-10;
- `layout::LayoutRequest`, `layout::LayoutCommand`,
  `layout::run`, and `layout::run_from_env` only with `layout-browser`;
- `css::CssRequest`, `css::CssCommand`, `css::run`, and
  `css::run_from_env` only with `css-corpus`.

All public structs have private fields and checked constructors. Public enums
whose variants may evolve are `#[non_exhaustive]`. The library does not expose
Chromiumoxide, Tokio task, Serde JSON value, filesystem lock, or child-process
types in public signatures. The public API is additive relative to the scaffold.

## SG-04 Semantic core types

### SG-04.1 Corpus location

`CorpusLocation` owns two canonical absolute paths:

- an owner root, used for repository-relative caches, provenance, and lease
  placement;
- a corpus root, which must exist as a directory at construction and must be the
  owner root itself or a descendant of it.

Construction rejects missing roots, non-directories, non-UTF-8 CLI inputs,
canonicalization failures, and corpus roots that escape the owner root through
lexical components or symlinks. Callers cannot mutate either path after
construction.

Every binary invocation requires explicit `--owner-root <path>` and
`--corpus-root <path>`. There is no default corpus, `CARGO_MANIFEST_DIR` fallback,
current-directory inference, or corpus-root environment override. A consumer may
pass relative CLI paths, but construction canonicalizes them before use.

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

- `Full` is verification-capable and may install the complete artifact set,
  write the canonical report, remove stale generated artifacts, and remove
  non-manifest reports after every job succeeds;
- `Filtered(RelativePath)` is iteration-only and may install only matching
  artifacts. It must not write or prune reports, remove stale nonmatching
  artifacts, or count as final verification evidence.

Filters name an exact source fixture or a directory prefix. Construction proves
that at least one source matches before a lease is acquired or any output is
written. Layout permits filters only for `generate-existing`; CSS permits them
only for `generate`. Empty filters are invalid rather than aliases for full runs.

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
import sections, then derives the pin and cache location from the manifest.
The fixed domain names (`chrome-for-testing`, `taffy`, `surgeist`, and
`constrained-html`) remain layout schema semantics.

The helper JavaScript and base CSS are loaded from
`scripts/gentest/test_helper.js` and
`scripts/gentest/test_base_style.css` under the supplied corpus root. The helper
directory must contain exactly those two regular files. Their bytes remain
layout-owned and feed the same hashes, browser document, XML provenance, and
report metadata as before.

### SG-05.3 CSS schema 1

The CSS driver reads `corpus.toml` schema 1 with exactly these sections:

```toml
schema_version = 1

[source]
kind = "csstree"
repository = "https://github.com/csstree/csstree.git"
revision = "<exact 40- or 64-lowercase-hex Git object id>"
fixture_root = "fixtures/ast"
import_root = "source/csstree"
expected_files = 1
expected_cases = 1

[artifacts]
expectation_root = "expectations"
report_file = "generation-reports/all.json"

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
relative paths. The report path must be one JSON file under
`generation-reports`. File and case counts are positive and exact.

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
2. `git rev-parse HEAD` exactly equals the manifest revision, not merely a
   prefix;
3. `git status --porcelain=v1 --untracked-files=all` is empty;
4. `git remote get-url origin` exactly equals the manifest repository URL;
5. the requested source subdirectory is a directory inside the canonical
   checkout without a symlink escape.

Verification performs no fetch, checkout, remote mutation, configuration change,
or network access. `VerifiedSource` retains the canonical root and exact revision
and cannot be constructed publicly without verification.

The layout `import-taffy` command retains its existing explicitly
acquisition-capable fetch/checkout behavior in domain code and then passes the
result through this verifier. No test or verification gate invokes that command.
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
- duplicate IDs or duplicate source identities fail validation;
- `FailedToGenerate` is a runtime report outcome, not a manifest disposition.

Layout semantics remain: active cases generate normally; expected-fail cases run
and are accounted separately; unsupported and quarantined cases do not run and
their old outputs are removed only during a successful full run. CSS applies the
same accounting to derived neutral cases.

### SG-07.2 CSSTree ingestion

`import-csstree` verifies the supplied checkout, collects the pinned fixture JSON
inventory, and atomically mirrors those JSON bytes beneath the CSS-owned
`import_root`. It preserves relative paths, rejects all non-JSON and special
entries, checks the exact source-file count, and removes stale imported JSON only
as part of a successful complete import transaction. It writes no expectations
or generation report.

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

Malformed top-level shapes, nonobject ordinary cases, missing/nonstring `source`,
ordinary cases without `ast`, nonarray `error`, nonobject error entries, invalid
`options`, invalid `generate`, duplicate derived IDs, and unmatched manifest
overrides fail before any expectation or report write.

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
are rejected.

## SG-09 Atomic installation and stale-output behavior

`ArtifactPlan` owns one canonical output root, a `BTreeMap` of unique relative
output paths to bytes, and the exact retained output inventory for a full run.
Construction hashes content and rejects path collisions before touching disk.

Installation follows one transaction:

1. create every parent beneath the output root;
2. write every new artifact to a unique sibling file opened with `create_new`;
3. flush and sync each staged file;
4. rename existing replacement and stale generated files to unique sibling
   backups in deterministic path order;
5. install staged artifacts by rename in deterministic path order;
6. on any failure, remove installed new files, restore every backup, and remove
   remaining staged files;
7. after success, delete backups and remove only now-empty generated directories.

The transaction never removes a file outside the declared generated extension
and roots. It never follows an output symlink. Residual temporary or backup names
from another process are collisions, not files to overwrite.

Domain generation may commit one coherent artifact group at a time, but each
group uses this transaction. Full-run stale removal occurs only after every job
succeeds and uses a complete retained inventory. A failed full run may write its
diagnostic report, but it preserves stale artifacts and nonmanifest reports.
Filtered runs never provide a stale inventory and cannot call stale removal.

The canonical report itself uses the same staged replace-and-rollback primitive.

## SG-10 Generation leases and lifecycle

`GenerationLease` uses the standard library's advisory file lock. Its key is the
domain plus canonical corpus-root digest, so full and filtered runs for the same
domain/corpus contend while unrelated layout and CSS corpora may run concurrently.
Lease and acquisition-gate files live beneath
`<owner-root>/target/surgeist-generator/` and include generator, PID, corpus root,
scope, command, and Unix start time.

The acquisition gate is locked while owner metadata is truncated, written, and
synced, preventing a contender from observing stale or empty ownership. A held
lease returns a semantic `LeaseActive` error with the recorded owner. Dropping the
lease releases it. Lock files may remain on disk as reusable coordination files;
they are not generated corpus artifacts.

The command lifecycle is:

```text
CLI parse
  -> CorpusLocation validation
  -> manifest parse and semantic validation
  -> filter/source inventory validation
  -> optional source verification/import
  -> generation lease
  -> deterministic collection and domain generation
  -> atomic artifact groups
  -> full-only report and stale cleanup
  -> success or semantic error
```

Manifest, source, and filter errors occur before the lease or writes. Generation
and install errors retain failure provenance and perform rollback as defined in
SG-09.

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
with 64 for CLI contract errors or 1 for all other errors. It contains no manifest,
path, source, generation, artifact, or report behavior.

## SG-12 Error model

`GeneratorError` is a public struct with private data, a nonexhaustive semantic
`GeneratorErrorKind`, `Display`, `std::error::Error`, `kind()`, and `exit_code()`.
Kinds are:

- `Cli`;
- `InvalidPath`;
- `InvalidManifest`;
- `InvalidInventory`;
- `SourceVerification`;
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

| Failure | Required kind | Mutation rule |
| --- | --- | --- |
| Missing/duplicate/unknown CLI argument | `Cli` | No filesystem mutation |
| Root, relative path, or symlink escape | `InvalidPath` | No corpus mutation |
| TOML parse, version, or field contract | `InvalidManifest` | No corpus mutation |
| Count, duplicate, unmatched override, or malformed CSSTree case | `InvalidInventory` | No artifact/report mutation |
| Wrong/dirty Git source or origin | `SourceVerification` | No import mutation |
| Contended generation lease | `LeaseActive` | No corpus mutation |
| Git/browser subprocess failure | `Process` | Domain cleanup; no stale pruning |
| Read/write/canonicalize failure | `Io` | Transaction rollback where applicable |
| Stage/backup/install/restore failure | `ArtifactTransaction` | Best-effort complete rollback with terminal diagnostic |
| Browser measurement or neutral conversion failure | `Generation` | No stale pruning; full report may record failure |
| Offline hash/provenance/report mismatch | `Verification` | Check commands remain read-only |

## SG-13 Verification behavior

### SG-13.1 Offline checks

Layout `check-corpus` remains browser-free and validates the schema-2 manifest,
helper-only asset directory, case/source inventory, report inventory and counts,
XML inventory, and every provenance hash. `check-taffy-corpus` verifies the
manifest-derived cached checkout and imported baseline without network access.

CSS `check-corpus` reads only the CSS-owned corpus. It validates schema 1, source
and expectation inventories, exact counts, derived case IDs, disposition reasons,
source-to-expectation one-to-one paths, expectation provenance, report inventory,
all source/artifact hashes, case/report counts, and absence of stale generated
JSON. It never requires the original CSSTree checkout after import.

All check commands are read-only. A failing check reports the first deterministic
violation and writes nothing.

### SG-13.2 Focused test outlines

Shared-core tests shall prove:

1. strict paths reject absolute, dot, dotdot, backslash, empty-component,
   non-UTF-8-at-CLI, and symlink escapes;
2. corpus locations reject roots outside the owner;
3. collection is sorted and rejects symlinks/special entries;
4. local Git verification accepts the exact clean revision and rejects prefixes,
   wrong revisions, dirty/untracked state, wrong origins, and escaped source roots;
5. dispositions require reasons exactly as SG-07 specifies and reject duplicates;
6. filtered scope cannot authorize report or stale-output operations;
7. full and filtered requests contend on one corpus lease, owner metadata is
   coherent, and dropping releases the lease;
8. artifact installation is deterministic, replaces atomically, removes stale
   files only when authorized, and restores every prior file after injected
   staging or installation failure;
9. hash text validation and report counts/provenance detect drift.

Layout tests shall use synthetic temporary manifests, helpers, HTML, JSON
measurements, and artifacts to prove:

1. the explicit root CLI and closed command/argument matrix;
2. schema-2 parsing, unknown/duplicate rejection, and manifest-derived Taffy pin;
3. helper/base-style loading and hashes from the supplied corpus;
4. managed/existing browser validation through injected fetch/version boundaries
   without launching or acquiring a browser;
5. representative XML shape, four variants, numeric formatting, layout fields,
   and unchanged generated-by provenance;
6. disposition accounting, report behavior, full/filtered isolation, and offline
   drift rejection.

CSS tests shall use official-shaped synthetic fixture JSON and local temporary Git
repositories to prove:

1. exact-pin import, deterministic JSON-only copying, count validation, and stale
   source removal;
2. ordinary and error-array flattening, JSON Pointer IDs, sorted cases, options,
   canonical CSS, and omission of AST/error/offset data;
3. malformed source structures and unmatched overrides fail before writes;
4. active/default and non-active reason accounting;
5. full generation writes expectations/report and removes stale outputs only
   after success;
6. filtered generation updates only matches and writes/prunes no report;
7. offline verification detects imported-source, expectation, report, hash,
   provenance, count, and stale-inventory drift.

No focused test reads or executes the real layout or CSS repository corpus.

### SG-13.3 Final command matrix

The final implementation is verified with already-present tooling only:

```sh
cargo check --locked --offline -p surgeist-generator --no-default-features
cargo test --locked --offline -p surgeist-generator --no-default-features
cargo clippy --locked --offline -p surgeist-generator --no-default-features --all-targets -- -F unsafe-code -D warnings
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

The final gate also builds the tracked/nonignored Surgeist-owned Rust manifest and
runs the canonical repository-wide unsafe scan. It does not run either binary's
acquisition or real-corpus generation paths and does not run commands in
`surgeist-layout` or `surgeist-css`.

## SG-14 Documentation, compatibility, and handoff

`README.md` shall describe the shared-core ownership, feature matrix, exact CLI
syntax, acquisition-capable commands, offline checks, and the fact that consumer
corpora remain in layout/CSS. `AGENTS.md` shall cease describing an empty scaffold
and shall point discovery at the new modules, features, binaries, tests, and
offline command matrix. No copied workflow is added.

Compatibility classification:

- Rust public API: additive relative to the scaffold; `CRATE_NAME` remains;
- Cargo features and binary targets: additive;
- layout generator CLI: intentionally breaking at the yet-unwired generator-crate
  boundary because roots and optional inputs become explicit arguments instead
  of environment/default state;
- layout manifest/XML/report schema: compatible schema 2;
- CSS manifest/expectation/report schema: new schema 1;
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
   published source.

Those handoffs do not block this leaf candidate when its synthetic contract and
feature matrix are clean.
