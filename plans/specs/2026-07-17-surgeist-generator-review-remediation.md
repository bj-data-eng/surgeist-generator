# Surgeist Generator Review Remediation Specification

## Header

- Repository: `/Users/codex/Development/surgeist-generator`.
- Status: `proposed`; implementation is prohibited until this specification and
  its implementation sequence each receive an independent `CLEAN` planning
  review.
- Immutable implementation-series base:
  `1e545530fb992e2e749f26a1ba7ac0d77ed85dd6`.
- First planning artifact: committed baseline review
  `plans/2026-07-17-crate-baseline-review.md` at
  `05293743a551454adcf63345e80ef0d3982786b1`, source baseline
  `1e545530fb992e2e749f26a1ba7ac0d77ed85dd6`.
- Terminal outcome: every actionable finding in that review is resolved on a
  clean, remotely verified `surgeist-generator` `main`, ready for the user's new
  independent review.

## SR-01 Authority, Evidence, And Scope

The normative inputs are the user's current goal, the committed baseline review,
the repository `AGENTS.md`, the current package/source/test tree, and this newly
reviewed specification. Deleted planning packets are historical evidence only.
They do not waive a current finding, authorize sibling work, or become normative
by reference.

Only `surgeist-generator` may be mutated. The allowed implementation surface is
its manifest and lockfile, library and binary source, focused synthetic tests,
README and repository guidance, and the planning/evidence files required by the
Surgeist workflow. Root `surgeist`, `surgeist-layout`, `surgeist-css`, their
corpora, and their generated artifacts remain outside the mutation and test
scope. No sibling checkout is required to complete this series.

The following are explicit non-goals:

- no root facade, gitlink, API-audit, or sibling integration change;
- no corpus, fixture, generated XML/JSON, manifest, or helper asset copied from a
  production crate;
- no real Chromium launch, browser download, Git clone/fetch, or production
  corpus generation during implementation or verification;
- no dependency, target, toolchain, browser, or other software acquisition;
- no generalized plugin framework or open driver trait;
- no executable `unsafe` in repository-owned Rust;
- no normal-build dependency from a production Surgeist crate to this tooling
  crate.

The exact 4,626-line preservation copy at
`src/layout/legacy_generator.rs` remains immutable with SHA-256
`d2f5ca87cea6b36826e9172e2d7ba7a99196c375e2ca53f8a84a075200e70a9f`
until the layout driver task proves its required behavior is represented. It is
then removed, not left as a second implementation. Corpus-owned helper assets are
loaded through the explicitly supplied corpus root; they are not copied here.

## SR-02 Acceptance And Finding Traceability

The series is complete only when all six baseline findings have executable or
documentary closure:

| Finding | Required closure |
| --- | --- |
| Rooted lease-tree rejection | Missing intermediate components are ordinary absence only for existence queries; strict alias, type, mount, and policy rejection remains; all lease/artifact tests reach their intended assertions. |
| Model-only crash/bootstrap tests | Deterministic test-only interruption drives the real bootstrap and transaction implementations, then a fresh engine runs real recovery and proves visibility, residue, and idempotence. |
| Missing domain surface | Real feature-owned layout and CSS modules plus `surgeist-layout-generate` and `surgeist-css-generate` targets use the shared core; features have real edges; the artificial linkage shim and represented legacy copy are gone. |
| Quality matrix | Formatting, Clippy with warnings denied, native feature combinations, and warning-denied portable library checking are clean offline. |
| Stale guidance | README and AGENTS describe the completed shared-core/driver boundary, supported target behavior, feature matrix, corpus ownership, and authoritative checks. |
| Tautological CLI test | A real parser/command error is classified as `Cli` and an interface binary is observed exiting with code 64. |

No finding is closed by deleting a test, weakening a filesystem invariant,
silencing a warning, or documenting missing behavior as complete.

## SR-03 Shared-Core Correction Contract

### SR-03.1 Rooted existence

`RootedFs::exists` is a non-mutating query. On the supported mutation target it
must traverse each component descriptor-relatively. If `statat` reports `NOENT`
for any component, it returns `Ok(false)`. Every component that does exist still
undergoes exact-name enumeration, no-follow opening, same-mount validation, and
held-identity/type/policy checks. A case alias, symlink, mount change,
non-directory intermediate component, non-UTF-8 sibling, permission error, or
other inconclusive state remains an error.

Generic `open_parent`, mutation helpers, and exact-entry validation retain their
strict fail-closed semantics. The correction is an existence-aware traversal,
not an error string match and not a blanket conversion of traversal failures to
false.

Focused tests cover:

- a fresh `.surgeist-generator/leases/layout` query where `leases` is absent;
- absence of the final component after valid existing parents;
- no filesystem mutation by either query;
- rejection of an existing aliased, symlink, or non-directory intermediate;
- the complete lease and artifact suites after the boundary correction.

### SR-03.2 Real durable transaction interruption

The hard-coded `TransactionProtocol::crash_prefixes`, `CrashPrefix`, and
`RecoveredPrefix` model may remain only as a supplementary ordering assertion.
They cannot be the proof for crash safety.

Production `TransactionEngine::install` and `recover_all` share one internal
state machine with a test-only interruption controller. The controller is
compiled only for tests, is scoped to one engine instance, and identifies named
durable boundaries rather than filesystem-call counts. Production construction
cannot select an interrupted return path. A simulated interruption returns from
the real install state machine without calling its same-process cleanup, drops
all held handles, and leaves exactly the durable bytes and names present at that
boundary.

At minimum the controller can interrupt immediately after each of these real
boundaries, including both exclusive creation and swap replacement where the
boundary applies:

1. active journal directory creation;
2. intent publication;
3. old-inventory sidecar publication;
4. stage reservation and stage-registration publication;
5. registered-stage rename;
6. staged tree population and directory sync;
7. new-inventory sidecar publication;
8. prepared-marker publication;
9. final-root commit rename/swap;
10. committed-root parent sync;
11. committed-outcome publication;
12. old-tree removal;
13. cleanup-receipt/completed-journal publication and final cleanup.

For every supported boundary a fixture starts with known old bytes, requests
known new bytes, executes the actual interrupted install, drops and reopens its
rooted authority, constructs a new `TransactionEngine`, and calls the production
`recover_all`. Assertions inspect actual files and journals:

- before the commit boundary, one complete old generation is visible;
- at and after commit, one complete new generation is visible;
- no mixed old/new tree is observable;
- recovery never removes an unrecorded replacement or unknown entry;
- an intentionally interrupted recovery retains sufficient journal evidence for
  another `recover_all` call;
- repeated recovery is idempotent and leaves no owned residue after completion.

The test uses only temporary synthetic trees. It never crashes the test runner,
forks an external helper, or touches a production corpus.

### SR-03.3 Real bootstrap interruption

Bootstrap proof likewise executes `open_or_bootstrap_lock`, bootstrap journal
validation, `recover_bootstrap`, and cleanup logic rather than a test-only step
array. A test-only, instance-scoped boundary hook interrupts the real bootstrap
after durable intent, stage creation/registration, complete header sync, final
rename, parent sync, and cleanup publication/removal boundaries. Recovery uses
the production algorithm with an injected liveness oracle only in tests so the
simulated abandoned owner is treated as dead; production continues to use the
real process-liveness probe.

Each prefix proves that recovery either publishes one complete immutable lock
header or safely removes only its recorded incomplete stage, never exposes a
partial header, is idempotent, and leaves no bootstrap journal after successful
completion. Existing corruption, identity substitution, alias, and live-owner
tests remain fail closed.

### SR-03.4 Quality and internal ownership

The identity `map_err`, portable unused import/dead helpers, and all rustfmt
deltas are corrected after behavioral tests are green. Target-specific helpers
are gated at their narrowest ownership boundary; portable builds are checked
with warnings denied.

`ArtifactPlan`, `PublicationInventory`, `PublicationPolicy`, `GenerationLease`,
`GenerationCheck`, and `Domain` remain crate-private implementation machinery.
The two real domain modules call them. `private_front_doors_are_linked` and all
artificial function references are removed. The default public value/source
contract remains additive and source compatible.

## SR-04 Package, Features, And Public Boundary

The package remains edition 2024, Rust 1.97, MIT, version 0.1.0, and
`default = []`. Exact existing shared dependencies remain unchanged. The
Apple-Silicon macOS `rustix = 1.1.4` dependency remains target-specific.

`layout-browser` owns optional heavy dependencies already present in the local
Cargo cache:

- `chromiumoxide = 0.9.1`, optional, default features disabled, with `bytes`,
  `fetcher`, `rustls`, and `zip8`;
- `futures = 0.3.31`, optional;
- `tokio = 1.48.0`, optional, with the runtime, macros, time, process, fs, and
  I/O features required by the private synchronous supervisor;
- `url = 2.5.7`, optional.

`css-corpus` activates no new dependency. Both features may be enabled together.
Each binary has an explicit `[[bin]]` entry and `required-features`. Default
library builds therefore compile no browser or CSS driver implementation.

Feature-gated root modules expose these concrete fronts and no open driver trait:

```rust
#[cfg(feature = "layout-browser")]
pub mod layout;
#[cfg(feature = "css-corpus")]
pub mod css;
```

The layout module exposes checked `LayoutRequest`, non-exhaustive
`LayoutCommand`, `run(LayoutRequest) -> Result<()>`, and
`run_from_env() -> Result<()>`. The CSS module exposes checked `CssRequest`,
non-exhaustive `CssCommand`, `run(CssRequest) -> Result<()>`, and
`run_from_env() -> Result<()>`. Request fields are private; constructors and
accessors expose only `CorpusLocation`, `RelativePath`, ordinary paths where an
external source checkout is required, and domain enums. No Tokio,
Chromiumoxide, rooted descriptor, lease, transaction, or untyped JSON type
appears in a public signature.

Both interface binaries are at most fifteen physical lines. They delegate to
the library, print one prefixed diagnostic on failure, and exit exclusively via
`GeneratorError::exit_code`.

## SR-05 Layout Driver Contract

### SR-05.1 Commands and explicit roots

The layout interface accepts:

```text
surgeist-layout-generate --owner-root <path> --corpus-root <path> \
  <generate|generate-existing|check-corpus|check-taffy-corpus|import-taffy> \
  [--browser-path <owner-relative-path>] [--filter <html-relative-path>]
```

`generate` uses the manifest-owned managed browser pin and rejects browser path
and filter. `generate-existing` requires a browser path contained beneath the
manifest cache and alone accepts a filter. Check and import commands reject
browser/filter arguments. Root, filter, browser-cache, and version environment
overrides from the legacy program are not accepted. All malformed/duplicate/
missing/unknown arguments fail before domain I/O as `GeneratorErrorKind::Cli`.

The public call is synchronous. It constructs and owns its Tokio runtime on a
private worker thread, drives the async browser handler to a terminal state, and
joins before returning. Browser, handler task, temporary browser profile, lease,
and runtime are never detached on success, error, or panic.

### SR-05.2 Manifest and corpus ownership

The driver strictly reads the corpus-owned schema-2 `corpus.toml` represented by
the preserved generator: source roots, pinned Taffy import, HTML cases and
dispositions, browser cache/version/version output/provenance/launch profile,
and canonical full/scoped report paths. Unknown fields, wrong schema, duplicate
case/source ownership, non-strict relative paths, namespace overlap, wrong
generator/status/reason combinations, and inconsistent report declarations fail
before mutation.

The owner and corpus roots come only from `CorpusLocation`. HTML input, generated
XML, manifest, reports, and these helper assets remain corpus-owned:

- `scripts/gentest/test_helper.js`;
- `scripts/gentest/test_base_style.css`.

The helpers are read, hashed, and injected through the supplied corpus root.
Their bytes are not compiled into or copied to this crate. The manifest-owned
browser cache remains owner-owned and outside generated corpus output.

### SR-05.3 Generation and publication

The transformed driver retains the represented legacy behavior for deterministic
HTML collection, helper/base-style injection, file/base URL handling, DOM-ready
waiting, computed layout and grid-template-area measurement, retry in a fresh
browser when classified retryable, deterministic XML rendering, provenance,
report accounting, and browser/profile cleanup.

The driver validates the exact browser executable containment and `--version`
output before launch. Managed acquisition remains available only to `generate`;
verification never invokes it. `generate-existing` never acquires software.
Synthetic tests replace the browser boundary with deterministic measurements and
exercise HTML transformation, ordering, XML bytes, report bytes, retry/cleanup,
and error classification without launching Chromium.

Mutation uses the shared layout-domain lease and `ArtifactPlan`:

- a successful unfiltered clean full run atomically installs the complete XML
  publication unit and canonical report and removes only manifest-classified
  stale output;
- diagnostic full preserves stale output;
- filtered generation writes only matched artifacts, writes no canonical report,
  and removes no stale output;
- failure before installation changes no generated artifact;
- offline checks acquire a shared check guard, create/recover nothing, validate
  manifest/source/artifact/report hashes and inventory, and finish the guard.

Taffy import uses the manifest pin, shared source verification, deterministic
HTML inventory, and the same exclusive layout-domain publication boundary. Tests
use a synthetic already-present checkout; implementation and verification never
run its acquisition path.

### SR-05.4 Preservation retirement

Before deleting `src/layout/legacy_generator.rs`, layout task evidence records its
original SHA-256 and maps every represented responsibility to compiled domain
code and a focused test or to an explicit corpus-owned input. The deletion occurs
in the same reviewed task that proves layout feature compilation and tests.

## SR-06 CSS Driver Contract

### SR-06.1 Commands and manifest

The CSS interface accepts:

```text
surgeist-css-generate --owner-root <path> --corpus-root <path> \
  <import-csstree|generate|check-corpus> \
  [--source-root <path>] [--filter <import-relative-json-or-prefix>]
```

`import-csstree` requires a source root and rejects a filter. `generate` rejects a
source root and optionally accepts a filter. `check-corpus` rejects both. Parser
failures are `Cli` errors before filesystem/process activity.

The corpus-owned schema-1 manifest contains a `csstree` HTTPS repository and
exact 40- or 64-lowercase-hex revision, strict `fixture_root`, one-component
`import_root` and `expectation_root`, exact positive file/case counts, canonical
`<expectation_root>/generation-reports/all.json`, and optional disposition
overrides. Writable namespaces are component-wise disjoint from each other,
`corpus.toml`, and `.surgeist-generator`.

### SR-06.2 Import and neutral expectations

Import verifies the explicitly supplied checkout with shared `PinnedSource` and
`verify_git_source`, takes an immutable deterministic JSON snapshot below
`fixture_root`, checks the exact file count, and atomically installs it beneath
`import_root` with a canonical `.surgeist-source.json` sidecar. The sidecar binds
schema, complete source pin, object format, sorted path/blob/digest inventory,
and count. Import writes no expectations or report and removes stale imported
files only as part of the successful complete root replacement.

Generation validates the sidecar against the current manifest and derives cases
from each imported top-level CSSTree fixture object:

- ordinary named object entries require string `source` and an `ast`; optional
  object `options` and string `generate` are preserved;
- `error` is an optional array of objects requiring string `source`;
- IDs are `<relative-json>#/<JSON-pointer-escaped-label>` or
  `<relative-json>#/error/<index>`;
- each case records context, label when present, input, canonical options,
  upstream outcome `parsed` or `rejected`, optional canonical CSS, disposition,
  and reason;
- ASTs, upstream diagnostic prose, offsets, comments, and recovery structures
  are never copied.

Duplicate decoded JSON members at any depth, malformed shapes, empty fixtures,
duplicate IDs, unmatched overrides, invalid options/generate fields, and count
drift fail before publication. Objects are recursively key-sorted, arrays retain
order, expectation files use deterministic pretty JSON with one final LF, and
source/provenance hashes bind every output.

Full clean, diagnostic, and filtered behavior matches SR-05.3 through the shared
CSS-domain lease and artifact plan. A filtered run writes no canonical report and
removes no stale output. `check-corpus` is offline/read-only and validates the
manifest, import sidecar, expectations, report, disposition accounting, hashes,
and exact inventories without invoking Git or mutating coordination state.

Synthetic fixtures cover ordinary/error cases, JSON-pointer escaping, repeated
source paths with unique case IDs, canonical options, duplicate keys, malformed
records, full/filtered stale behavior, sidecar drift, and report accounting.

## SR-07 Errors, Documentation, And Verification

`GeneratorErrorKind` remains the stable non-exhaustive semantic set already in
source. Domain adapters preserve safe I/O/process sources and map malformed CLI
to `Cli`, schema to `InvalidManifest`, inventory/fixture shape to
`InvalidInventory`, source pin drift to `SourceVerification`, unsupported
mutation targets to `UnsupportedPlatform`, lifecycle failures to the existing
lease/process/transaction/generation/verification kinds. No external input is
handled by panic.

At least one unit parser test constructs a real CLI-kind error and proves
`exit_code() == 64`. Each binary has an integration test that invokes an invalid
argument form, observes one prefixed diagnostic, and observes process status 64
without accessing a corpus or browser.

README and AGENTS are updated only after both drivers are real. They describe:

- the small default shared core and exact feature/binary matrix;
- explicit owner/corpus roots and corpus ownership;
- layout browser/HTML/XML and CSS CSSTree/neutral-expectation responsibilities;
- Apple-Silicon macOS mutation support versus portable read/value compilation;
- offline checking and the fact that normal production builds do not depend on
  this crate;
- the authoritative locked/offline command inventory.

The final candidate must pass, using only already installed tooling and cached
dependencies:

```sh
cargo check --locked --offline -p surgeist-generator --no-default-features
cargo test --locked --offline -p surgeist-generator --no-default-features
cargo check --locked --offline -p surgeist-generator --features layout-browser
cargo test --locked --offline -p surgeist-generator --features layout-browser
cargo check --locked --offline -p surgeist-generator --features css-corpus
cargo test --locked --offline -p surgeist-generator --features css-corpus
cargo check --locked --offline -p surgeist-generator --all-features
cargo test --locked --offline -p surgeist-generator --all-features
cargo clippy --locked --offline -p surgeist-generator --all-features --all-targets -- -F unsafe-code -D warnings
RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --target wasm32-unknown-unknown --no-default-features --lib
cargo fmt --check
```

Feature tests must use synthetic adapters and cannot launch/download Chromium,
clone/fetch Git, or read a sibling corpus. The final evidence also records an
owned-Rust executable-unsafe scan, the retired preservation-copy digest proof,
the exact baseline-review disposition table, clean status, immutable remote
readback, and plan-directory cleanup retaining canonical `plans/.gitkeep`.

## SR-08 Workflow And Completion

The reviewed implementation sequence allocates every SR section and baseline
finding exactly once across publishable cycles. For every cycle the coordinator
writes and commits a bounded cycle plan, obtains a fresh independent `CLEAN`
planning review, assigns fresh workers serially, assigns a fresh task reviewer
after each worker, uses fresh worker and full rereviewer identities for every
defect correction, runs the cycle gate, obtains a distinct fresh holistic
`CLEAN` review, reruns the unchanged reviewed gate, and lands/publishes/reads back
the cycle before planning its successor from that immutable SHA.

The final cycle removes completed planning/review/evidence files while retaining
`plans/.gitkeep`; their Git history remains auditable. Publication is not proof by
itself: completion requires local `main`, `origin/main`, and immutable remote
readback to agree, plus an empty working tree.
