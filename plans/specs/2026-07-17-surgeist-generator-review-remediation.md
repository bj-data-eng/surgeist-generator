# Surgeist Generator Review Remediation Specification

## Header

- Repository: `/Users/codex/Development/surgeist-generator`.
- Status: `proposed`; no implementation may begin before this specification and
  its implementation sequence independently receive `CLEAN` planning reviews.
- Immutable implementation-series base:
  `1e545530fb992e2e749f26a1ba7ac0d77ed85dd6`.
- First planning artifact: committed baseline review
  `plans/2026-07-17-crate-baseline-review.md` at
  `05293743a551454adcf63345e80ef0d3982786b1`.
- Terminal outcome: every actionable baseline-review finding is resolved on a
  clean, remotely verified `surgeist-generator` `main`, ready for the user's new
  independent review.

## SR-01 Authority, Scope, And Product Decisions

### SR-01.1 Normative evidence and mutation envelope

The normative inputs are the user's current goal and original two-driver request,
the committed baseline review, repository `AGENTS.md`, the current
package/source/test tree, and this newly reviewed specification. Deleted planning
packets are historical provenance only. They do not waive a finding, authorize
sibling work, or supply missing normative behavior.

Only `surgeist-generator` may be mutated. The allowed surface is its manifest and
lockfile, library and binary source, focused synthetic tests, README and AGENTS,
and workflow planning/evidence files. Root `surgeist`, `surgeist-layout`,
`surgeist-css`, their corpora, and their artifacts remain outside mutation and
test scope. No sibling checkout is needed.

The following are non-goals:

- no root facade, gitlink, API-audit, or sibling integration change;
- no corpus, fixture, manifest, helper asset, generated XML, or expectation
  copied from a production crate;
- no real Chromium launch, Git clone/fetch, source import, or corpus generation
  during implementation verification;
- no browser/source downloader, archive extractor, or external-software
  acquisition path in the finished product;
- no dependency, target, toolchain, browser, or other software acquisition while
  implementing this series;
- no generalized plugin framework or open driver trait;
- no executable `unsafe` in repository-owned Rust;
- no normal-build dependency from a production Surgeist crate to this tooling
  crate.

The exact 4,626-line preservation copy at
`src/layout/legacy_generator.rs` remains immutable with SHA-256
`d2f5ca87cea6b36826e9172e2d7ba7a99196c375e2ca53f8a84a075200e70a9f`
until a reviewed layout task maps its retained responsibilities to compiled code
and focused tests. It is then removed, not retained as a second implementation.
Corpus-owned helper assets are loaded through the supplied corpus root.

### SR-01.2 Decision record and rejected alternatives

Both declared drivers are selected because the user's product request explicitly
requires two thin interfaces and the baseline finding identifies both inert
features as an incomplete product. Fixing only the shared core, deleting either
feature, or documenting either driver as deferred would leave that objective and
finding unresolved. The selected surface is additive and feature-gated; default
consumers retain the small shared library.

A generalized trait/plugin architecture is rejected because there are exactly
two known domains with materially different browser and fixture behavior.
Concrete modules produce a smaller API and keep heavy types private.

Managed browser and source acquisition is rejected. The cached
Chromiumoxide fetcher extracts untrusted archives through path/symlink-sensitive
code and does not provide the rooted, recoverable containment required here.
Building a new downloader/extractor would widen the objective and verification
surface. The layout driver therefore accepts an explicitly supplied, existing,
manifest-pinned browser executable; Taffy and CSSTree commands accept explicitly
supplied, existing Git checkouts. All three resources are verified read-only and
never installed, repaired, or removed.

Library request fronts are selected rather than binary-only behavior because
separate Cargo binary targets can consume only the library's public API. Exact
checked request types keep parsing out of the thin binaries and make the command
matrix testable without processes. No domain implementation type is public.

Canonical import sidecars are selected because offline corpus verification
cannot prove an imported tree's source revision from a mutable manifest alone.
The sidecars bind exact source pins and byte inventories without requiring the
external checkout during ordinary checking.

## SR-02 Acceptance And Finding Traceability

| Baseline finding | Required closure |
| --- | --- |
| Rooted lease-tree rejection | Missing intermediate components are ordinary absence only for existence queries; strict alias/type/mount/policy rejection remains; every lease/artifact test reaches its intended assertion. |
| Model-only crash/bootstrap tests | Test-only interruption drives the real bootstrap/install/recovery implementations for success, rollback, commit, cleanup, and lost-contended paths; actual visibility, residue, errors, and idempotence are asserted. |
| Missing domain surface | Real feature-owned layout and CSS modules plus both binary targets use shared leases/publication; features have exact edges; artificial linkage and the represented legacy copy are gone. |
| Quality matrix | Format, warning-denied native feature combinations, Clippy feature combinations, portable warning-denied default library, license, and offline advisory gates are clean. |
| Stale guidance | README and AGENTS describe the completed core/driver boundary, target support, features, corpus ownership, acquisition-free resource model, and checks. |
| Tautological CLI test | Real parser errors are `Cli`; both binaries are observed printing their prefix and exiting 64 for invalid syntax. |

No finding is closed by weakening an invariant, deleting a meaningful test,
silencing a warning, or describing unimplemented behavior as complete.

## SR-03 Namespace And Protection Contract

### SR-03.1 Owned and protected namespaces

Every command computes its complete namespace set before its first write.
`verify_git_source` continues to return its public source proof and additionally
retains a crate-private protection snapshot covering the canonical worktree,
per-worktree Git directory, common Git directory, primary object directory, and
recursive local alternate object directories. The snapshot records canonical
paths and held directory identities for closing revalidation; it does not widen
the public `VerifiedSource` API.

The command-specific namespaces are:

| Command/resource | Writable namespaces | Protected read-only namespaces |
| --- | --- | --- |
| Layout generate | `xml`; `.surgeist-generator` including journals, stages, lock files, and browser profiles | `corpus.toml`; `html`; both helper assets; validated Taffy sidecar/files; exact browser version root and executable |
| Layout import Taffy | `html`; `.surgeist-generator` including journals/stages | `corpus.toml`; helpers; manifest-classified authored HTML; complete Taffy source protection snapshot |
| Layout check Taffy | none | corpus inputs plus complete Taffy source protection snapshot |
| Layout check corpus | none | manifest, helpers, HTML/Taffy sidecar, XML, reports, and existing coordination state |
| CSS import CSSTree | `import_root`; `.surgeist-generator` including journals/stages | `corpus.toml`; `expectation_root`; complete CSSTree source protection snapshot |
| CSS generate | `expectation_root`; `.surgeist-generator` including journals/stages | `corpus.toml`; complete imported tree and source sidecar |
| CSS check corpus | none | manifest, imported tree/sidecar, expectations, report, and existing coordination state |

Transaction reservations named `._surgeist-*` are writable members of the same
corpus-root publication authority and are included even when not yet present.
Layout browser profiles live only beneath
`.surgeist-generator/profiles/layout/<lease-token>` and are created and removed
by the held rooted authority. No owner-global or system-temporary profile path is
used. The browser cache/version root is read-only.

### SR-03.2 Disjointness and revalidation

For each command, every writable namespace is compared with every protected
namespace and with every other writable final root. Equality, protected ancestor
of writable, writable ancestor of protected, and component-wise overlap are all
rejected. The browser version root must be outside the complete corpus root, not
only outside `xml`; import and expectation roots must be disjoint from each other,
`corpus.toml`, and `.surgeist-generator`.

The comparison has three layers:

1. checked normalized path components before filesystem access;
2. canonical paths for existing objects and nearest existing parents for absent
   suffixes;
3. descriptor ancestry identities `(device, inode, fsid)` in both directions to
   reject case aliases, symlink aliases, mounts, firmlinks, or other canonical
   spellings that share authority.

An absent suffix is represented as its nearest held existing parent plus exact
remaining components; failure to prove separation is `InvalidPath`. No candidate
alias, probe file, coordination directory, import root, profile, or stage is
created during this preflight.

Source verification and immutable snapshot construction are read-only. After
preflight, exclusive lease acquisition may create/recover only the already-proved
disjoint coordination namespace. While the domain mutex is held and before any
publication/profile write or external process launch, the command reopens every
protected directory without following links, requires its recorded identity,
and repeats the complete disjointness matrix through a crate-private protected
revalidation callback. A changed path or identity fails closed. The snapshot
bytes—not a reread checkout—feed import publication.

Existing browser validation performs the same canonical and identity checks on
the manifest version root and CLI executable, rejects any resolution outside the
version root, requires an executable regular file, and compares exact normalized
`--version` output with the manifest. It executes only that version command and
Chromium itself; it never writes the cache.

Synthetic tests cover both ancestor directions, missing suffixes, case/symlink
aliases, source object-store overlap, browser-cache/corpus overlap, replacement
between preflight and protected revalidation, and outside sentinels that remain
byte-identical.

## SR-04 Shared-Core Correction Contract

### SR-04.1 Rooted existence

`RootedFs::exists` is a non-mutating descriptor-relative query. If `statat`
reports `NOENT` at any component, it returns `Ok(false)`. Every existing
component still undergoes exact-name enumeration, no-follow opening, same-mount
validation, and held-identity/type/policy checks. A case alias, symlink, mount
change, non-directory intermediate, non-UTF-8 sibling, permission failure, or
inconclusive state remains an error.

Generic `open_parent`, mutation helpers, and exact-name validation remain strict.
The correction uses an existence-aware traversal; it never recognizes error
text or converts arbitrary traversal errors to false.

Tests cover a missing intermediate and missing leaf, prove neither query mutates,
retain alias/symlink/non-directory rejection, and rerun every lease/artifact test.

### SR-04.2 Install interruption matrix

The hard-coded `TransactionProtocol::crash_prefixes`, `CrashPrefix`, and
`RecoveredPrefix` model may remain only as supplementary ordering documentation.
Production `TransactionEngine::install` and `recover_all` share one real state
machine with an instance-scoped test-only interruption controller. Production
construction cannot select an interrupted return. A test interruption exits the
real state machine without same-process cleanup, drops handles, and leaves the
durable state at that exact boundary.

The install controller names these real boundaries:

| ID | Boundary completed before interruption |
| --- | --- |
| I01 | active journal directory created |
| I02 | intent published |
| I03 | old-inventory sidecar published |
| I04 | empty stage reservation created |
| I05 | stage-identity registration published |
| I06 | registered stage renamed into corpus root |
| I07 | staged files/directories populated and synced |
| I08 | new-inventory sidecar published |
| I09 | prepared marker published |
| I10 | final-root exclusive rename or swap completed |
| I11 | corpus-root directory synced after commit |
| I12 | committed outcome published |
| I13 | recorded losing tree removed |
| I14 | cleanup receipt and completed-journal name published |
| I15 | each receipt-bound member and final journal removed/synced |

Each boundary is exercised in two distinct matrices:

| Commit kind | Initial final root | I01-I09 visible before/after recovery | I10-I15 visible before/after recovery |
| --- | --- | --- | --- |
| Exclusive | absent | absent / absent | complete new / complete new |
| Swap | complete old tree | complete old / complete old | complete new / complete new |

No test claims that exclusive pre-commit recovery exposes an old tree. At each
prefix the harness uses real temporary files, drops and reopens `RootedFs`,
constructs a fresh engine, invokes production `recover_all`, verifies exact tree
bytes and journal inventory, invokes recovery again, and requires an identical
terminal state with no owned residue.

### SR-04.3 Recovery interruption matrix

The same controller can interrupt production recovery at:

| ID | Recovery boundary |
| --- | --- |
| R01 | active/completed journal identity and member inventory validated |
| R02 | missing old-inventory sidecar reconstructed and durably published |
| R03 | prepared/commit state classified from bound final/stage identities |
| R04 | aborted or committed outcome marker published |
| R05 | recorded losing stage/old tree removed |
| R06 | cleanup receipt and completed-journal name published |
| R07 | each receipt-bound member removal |
| R08 | final journal directory removal and parent sync |

For every reachable R-boundary, four seeds are used: exclusive pre-commit,
exclusive post-commit, swap pre-commit, and swap post-commit. The visible
generation remains respectively absent, new, old, and new before and after the
injected recovery interruption. The interrupted call returns only a private
test sentinel and retains a valid journal; a fresh unhooked `recover_all` returns
`Ok`, reaches the same terminal visibility, removes owned residue, and is
idempotent. Separately seeded corruption, unknown members, or identity
replacement returns `ArtifactTransaction`, preserves evidence it cannot safely
classify, and does not change the visible generation.

### SR-04.4 Bootstrap interruption matrix

Bootstrap tests execute real `open_or_bootstrap_lock`, state validation,
`recover_bootstrap`, and cleanup. A test-only liveness callback treats only the
synthetic abandoned owner as dead; production always uses the process probe.

Success-path boundaries are active journal creation, intent publication, empty
stage creation, stage-identity publication, every header byte prefix from zero
through the complete immutable header, complete-header sync, stage lock, final
rename, final-parent sync, cleanup receipt publication, each receipt-bound member
removal, and journal removal. Incomplete header prefixes are produced through the
real stage handle under the test controller. Recovery may remove an incomplete
recorded stage but may publish only the exact complete header. Every prefix ends
with one complete lock or clean absence as dictated by whether final rename
occurred, no journal, and idempotent repeated recovery.

The lost-contended branch is exercised with an independently held valid final
lock. Interruptions occur after the losing stage is released but before its
marker, after `lost-contended` is published, after recovery claim rename, after
cleanup receipt, and during member removal. Recovery validates the winning
lock's bound identity/header, never removes or replaces it, removes only the
recorded losing stage/journal, returns `LeaseActive` while the winner remains
held, and is clean/idempotent after release. Corruption and live-owner cases
remain errors with evidence preserved.

### SR-04.5 Core ownership and quality

`ArtifactPlan`, `PublicationInventory`, `PublicationPolicy`, `GenerationLease`,
`GenerationCheck`, and `Domain` remain crate-private. Real domain modules call
them. `private_front_doors_are_linked` and artificial function references are
removed. The identity `map_err`, target-inappropriate imports/helpers, and
rustfmt deltas are corrected after behavioral tests pass. Target-specific code is
gated at its narrowest boundary; default public contracts remain additive.

## SR-05 Package, Dependencies, And Exact Public API

### SR-05.1 Feature and dependency matrix

The package remains version 0.1.0, edition 2024, Rust 1.97, MIT,
`default = []`, and preserves exact shared dependencies and target-specific
`rustix = 1.1.4`.

The exact new manifest entries are:

```toml
[features]
default = []
layout-browser = ["dep:chromiumoxide", "dep:futures", "dep:tokio", "dep:url"]
css-corpus = []

[dependencies]
chromiumoxide = { version = "=0.9.1", default-features = false, features = ["bytes"], optional = true }
futures = { version = "=0.3.31", optional = true }
tokio = { version = "=1.48.0", features = ["fs", "io-util", "macros", "process", "rt-multi-thread", "sync", "time"], optional = true }
url = { version = "=2.5.7", optional = true }

[[bin]]
name = "surgeist-layout-generate"
path = "src/bin/surgeist-layout-generate.rs"
required-features = ["layout-browser"]

[[bin]]
name = "surgeist-css-generate"
path = "src/bin/surgeist-css-generate.rs"
required-features = ["css-corpus"]
```

No `chromiumoxide` fetcher/TLS/zip feature and no downloader dependency is
enabled. The four exact direct sources and their transitive index entries are
already cached. Their declared MSRVs are at or below 1.97. Direct licenses are
MIT or include an MIT-compatible alternative; the complete resolved graph must
pass the offline license gate. The tracked lockfile is refreshed only with
`cargo generate-lockfile --offline`, and failure to resolve exclusively from the
cache stops the dependency task rather than permitting network.

The layout feature has intentional compile-time/binary-size cost from the browser
protocol/runtime graph; isolation behind `required-features` keeps that graph out
of default and CSS-only builds. No compiled-size budget or measurement tool is
configured, so no new tool is introduced. Coordination/runtime cost is one
domain mutex, one private runtime thread, one handler task per browser, bounded
batches from the manifest, and one profile per active browser; all are terminal
before return. CSS adds no dependency or runtime thread.

The installed advisory database is offline and may be stale; `cargo audit
--no-fetch --stale` is a fail-on-reported-advisory gate, not a claim of current
online security. Any license/advisory/MSRV conflict stops the task for plan
correction.

### SR-05.2 Exact layout API

Only with `layout-browser`, `lib.rs` exposes `pub mod layout` with exactly:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum LayoutCommand {
    Generate,
    CheckCorpus,
    CheckTaffyCorpus,
    ImportTaffy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LayoutRequest { /* private fields */ }

impl LayoutRequest {
    pub fn new(
        location: CorpusLocation,
        command: LayoutCommand,
        browser_path: Option<RelativePath>,
        source_root: Option<std::path::PathBuf>,
        filter: Option<RelativePath>,
    ) -> Result<Self>;
    pub const fn location(&self) -> &CorpusLocation;
    pub const fn command(&self) -> LayoutCommand;
    pub const fn browser_path(&self) -> Option<&RelativePath>;
    pub fn source_root(&self) -> Option<&std::path::Path>;
    pub const fn filter(&self) -> Option<&RelativePath>;
}

pub fn run(request: LayoutRequest) -> Result<()>;
pub fn run_from_env() -> Result<()>;
```

The private fields correspond one-for-one to constructor arguments. The
constructor performs no filesystem access. It rejects an empty source path and
enforces this exact matrix as `Cli`:

| Command | browser_path | source_root | filter |
| --- | --- | --- | --- |
| Generate | required | forbidden | optional |
| CheckCorpus | forbidden | forbidden | forbidden |
| CheckTaffyCorpus | forbidden | required | forbidden |
| ImportTaffy | forbidden | required | forbidden |

### SR-05.3 Exact CSS API

Only with `css-corpus`, `lib.rs` exposes `pub mod css` with exactly:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CssCommand {
    ImportCsstree,
    Generate,
    CheckCorpus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CssRequest { /* private fields */ }

impl CssRequest {
    pub fn new(
        location: CorpusLocation,
        command: CssCommand,
        source_root: Option<std::path::PathBuf>,
        filter: Option<RelativePath>,
    ) -> Result<Self>;
    pub const fn location(&self) -> &CorpusLocation;
    pub const fn command(&self) -> CssCommand;
    pub fn source_root(&self) -> Option<&std::path::Path>;
    pub const fn filter(&self) -> Option<&RelativePath>;
}

pub fn run(request: CssRequest) -> Result<()>;
pub fn run_from_env() -> Result<()>;
```

The private fields correspond to the arguments. The constructor is I/O-free,
rejects an empty source path, and enforces:

| Command | source_root | filter |
| --- | --- | --- |
| ImportCsstree | required | forbidden |
| Generate | forbidden | optional |
| CheckCorpus | forbidden | forbidden |

These feature-gated additions are additive. Request structs intentionally have
only the shown traits; enums are non-exhaustive so new commands are not a
breaking match promise. No Tokio, Chromiumoxide, JSON value, descriptor, lease,
or transaction type appears publicly. Public items have rustdoc with an
acquisition-free example; feature-specific public API tests assert construction,
traits, accessors, invalid matrices, and examples.

### SR-05.4 CLI and supervisor boundary

The binaries are each at most fifteen physical lines. They call the corresponding
`run_from_env`, print exactly `surgeist-layout-generate: <error>` or
`surgeist-css-generate: <error>` once, and exit only through
`GeneratorError::exit_code`.

`run_from_env` reads `args_os` and no configuration environment variable. Flag
and command names must be UTF-8; owner/corpus/source roots retain OS-native path
bytes; browser/filter values must convert to checked UTF-8 `RelativePath`.
Unknown/duplicate flags, missing values, repeated positionals, invalid command,
or command-option mismatch returns `Cli` before domain I/O. Filesystem absence,
canonicalization, identity, manifest, and source failures retain their domain
error kinds.

Layout `run` spawns one named private worker thread. Spawn or Tokio runtime-build
failure is `Generation` before resource acquisition. The worker owns a
multi-thread Tokio runtime and a terminal-resource registry. External-input
errors never panic. An unexpected internal panic is caught inside the worker,
all registered browser/handler/profile/lease resources are terminalized
idempotently, and the original panic payload is resumed on the caller after
join; it is not mislabeled as success or an input error. A normal join returns
the semantic result only after cleanup. CSS `run` is synchronous and threadless.

## SR-06 Layout Domain Contract

### SR-06.1 Exact commands and schema-2 manifest

The CLI is:

```text
surgeist-layout-generate --owner-root <path> --corpus-root <path> \
  <generate|check-corpus|check-taffy-corpus|import-taffy> \
  [--browser-path <owner-relative-path>] [--source-root <path>] \
  [--filter <html-relative-path>]
```

Its option matrix is SR-05.2. There is no `generate-existing` distinction and no
managed acquisition: the one generate command always requires the existing
browser path.

The exact schema-2 TOML object structure is:

```toml
schema_version = 2

[browser]
source = "chrome-for-testing"
version = "<nonempty pinned version>"
version_output = "<exact normalized --version output>"
cache_root = "<strict owner-relative directory>"
repository_relative_executable = "<strict path below cache_root/version>"
tree_inventory_sha256 = "<canonical complete version-root inventory digest>"
provenance_format = "<contains {version} and {repository_relative_executable}>"

[browser.launch]
batch_size = 1
navigation_timeout_ms = 1
dom_poll_interval_ms = 1
retry_count = 0
job_order = "manifest"
retry_error_class = "transport"
profile_scope = "browser"
page_scope = "job"
disable_default_args = false
disable_cache = true
arguments = []

[generation_reports.full]
file = "all.json"
generated = 0
unsupported = 0
expected_fail = 0
quarantined = 0
failed_to_generate = 0

[[generation_reports.scoped]]
filter = "<normalized html-relative prefix>"
file = "<one-component .json name>"
generated = 0

[source_roots.taffy]
kind = "upstream"
path = "html"
upstream_commit = "<same exact revision as imports.taffy.commit>"
description = "<nonempty trimmed text>"

[source_roots.surgeist]
kind = "local"
path = "html"
description = "<nonempty trimmed text>"

[imports.taffy]
repo = "https://github.com/DioxusLabs/taffy.git"
commit = "<exact lowercase 40- or 64-hex revision>"
source_dir = "<strict source-relative directory>"
destination = "html"
expected_count = 1
excluded_destination_dirs = []

[[cases]]
id = "<unique canonical id>"
source_root = "surgeist"
source = "<strict .html path below html>"
generator = "constrained-html"
status = "active"
# reason exists only for non-active status
```

All objects deny unknown/duplicate fields. Counts, durations, and batch size are
bounded positive values except retry count and disposition counts may be zero.
Paths are strict normalized relative paths. `cache_root` resolves beneath owner
and outside corpus; report files are unique one-component JSON names; full is
exactly `all.json`; scoped filters/files are unique. Taffy and Surgeist share the
`html` physical root but have disjoint manifest ownership. Every authored
Surgeist file appears exactly once in `cases`; imported Taffy files come only
from the verified sidecar. Unknown HTML entries fail inventory validation.

The CLI browser path must equal
`<cache_root>/<version>/<repository_relative_executable>` exactly. The complete
version-root inventory accepts only directories, regular files, and relative
symlinks whose normalized targets remain inside that root; it rejects hard-linked
regular files, duplicate/case-aliased names, special files, absolute/parent
symlink targets, cycles, and mount changes. Canonical inventory bytes sort strict
relative paths and record node kind, normalized mode, link target or raw-file
SHA-256; their SHA-256 must equal `tree_inventory_sha256` before either the
version command or Chromium runs. The executable must be a single-link regular
executable at the manifest path. Closing revalidation repeats root/executable
identity and tree digest after lease acquisition and before launch.

### SR-06.2 Taffy import and offline proof

`import-taffy` verifies the explicit source checkout against the manifest pin,
takes an immutable byte snapshot of regular `.html` files below `source_dir`,
requires `expected_count`, and constructs canonical
`html/.surgeist-taffy-source.json`. Its compact JSON plus final LF contains, in
order: `schema_version: 1`, canonical repository/revision/source directory,
object format, file count, and strictly sorted records of relative path, Git
mode `100644`, blob object ID, and SHA-256.

The exact sidecar shape is:

```json
{"schema_version":1,"source":{"label":"taffy","repository_url":"https://github.com/DioxusLabs/taffy.git","revision":"<full-object-id>","source_subdirectory":"<manifest source_dir>"},"object_format":"sha1","file_count":1,"files":[{"path":"relative.html","git_mode":"100644","blob_object_id":"<full-blob-id>","sha256":"<raw-byte-sha256>"}]}
```

The current `html` publication inventory admits only:

- the reserved sidecar;
- Taffy files listed by the old validated sidecar, when present;
- manifest-owned Surgeist `.html` files;
- directories implied by those exact files.

Before publication every Surgeist file is read through the held rooted authority
and retained byte-for-byte. A missing/replaced authored file or any unknown entry
fails without mutation. The desired clean-full tree is the new Taffy snapshot,
new sidecar, and byte-identical authored files. Thus stale old Taffy files are
removed atomically, authored fixtures cannot be deleted or rewritten, and no
report/XML is touched. `check-taffy-corpus` performs the same source comparison
read-only. `check-corpus` requires only the persisted sidecar and verifies its
pin/count/digests against manifest and HTML without Git.

### SR-06.3 Browser measurement, XML, and reports

Generate reads corpus-owned `scripts/gentest/test_helper.js` and
`scripts/gentest/test_base_style.css`, hashes and injects them through the
supplied corpus root, and retains the preserved implementation's deterministic
HTML document/base-URL handling, DOM-ready polling, grid-template-area capture,
retry classification, browser batching, and cleanup. It never embeds those
assets in this crate.

For each selected `html/<group>/<stem>.html`, measurement requires the four exact
keys and maps them to:

- `borderBoxLtrData` -> `xml/<group>/<stem>__border_box_ltr.xml`;
- `contentBoxLtrData` -> `xml/<group>/<stem>__content_box_ltr.xml`;
- `borderBoxRtlData` -> `xml/<group>/<stem>__border_box_rtl.xml`;
- `contentBoxRtlData` -> `xml/<group>/<stem>__content_box_rtl.xml`.

The deterministic XML renderer retains the preserved element/style/layout
mapping and generated-by provenance comment. Provenance includes schema 2,
`html/...` source and digest, linked-resource inventory, helper/base-style
digests, exact browser provenance, launch-profile digest, manifest digest, Taffy
revision, and Taffy-sidecar digest. Unsupported measured variants have no XML and
are reported with reason.

Full generation serializes all manifest-declared report files beneath
`xml/generation-reports/`. `all.json` has exactly `metadata`, `filter: null`,
`summary`, `generated`, `unsupported`, `expected_fail`, `quarantined`, and
`failed_to_generate` in that order. Metadata contains schema/generator, browser
source/version/provenance, launch/helper/base-style/manifest/Taffy-sidecar
digests, and Taffy revision. Entry shapes retain the preserved
name/source/output/variant/reason fields. Each scoped report has its declared
filter, contains the deterministic subset of the full result, and matches its
manifest count. Pretty JSON has one final LF.

The publication matrix is exact:

| Run outcome | Policy/result | Artifacts | Stale/unknown behavior |
| --- | --- | --- | --- |
| Full, every browser job reaches a classified generated/unsupported result and no lifecycle failure | `CleanFull`, then `Ok` | all generated XML plus every full/scoped report | remove only manifest-classified nonretained XML/report paths; unknown entry fails before intent |
| Full, one or more jobs exhaust retry but supervisor reaches terminal cleanup and a complete diagnostic report exists | `DiagnosticFull`, install, then return `Generation` | successful XML plus all diagnostic reports with failed entries | preserve every existing unmatched classified XML/report; unknown entry fails |
| Filtered, all selected jobs succeed/classify | `Filtered`, then `Ok` | selected generated XML only | write no report; preserve every other XML/report; remove nothing |
| Filtered job failure, validation failure, browser/version failure, panic cleanup failure, or incomplete report | no plan/install; return semantic error | none | byte-identical generated tree |

Classified XML is the four exact paths for every manifest-admitted HTML fixture;
retained XML is only successfully generated variants. Classified reports are
exactly manifest-declared full/scoped paths. Report paths are retained only in a
complete full/diagnostic report set. Unsupported/quarantined paths are therefore
removed only by successful clean full publication. Filtered publication cannot
write reports. Check commands acquire/finish a shared guard, create/recover
nothing, and verify exact inventory, hashes, report relationships, XML
provenance, and absence of unknown entries.

Browser-independent synthetic adapters and byte goldens cover HTML injection,
URLs, four-variant mapping, measurement conversion, XML, reports, retry,
clean/diagnostic/filtered matrices, profile cleanup, and CLI errors. No test
launches or downloads a browser or reads a sibling corpus.

### SR-06.4 Preservation retirement

Layout evidence records the preservation digest and maps each retained behavior
to compiled code/test or a corpus-owned input. The intentionally rejected managed
fetch/import and legacy environment/global-lock/direct-write mechanisms are
listed as replaced by this specification, not silently copied. Only after that
mapping, layout feature/API/binary tests, and full synthetic behavior tests pass
is `legacy_generator.rs` deleted in the reviewed layout task.

## SR-07 CSS Domain Contract

### SR-07.1 Exact commands and schema-1 manifest

The CLI is:

```text
surgeist-css-generate --owner-root <path> --corpus-root <path> \
  <import-csstree|generate|check-corpus> \
  [--source-root <path>] [--filter <import-relative-json-or-prefix>]
```

Its option matrix is SR-05.3. The exact manifest shape is:

```toml
schema_version = 1

[source]
kind = "csstree"
repository = "https://github.com/csstree/csstree.git"
revision = "<exact lowercase 40- or 64-hex revision>"
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
reason = "<nonempty trimmed reason>"
```

Objects deny unknown/duplicate fields. `kind` is exactly `csstree`; repository is
canonical HTTPS Git; revision is exact; counts are positive; roots are strict;
import/expectation roots are distinct one-component names; report path is exactly
`<expectation_root>/generation-reports/all.json`. Case IDs are unique; active has
no reason and all other statuses require one. Overrides must match exactly one
derived case. A derived case without an override defaults to active.

### SR-07.2 Import sidecar and publication

Import verifies the explicit checkout and immutable snapshot under
`fixture_root`, accepts only regular Git mode `100644` JSON, requires exact file
count, and rejects the reserved root `.surgeist-source.json`. The canonical
sidecar contains schema 1, full source pin, object format, exact file count, and
sorted path/Git-mode/blob-ID/SHA-256 records. It and snapshot bytes are published
as one clean-full `import_root` transaction.

The exact compact sidecar plus final LF is:

```json
{"schema_version":1,"source":{"label":"csstree","repository_url":"https://github.com/csstree/csstree.git","revision":"<full-object-id>","source_subdirectory":"fixtures/ast"},"object_format":"sha1","file_count":1,"files":[{"path":"declaration/Declaration.json","git_mode":"100644","blob_object_id":"<full-blob-id>","sha256":"<raw-byte-sha256>"}]}
```

`object_format` is exactly `sha1` or `sha256` and fixes revision/blob-ID widths;
all object IDs and SHA-256 values are lowercase full-width hex. File records are
strictly increasing and unique.

Current import classification comes only from the old validated sidecar plus its
listed files. Unknown entries fail before intent. Desired retention is exactly
the new sidecar and snapshot, so stale old fixture files disappear atomically.
No expectation/report changes. Closing source revalidation follows SR-03.

### SR-07.3 Neutral expectation schema

Each imported `<import_root>/<relative>.json` maps exactly to
`<expectation_root>/<relative>.json`. Its pretty JSON plus final LF has this field
order and shape:

```json
{
  "schema_version": 1,
  "generator": "surgeist-css-generate",
  "source": "source/declaration/Declaration.json",
  "source_sha256": "<64 lowercase hex>",
  "source_revision": "<exact manifest/sidecar revision>",
  "import_provenance_sha256": "<sidecar digest>",
  "cases": [
    {
      "id": "declaration/Declaration.json#/label",
      "context": "declaration",
      "label": "label",
      "input": "a { color: red }",
      "options": {},
      "upstream_outcome": "parsed",
      "canonical_css": "a{color:red}",
      "status": "active"
    }
  ]
}
```

`label`, `options`, `canonical_css`, and `reason` are omitted when absent;
`reason` exists exactly for non-active status. Ordinary top-level case objects
require string `source` and any `ast`; optional `options` must be an object and
optional `generate` a string. `error` is an optional array of objects with string
`source`; its cases omit label/options/canonical CSS and use outcome `rejected`.
Other ordinary cases use outcome `parsed`. `context` is the first component of
the fixture-relative path. IDs use decoded-label JSON Pointer escaping (`~` to
`~0`, `/` to `~1`) or `/error/<index>`.

AST, upstream diagnostic prose, offsets, comments, and recovery structures are
never copied. A streaming prepass rejects duplicate decoded object members at
every depth and trailing values before typed parsing. Objects in preserved
options are recursively sorted by decoded Unicode-scalar key, arrays retain
order, and scalar JSON serialization is Serde canonical output. Malformed shape,
empty derived case set, duplicate ID, unmatched override, or full count mismatch
is `InvalidInventory` before publication.

### SR-07.4 CSS reports and publication

The full report is the existing shared `GenerationReport` schema serialized as
pretty JSON plus final LF at the exact manifest report path. It binds manifest
digest, sidecar repository/revision, disposition counts, and one sorted
`ReportArtifact` per expectation. Each artifact uses:

- source path `<import_root>/<relative>.json` and its digest;
- generator `surgeist-css-generate`;
- schema version 1;
- exact domain provenance map
  `{"csstree-import": <sidecar SHA-256>}`;
- output path `<expectation_root>/<relative>.json`, output digest, and positive
  derived case count.

Counts classify every derived case exactly once as active, expected-fail,
unsupported, or quarantined; deterministic CSS ingestion has no recoverable
per-case failure, so `failed_to_generate` is zero. Any fixture failure aborts the
whole run without publication.

The publication matrix is:

| Command/outcome | Policy | Exact retained set |
| --- | --- | --- |
| Full generate success | `CleanFull` | one expectation per sidecar-listed fixture plus the one report |
| Filtered generate success | `Filtered` | selected expectations only; no report write/removal and no stale removal |
| Any validation/derivation/publication failure | no install | current expectation tree byte-identical |

Current classification is expectations mapped from the validated import sidecar,
the manifest report, and directories implied by them. Any other entry fails
before intent. A full clean run removes stale expectations that were classified
by the previous valid report/sidecar but are absent from the new exact set; it
never guesses ownership from extension alone. `check-corpus` performs no Git or
mutation and validates manifest, sidecar/files, every expectation byte/schema,
counts, report relationship, hashes, and exact inventories.

Byte-golden tests cover ordinary/error cases, escaping, canonical options,
default/override dispositions, repeated source paths with unique IDs, duplicate
members, malformed/empty fixtures, sidecar drift, report provenance/counts,
unknown entries, stale full removal, and filtered preservation.

## SR-08 Errors, Documentation, And Verification

### SR-08.1 Errors and CLI proof

The existing non-exhaustive `GeneratorErrorKind` set remains stable. Malformed
syntax/option matrices map to `Cli`; manifest schema to `InvalidManifest`;
fixture/current-tree shape to `InvalidInventory`; pin/snapshot drift to
`SourceVerification`; unsupported mutation target to `UnsupportedPlatform`;
lease/process/transaction/generation/check failures retain their existing kinds.
Safe I/O/process sources are preserved. External input never asserts or panics.

Focused parser tests construct real `Cli` errors and require exit code 64. Each
binary integration test invokes invalid syntax, observes only its exact prefixed
diagnostic on stderr, no stdout, and status 64 without touching a corpus or
browser. The tautological public test is replaced by a real error-path assertion.

### SR-08.2 Documentation

After both drivers are real, README and AGENTS describe the small default core,
exact feature/binary matrix, explicit roots, acquisition-free resource model,
layout browser/HTML/XML responsibility, CSS CSSTree/neutral responsibility,
corpus ownership, Apple-Silicon macOS mutation support versus portable default
value/read compilation, offline checking, and the fact that production crates do
not normally depend on this tooling crate. They do not call the crate a scaffold
or claim sibling integration completed.

### SR-08.3 Final offline matrix

The candidate uses only installed tooling/caches. Warning-denied native checks
cover every supported feature combination:

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
cargo clippy --locked --offline -p surgeist-generator --no-default-features --all-targets -- -F unsafe-code -D warnings
cargo clippy --locked --offline -p surgeist-generator --features layout-browser --all-targets -- -F unsafe-code -D warnings
cargo clippy --locked --offline -p surgeist-generator --features css-corpus --all-targets -- -F unsafe-code -D warnings
cargo clippy --locked --offline -p surgeist-generator --all-features --all-targets -- -F unsafe-code -D warnings
RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --target wasm32-unknown-unknown --no-default-features --lib
cargo fmt --check
cargo deny --all-features --locked --offline list --format tsv --layout license
cargo audit --no-fetch --stale
```

Feature tests are synthetic and cannot launch/download Chromium, execute a source
clone/fetch, or read siblings. Final evidence records the owned-Rust executable
unsafe scan, exact preservation digest/retirement map, baseline-finding closure
table, license/advisory output and database staleness, clean status, and immutable
remote readback.

## SR-09 Initiative Constraints And Handoff

The implementation sequence must allocate each SR section and baseline finding
to exactly one bounded closure cycle, keep shared-core correction ahead of domain
use, keep layout preservation retirement inside layout closure, and leave final
documentation/plan cleanup until both features are executable. The
`surgeist-agent` skill is the sole execution and publication authority; this
specification does not restate or redefine that gate.

The terminal tree retains canonical `plans/.gitkeep` and removes completed
planning/review/evidence files after their Git history has been reviewed. The
handoff is the immutable published leaf SHA and exact feature/command/verification
contract; root and sibling adoption remain separate future work.
