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
lockfile, repository-local `deny.toml`, library and binary source, focused
synthetic tests, README and AGENTS, and workflow planning/evidence files. Root
`surgeist`, `surgeist-layout`, `surgeist-css`, their corpora, and their artifacts
remain outside mutation and test scope. No sibling checkout is needed.

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
surface. The layout driver therefore accepts an explicitly supplied, existing
browser executable as a trusted external capability and checks only its identity,
cache containment, executable type, and manifest version output. Those checks do
not prove the program benign. Taffy and CSSTree commands accept explicitly
supplied, existing Git checkouts whose bytes and provenance are verified
read-only. None of the resources is installed, repaired, or removed.

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

Every command computes its complete generator-owned namespace set before its
first write.
`verify_git_source` continues to return its public source proof and additionally
retains a crate-private protection snapshot covering the canonical worktree,
per-worktree Git directory, common Git directory, primary object directory, and
recursive local alternate object directories. The snapshot records canonical
paths and held directory identities for closing revalidation; it does not widen
the public `VerifiedSource` API.

The command-specific namespaces are:

| Command/resource | Writable namespaces | Protected read-only namespaces |
| --- | --- | --- |
| Layout generate | `xml`; `.surgeist-generator` including journals, stages, lock files, and browser profiles | `corpus.toml`; `html`; both helper assets; validated Taffy sidecar/files; complete browser cache root and exact executable |
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
used. The complete browser cache root is read-only.

### SR-03.2 Disjointness and revalidation

For each command, every generator-owned writable namespace is compared with every
protected namespace and with every other writable final root. Equality, protected
ancestor of writable, writable ancestor of protected, and component-wise overlap
are all rejected. The browser cache root must be outside the complete corpus
root, not only outside `xml`; import and expectation roots must be disjoint from
each other, `corpus.toml`, and `.surgeist-generator`.

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

Source verification and immutable snapshot construction are read-only and launch
no external resource executable. After
preflight, exclusive lease acquisition may create/recover only the already-proved
disjoint coordination namespace. While the domain mutex is held and before any
publication/profile write or external process launch, the command reopens every
protected directory without following links, requires its recorded identity,
and repeats the complete disjointness matrix through a crate-private protected
revalidation callback. A changed path or identity fails closed. The snapshot
bytes—not a reread checkout—feed import publication.

Existing browser validation performs the same canonical and identity checks on
the manifest cache root and CLI executable, rejects any resolution outside the
cache, and requires an executable regular file. Only after lease acquisition and
the complete closing revalidation does it create the rooted profile hierarchy,
execute `<browser> --version` with the fixed environment below, compare the exact
normalized output with the manifest, and launch Chromium with that same
environment. The trusted executable capability can write or spawn outside the
generator-owned namespaces; this specification does not claim to sandbox or
contain it. Operator trust in that exact executable is a command precondition,
and README/rustdoc must state the boundary.

The preserved launch profile is additionally constrained to 28 unique printable
ASCII arguments, one exactly `use-mock-keychain`, with no slash, backslash,
control byte, or NUL. After stripping an optional leading `--` and text after the
first `=`, these keys are forbidden: `user-data-dir`, `disk-cache-dir`,
`data-path`, `log-file`, `crash-dumps-dir`, `download-default-directory`,
`remote-debugging-port`, `remote-debugging-address`, `load-extension`, and
`disable-extensions-except`. The driver alone supplies its rooted user-data
profile and Chromiumoxide transport arguments. This rejects known configuration
redirection but is not presented as a security boundary against a malicious
binary.

The browser environment is not configurable. Both version and measurement
commands override `HOME`, `TMPDIR`, `TMP`, `TEMP`, `XDG_CONFIG_HOME`,
`XDG_CACHE_HOME`, and `XDG_DATA_HOME` with precreated directories beneath the
lease profile; override `TZ`, `LANG`, and `LC_ALL` with `UTC`, `C`, and `C`;
override `HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`, `http_proxy`, `https_proxy`,
and `all_proxy` with the empty string; override `NO_PROXY` and `no_proxy` with
`*`; and set no manifest- or CLI-supplied environment entry. Before the lease,
capability validation rejects
`InvalidPath` if the inherited environment contains a key beginning `DYLD_`,
`LD_`, `CHROME_`, or `CHROMIUM_`, or an exact key `SSLKEYLOGFILE`,
`FONTCONFIG_FILE`, `FONTCONFIG_PATH`, `GOOGLE_API_KEY`,
`GOOGLE_DEFAULT_CLIENT_ID`, or `GOOGLE_DEFAULT_CLIENT_SECRET`. Chromiumoxide
inherits other process entries because it has no environment-clear interface;
that residual OS context is explicitly part of the trusted external capability,
not a filesystem-containment or malicious-executable defense. README/rustdoc
state this residual boundary and the generator reads no environment override as
configuration.

Synthetic tests cover both ancestor directions, missing suffixes, case/symlink
aliases, source object-store overlap, browser-cache/corpus overlap, replacement
between preflight and protected revalidation, argument/environment rejection,
the exact fixed environment map, and outside sentinels that remain byte-identical
after generator-owned operations. They do not make a containment claim about the
trusted executable, which tests never launch.

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

### SR-04.2 Exhaustive real-prefix harness

The hard-coded `TransactionProtocol::crash_prefixes`, `CrashPrefix`, and
`RecoveredPrefix` model may remain only as supplementary ordering documentation.
The executable proof instruments the production `RootedFs`,
`TransactionEngine::install`, `TransactionEngine::recover_all`, bootstrap, and
cleanup paths with an instance-scoped `#[cfg(test)]` observer. Production
construction has no observer and no interrupted return.

The observer emits one event after every recovery-distinct mutation or
durability operation, not only after high-level phases. Events contain a stable
phase, primitive, strict relative path, and per-phase ordinal. They include:

- every directory creation;
- every temporary-file create, partial/full write, file sync, publication rename,
  and containing-directory sync inside `publish_file_exclusive`;
- every stage directory and individual staged-file creation and each deepest-
  first directory sync;
- every exclusive/swap/claim/completed-journal rename and each parent sync;
- every individual receipt-bound file/directory removal, receipt removal,
  journal removal, and parent sync.

An unhooked run first records the complete ordered trace for the same fixture.
The harness then reruns from a fresh fixture once for every trace prefix and
interrupts immediately after event `n`. This parameterization is the normative
prefix set; phase tables are summaries and cannot collapse individual entries,
temporary publications, receipts, renames, removals, or syncs. The nested fixture
contains at least two directories, three files, an old-only file, a new-only
file, and a replaced file so all per-entry branches execute.

The private interruption sentinel bypasses same-process recovery, drops held
handles, and is never a `GeneratorError` available to production. Each prefix is
reopened through a fresh `RootedFs`; actual tree bytes and the complete journal/
temporary/stage/reservation/receipt inventory are asserted.

### SR-04.3 Install and recovery oracles

Install traces cover both commit kinds:

| Commit kind | Initial final root | Prefix before commit event | Prefix at/after commit event |
| --- | --- | --- | --- |
| Exclusive | absent | final remains absent | complete new is visible |
| Swap | complete old | complete old is visible | complete new is visible |

After every install prefix, a fresh unhooked `recover_all` returns `Ok`, preserves
the applicable visibility, removes all owned residue, and a repeated call makes
no change. No test claims an old generation for exclusive pre-commit state.

Recovery itself is traced and exhaustively interrupted for four seeds:
exclusive pre-commit, exclusive post-commit, swap pre-commit, and swap
post-commit. The trace includes every old-sidecar reconstruction suboperation,
commit-state classification boundary, outcome temporary/publication, every
individual losing-tree removal, cleanup-receipt temporary/publication,
active-to-completed rename, each receipt-bound member removal, receipt removal,
journal removal, and directory sync. Visibility remains respectively absent,
new, old, and new at every interrupted recovery prefix. The interrupted call
returns only the test sentinel and leaves a state accepted by another fresh
production `recover_all`; that call returns `Ok`, reaches no owned residue, and
is idempotent.

Separately seeded corruption, unknown members, or identity replacement returns
`ArtifactTransaction`, preserves the evidence it cannot safely classify, and
does not change the visible generation. A post-commit operational or cleanup
failure also returns `ArtifactTransaction` while retaining the complete new
generation and any valid resumable journal.

### SR-04.4 Bootstrap success and contention oracles

Bootstrap uses the same exhaustive primitive trace over real
`open_or_bootstrap_lock`, state validation, `recover_bootstrap`, and receipt
cleanup. A test-only liveness callback treats only the synthetic abandoned owner
as dead; production always uses the process probe. In addition to the primitive
trace, the header writer is rerun for every byte prefix from zero through the
complete immutable header. Recovery may remove an incomplete recorded stage but
may publish only the exact complete header.

Three branches are distinct:

1. **Uncontended publish:** every primitive/header prefix recovers to clean
   absence before final rename or one complete final lock at/after rename;
   `recover_bootstrap` returns `Ok`, leaves no journal, and is idempotent.
2. **Winner held:** a hook publishes and exclusively holds an independently valid
   final lock immediately before the local exclusive rename. The local
   `open_or_bootstrap_lock(..., Exclusive)` releases its stage, durably publishes
   `lost-contended`, cleans when possible, and returns `LeaseActive`. If an
   interruption leaves the losing journal, `recover_bootstrap` returns `Ok` even
   while the winner is held, preserves the winner identity/header, removes only
   receipt-bound losing state, and is idempotent. A later ordinary acquisition
   after winner release returns `Ok` on that same lock.
3. **Winner released/adopted:** the hook publishes a valid final lock without
   holding it before the local rename. The local rename loses, then
   `open_or_bootstrap_lock(..., Exclusive)` acquires the winner, cleans the losing
   stage/journal, and returns `Ok` with the winner handle. Every primitive prefix
   of that adoption/cleanup branch recovers without replacing the winner.

The primitive trace explicitly includes stage release, lost-marker temporary and
publication, recovery-claim rename, cleanup-receipt temporary and publication,
each member removal, receipt removal, journal removal, and all parent syncs.
Corruption and a genuinely live non-relinquished owner remain errors with
evidence preserved.

### SR-04.5 Shared publication state and error matrix

Every domain mutation command uses these exact `ArtifactPlan` semantics. In this
section, **absent** means the final publication root does not exist; **current**
means its structure, provenance, digests, and report relationships all match the
currently validated inputs; **stale** means every entry is classifiable but at
least one required artifact is absent or has old provenance, bytes, digest, or
report linkage; and **invalid** means at least one entry cannot be classified by
the old validated inventory.

| Initial publication | Command form | Successful result |
| --- | --- | --- |
| final root absent | full import/generate | exclusive commit of one complete desired root |
| structurally valid current root | full import/generate | swap to one complete desired root |
| classifiable but stale current root | full import/generate | swap to the new complete root; only classified stale entries disappear |
| unknown/unclassifiable entry | any mutation | `InvalidInventory` before transaction intent; final tree unchanged |
| final root absent | filtered generate | `Verification`; no partial corpus is established |
| structurally classifiable current root | filtered generate | selected artifacts updated; reports and every other entry preserved |

The exact command-level state transitions are:

| Command | Admitted initial publication state | Successful return and resulting state | Required follow-up |
| --- | --- | --- | --- |
| Layout `import-taffy` | manifest-authored HTML is exact; the Taffy-owned portion is absent/current/stale; downstream XML is absent/current/stale | `Ok`; HTML becomes current. If the canonical Taffy sidecar digest changed, existing classifiable downstream becomes stale and absent downstream remains absent; if it did not change, downstream retains its prior state. | A changed import requires full layout generate before `check-corpus` can return `Ok`; an unchanged import does not manufacture staleness. |
| Layout full `generate` | XML absent/current/stale and current HTML/sidecar | Clean completion returns `Ok` and XML/reports become current. A complete diagnostic publication returns `Generation` and leaves a classifiable stale diagnostic generation. | Diagnostic output requires another successful clean full generate. |
| Layout filtered `generate` | classifiable XML current/stale | `Ok`; selected XML is replaced and every other XML/report byte is preserved. The resulting full state is current only if the preserved reports still validate against all resulting XML; otherwise it is stale. | If stale, run a successful clean full generate before `check-corpus` can return `Ok`. |
| Layout `check-taffy-corpus` | any HTML state plus explicit source | `Ok` only when the checkout pin/snapshot and imported sidecar/files match; otherwise `SourceVerification` for checkout pin/object/snapshot drift, `Verification` for absent/stale known imported state, or `InvalidInventory` for unknown/malformed corpus shape. No bytes change. | Correct or re-import the named source; the command never repairs it. |
| Layout `check-corpus` | HTML and XML current/stale/absent/invalid | `Ok` only when both are current; `Verification` for absent/stale/diagnostic known state; `InvalidInventory` for unknown entries or malformed known artifacts. No bytes change. | Run the indicated import or clean full generate; the command never repairs state. |
| CSS `import-csstree` | import root absent/current/stale; downstream expectations absent/current/stale | `Ok`; import root becomes current. If the canonical CSSTree sidecar digest changed, existing classifiable downstream becomes stale and absent downstream remains absent; if it did not change, downstream retains its prior state. | A changed import requires full CSS generate before `check-corpus` can return `Ok`; an unchanged import does not manufacture staleness. |
| CSS full `generate` | expectation root absent/current/stale and current import sidecar/files | `Ok`; expectations/report become current. | None. |
| CSS filtered `generate` | classifiable expectations current/stale | `Ok`; selected expectations are replaced and every other expectation/report byte is preserved. The resulting full state is current only if the preserved report still validates against all resulting expectations; otherwise it is stale. | If stale, run a successful full generate before `check-corpus` can return `Ok`. |
| CSS `check-corpus` | import and expectation roots current/stale/absent/invalid | `Ok` only when both are current; `Verification` for absent/stale known state; `InvalidInventory` for unknown entries or malformed known artifacts. No bytes change. | Run the indicated import or full generate; the command never repairs state. |

For both filtered commands, an absent final root returns `Verification` before
transaction intent. For every mutation command, an invalid current root returns
`InvalidInventory` before transaction intent. A mutation whose required import
sidecar/files are absent or stale returns `Verification` before lease; an
explicit checkout pin, object, or immutable-snapshot mismatch returns
`SourceVerification` before lease.

Read-only checks acquire and finish only `GenerationCheck`. An active exclusive
lease returns `LeaseActive`; abandoned, resumable, or malformed coordination/
transaction evidence returns `ArtifactTransaction`. A check never bootstraps,
recovers, removes, or otherwise repairs that state.

Failure state is determined by the actual durable boundary, not by whether the
public call returned `Err`:

| Failure point | Visible final generation | Residue | Returned kind |
| --- | --- | --- | --- |
| CLI/manifest/source/namespace/capability validation before lease | prior absent/current tree | none created by the command | owning `Cli`, `InvalidManifest`, `SourceVerification`, `InvalidPath`, or `UnsupportedPlatform` kind |
| lease/bootstrap/recovery before transaction intent | prior tree | only a valid coordination state that later acquisition can inspect; no publication journal | `LeaseActive` or owning transaction/I/O kind |
| after intent but before commit, with synchronous recovery successful | absent for exclusive or complete old for swap | no transaction residue | original owning error kind |
| commit completed but root sync/outcome/cleanup fails, with recovery successful | complete new | no residue after successful recovery | `ArtifactTransaction` because commit occurred |
| any recovery or cleanup cannot safely complete | absence/old/new dictated by the commit oracle | valid resumable evidence retained | `ArtifactTransaction` containing operation and recovery context |

No domain matrix may promise byte-identical old output after a post-commit
failure. Repeating acquisition/recovery must preserve the committed new tree and
finish only receipt-bound cleanup.

### SR-04.6 Core ownership and quality

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

The dependency cycle adds tracked `deny.toml` with exactly one license policy:

```toml
[licenses]
confidence-threshold = 0.8
allow = [
  "0BSD",
  "Apache-2.0",
  "Apache-2.0 WITH LLVM-exception",
  "BSD-2-Clause",
  "BSD-3-Clause",
  "BSL-1.0",
  "CC0-1.0",
  "ISC",
  "MIT",
  "MPL-2.0",
  "OpenSSL",
  "Unicode-3.0",
  "Unicode-DFS-2016",
  "Unlicense",
  "Zlib",
]
```

There are no license exceptions, private-crate bypasses, or clarifications.
`cargo deny check licenses` fails on an unlicensed, unknown, low-confidence, or
not-allowed expression; the allow list intentionally excludes GPL, AGPL, LGPL,
SSPL, BUSL, Commons-Clause, and noncommercial/source-available terms. A direct
or transitive crate requiring anything outside this list stops dependency work
for an explicit reviewed policy or dependency change; it is never silently
added merely to make the gate pass.

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
provenance_format = "<contains {version} and {repository_relative_executable}>"

[browser.launch]
batch_size = 1
navigation_timeout_ms = 1
dom_poll_interval_ms = 1
retry_count = 1
job_order = "sorted-sequential"
retry_error_class = "open-load-reset-timeout"
profile_scope = "per-batch-and-retry"
page_scope = "per-job"
disable_default_args = true
disable_cache = true
arguments = ["use-mock-keychain", "<27 additional allowed unique arguments>"]

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
kind = "taffy"
path = "html"
upstream_commit = "d1ff7e339b9ee35b33858779f8d7653197e93d92"
description = "<nonempty trimmed text>"

[source_roots.surgeist]
kind = "surgeist"
path = "html"
description = "<nonempty trimmed text>"

[imports.taffy]
repo = "https://github.com/DioxusLabs/taffy.git"
commit = "d1ff7e339b9ee35b33858779f8d7653197e93d92"
source_dir = "test_fixtures"
destination = "html"
expected_count = 1103
excluded_destination_dirs = ["grid-lanes", "subgrid"]

[[cases]]
id = "<unique canonical id>"
source_root = "surgeist"
source = "<strict .html path below html>"
generator = "constrained-html"
status = "active"
# reason exists only for non-active status
```

The angle-bracket strings above are grammar notation, not accepted literal
values. All objects deny unknown and duplicate fields. Schema 2 adds no required
manifest field relative to the preserved source. `batch_size` is a positive
`usize`; navigation timeout and polling interval are positive `u64` values;
disposition/report counts are nonnegative `usize` values; `retry_count` and all
lifecycle strings/booleans are exactly the shown values. `arguments` contains
exactly 28 unique entries, includes `use-mock-keychain`, and satisfies SR-03.2's
redirect restrictions. The two excluded destination directories are exactly the
shown unique set in either order. The Taffy repository, revision, source
directory, pre-exclusion count, destination, source-root kinds/paths, and upstream
commit are exact.

Paths are strict normalized relative paths. `cache_root` resolves beneath owner
and outside corpus; the CLI browser executable resolves beneath that exact cache
root. Report files are unique one-component JSON names; full is exactly
`all.json`; scoped filters/files are unique. Taffy and Surgeist share the `html`
physical root but have disjoint manifest ownership. Every authored Surgeist file
appears exactly once in `cases`; imported Taffy files come only from the verified
sidecar. Unknown HTML entries fail inventory validation.

Compatibility classification is backward-compatible schema 2 with two explicit
security tightenings: duplicate launch/exclusion entries and path-redirecting
launch arguments formerly admitted by loose validation are rejected. No browser
pin field or schema 3 is introduced. Exact preserved-schema fixtures and a
full-field TOML golden must parse to the same domain values and launch digest.

The preserved launch digest is lowercase SHA-256 of the exact bytes returned by
`serde_json::to_vec` for this tuple, with no final LF:

```rust
(
    1_u8,
    launch.batch_size,
    launch.navigation_timeout_ms,
    launch.dom_poll_interval_ms,
    launch.retry_count,
    &launch.job_order,
    &launch.retry_error_class,
    &launch.profile_scope,
    &launch.page_scope,
    launch.disable_default_args,
    launch.disable_cache,
    &launch.arguments,
)
```

No sorting is applied to launch arguments because manifest order is part of the
preserved digest and browser invocation.

The Taffy sidecar is an artifact migration, not a manifest migration. A legacy
schema-2 corpus without it returns `Verification` from generate/check with the
instruction to run `import-taffy --source-root ...`; import atomically adds the
sidecar while preserving authored HTML. A future layout-owned adoption must run,
review, and commit that one corpus migration before switching its scripts. This
repository never performs that sibling migration.

### SR-06.2 Taffy import and offline proof

`import-taffy` verifies the explicit source checkout against the manifest pin,
takes an immutable byte snapshot of regular `.html` files below `source_dir`,
requires `expected_count`, and constructs canonical
`html/.surgeist-taffy-source.json`. Its compact JSON plus final LF contains, in
order: `schema_version: 1`, canonical repository/revision/source directory,
object format, pre-exclusion source count, sorted excluded-directory set,
post-exclusion imported count, and strictly sorted included-file records of
relative path, Git mode `100644`, blob object ID, and SHA-256.

The exact sidecar shape is:

```json
{"schema_version":1,"source":{"label":"taffy","repository_url":"https://github.com/DioxusLabs/taffy.git","revision":"<full-object-id>","source_subdirectory":"test_fixtures"},"object_format":"<sha1-or-sha256>","source_file_count":1103,"excluded_destination_dirs":["grid-lanes","subgrid"],"imported_file_count":1,"files":[{"path":"relative.html","git_mode":"100644","blob_object_id":"<full-blob-id>","sha256":"<raw-byte-sha256>"}]}
```

`expected_count`/`source_file_count` counts every regular upstream `.html` before
exclusion. A file is excluded exactly when its first relative component is
`grid-lanes` or `subgrid`; excluded bytes are neither copied nor listed.
`imported_file_count` equals `files.len()` after exclusion. `object_format` is
derived from the verified repository and is `sha1` for a 40-hex revision or
`sha256` for a 64-hex revision; every blob ID has the matching width. Files are
strictly increasing by `RelativePath`. The SHA-256 golden names are
`layout_taffy_sidecar_sha1_golden` and
`layout_taffy_sidecar_sha256_golden`.

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

A successful import returns `Ok` after the new HTML/sidecar transaction while
leaving existing XML/reports untouched. If the canonical sidecar digest changed,
those downstream artifacts have old provenance and `check-corpus` returns
`Verification` with a regenerate instruction until one successful full generate
atomically refreshes XML/reports; `check-taffy-corpus` may pass during that
intermediate state. If the sidecar digest is unchanged, downstream freshness is
unchanged. Pre-commit and post-commit import failures follow SR-04.5; post-commit
`ArtifactTransaction` can coexist with the complete new import and, when its
sidecar changed, the same required regeneration.

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
| Filtered job failure, validation failure, browser/version failure, panic cleanup failure, or incomplete report before plan construction | no plan/install; return semantic error | none | prior generated tree remains |
| Artifact transaction fails before commit and synchronous recovery succeeds | error per SR-04.5 | none installed | prior absent/old tree remains; no transaction residue |
| Artifact transaction commits but root sync/outcome/cleanup reports failure | return `ArtifactTransaction` | complete new XML/report set remains visible | recovery completes or retains valid resumable evidence; never restore old |

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
{"schema_version":1,"source":{"label":"csstree","repository_url":"https://github.com/csstree/csstree.git","revision":"<full-object-id>","source_subdirectory":"fixtures/ast"},"object_format":"<sha1-or-sha256>","file_count":1,"files":[{"path":"declaration/Declaration.json","git_mode":"100644","blob_object_id":"<full-blob-id>","sha256":"<raw-byte-sha256>"}]}
```

`object_format` is exactly `sha1` or `sha256` and fixes revision/blob-ID widths;
all object IDs and SHA-256 values are lowercase full-width hex. File records are
strictly increasing and unique.

Current import classification comes only from the old validated sidecar plus its
listed files. Unknown entries fail before intent. Desired retention is exactly
the new sidecar and snapshot, so stale old fixture files disappear atomically.
No expectation/report changes. Closing source revalidation follows SR-03.

A successful import returns `Ok` with the new source tree while preserving every
old expectation/report byte. If the canonical sidecar digest changed,
`check-corpus` returns `Verification` because downstream source
revision/digests are stale until a successful full generate replaces the
complete expectation root. If the sidecar digest is unchanged, downstream
freshness is unchanged. Import post-commit failure semantics are the same except
the command returns `ArtifactTransaction` while the new source tree remains
visible.

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

Top-level ordinary members are semantically unordered. After all ordinary and
error cases are derived and overrides resolved, the complete `cases` array is
sorted by the final escaped `id` using Rust string lexicographic order. No source
object-member order is retained; error indices remain part of their IDs and
therefore participate in the same final sort.

AST, upstream diagnostic prose, offsets, comments, and recovery structures are
never copied. A streaming prepass rejects duplicate decoded object members at
every depth and trailing values before typed parsing. Objects in preserved
options are recursively sorted by decoded Unicode-scalar key, arrays retain
order, and scalar JSON serialization is Serde canonical output. Malformed shape,
empty derived case set, duplicate ID, unmatched override, or full count mismatch
is `InvalidInventory` before publication.

Required encoding goldens are `css_import_sidecar_sha1_golden`,
`css_import_sidecar_sha256_golden`, and
`css_expectation_case_order_golden`; each asserts complete bytes including the
single final LF.

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
| Validation/derivation failure before plan | no install | prior expectation tree remains |
| Artifact failure before commit with successful recovery | no completed install | prior absent/old expectation tree remains |
| Artifact failure after commit | return `ArtifactTransaction` | complete new expectation/report root remains; cleanup completes or retains resumable evidence |

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
not normally depend on this tooling crate. They identify Chromium as a trusted
external executable capability outside generator-owned filesystem containment
and explain the one-time Taffy-sidecar migration. They do not call the crate a
scaffold or claim sibling integration completed.

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
cargo deny --all-features --locked --offline check licenses
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

The terminal tree retains canonical `plans/.gitkeep` and the reviewed
specification, sequence, cycle, review, and evidence paths required by the
candidate handoff. The handoff names each path and reviewed revision together
with the immutable published leaf SHA and exact feature/command/verification
contract; root and sibling adoption remain separate future work. Execution-
resource cleanup remains solely governed by `surgeist-agent`.
