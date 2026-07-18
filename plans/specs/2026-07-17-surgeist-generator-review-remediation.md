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
synthetic tests, README and AGENTS, and canonical planning files. Root
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
cache containment, executable type, raw SHA-256 stability, and manifest version
output. Those checks do not prove the program benign. Taffy and CSSTree commands
accept explicitly supplied, existing Git checkouts whose bytes and provenance
are verified read-only. None of the resources is installed, repaired, or
removed.

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
| Layout import Taffy | `html` as one partition-aware publication authority: reserved sidecar and Taffy-owned paths are replaceable while exact manifest-authored files are retained; `.surgeist-generator` including journals/stages | `corpus.toml`; helpers; the held retained-authored partition; complete Taffy source protection snapshot |
| Layout check Taffy | none | corpus inputs plus complete Taffy source protection snapshot |
| Layout check corpus | none | manifest, helpers, HTML/Taffy sidecar, XML, reports, and existing coordination state |
| CSS import CSSTree | `import_root`; `.surgeist-generator` including journals/stages | `corpus.toml`; `expectation_root`; complete CSSTree source protection snapshot |
| CSS generate | `expectation_root`; `.surgeist-generator` including journals/stages | `corpus.toml`; complete imported tree and source sidecar |
| CSS check corpus | none | manifest, imported tree/sidecar, expectations, report, and existing coordination state |

Transaction reservations named `._surgeist-*` are writable members of the same
corpus-root publication authority and are included even when not yet present.
Layout browser profile journals live only beneath
`.surgeist-generator/profiles/layout/` and are created, recovered, and removed by
the held rooted authority. No owner-global or system-temporary profile path is
used. The complete browser cache root is read-only.

### SR-03.2 Disjointness and revalidation

For each command, every generator-owned writable namespace is compared with every
external protected namespace and with every other writable final root. Equality,
protected ancestor of writable, writable ancestor of protected, and
component-wise overlap are all rejected. The browser cache root must be outside
the complete corpus root, not only outside `xml`; import and expectation roots
must be disjoint from each other, `corpus.toml`, and `.surgeist-generator`.

Layout Taffy import has one narrow internal exception: `html` is the atomic final
root and contains an exact retained-authored partition. Before lease acquisition,
the command derives three strict file sets: manifest-owned Surgeist paths,
current Taffy-owned paths, and desired Taffy-owned paths plus the reserved
sidecar. The authored set must be disjoint from both Taffy sets; any exact file
collision is `InvalidInventory`. Directories may be shared only as implied
ancestors of disjoint files. Every authored file is opened through the held
`html` authority, identity-bound, snapshotted, and revalidated while the mutex is
held; its snapshot bytes are copied unchanged into the staged complete `html`
root. Taffy paths and the reserved sidecar are the only replaceable partition.
No other command or protected/writable pair receives this ancestor exception.

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
no external resource executable. After preflight, exclusive acquisition may run
only the prerequisite transaction/coordination recovery protocols over namespaces
already proved writable and disjoint; those protocols consume no protected-source
snapshot and create no new command publication intent. While the domain mutex is
held and before any new command publication intent, profile creation or cleanup,
or external process launch, the command reopens every protected directory without
following links, requires its recorded identity, and repeats the complete
disjointness matrix through a crate-private protected revalidation callback. A
changed path or identity fails closed. The snapshot bytes—not a reread
checkout—feed import publication.

Existing browser validation performs the same canonical and identity checks on
the manifest cache root and CLI executable, rejects any resolution outside the
cache, requires an executable single-link regular file, and snapshots its raw
SHA-256 through the held descriptor. Closing revalidation requires the same
identity and digest. Only after lease acquisition and the complete closing
revalidation does it create the rooted profile hierarchy,
execute `<browser> --version` with the fixed environment below, compare the exact
normalized output with the manifest, and launch Chromium with that same
environment. The trusted executable capability can write or spawn outside the
generator-owned namespaces; this specification does not claim to sandbox or
contain it. Operator trust in that exact executable is a command precondition,
and includes ordinary Chromium behavior: the cache path is not concurrently
replaced, and the browser and its helpers remain in the inherited process group.
The supervisor revalidates path identity/digest immediately before spawn and
again after terminalization, but macOS path-based spawn is not an atomic
execute-from-held-descriptor proof. A malicious binary could replace itself,
daemonize, or leave the recorded group. README/rustdoc must state that boundary;
the lifecycle proof covers the recorded group, not processes a trusted
capability deliberately detaches from it.

The preserved manifest contains 28 launch strings and their order remains part of
the preserved launch digest. For invocation, each string is normalized by
removing exactly one optional leading `--`, splitting at the first `=`, and
requiring a nonempty ASCII key that does not begin with `-`. The 28 normalized
keys must be unique; every complete string must be printable ASCII and contain no
slash, backslash, control byte, or NUL. One normalized string is exactly
`use-mock-keychain`. These manifest keys are forbidden:
`user-data-dir`, `disk-cache-dir`, `media-cache-dir`, `data-path`, `homedir`,
`log-file`, `log-net-log`, `crash-dumps-dir`, `crash-dump-dir`,
`breakpad-dump-location`, `download-default-directory`, `ssl-key-log-file`,
`trace-startup-file`, `profiling-file`, `print-to-pdf`, `screenshot`,
`remote-debugging-port`, `remote-debugging-address`, `remote-debugging-pipe`,
`disable-extensions`, `load-extension`, and `disable-extensions-except`. The
driver-owned normalized switch set is exactly `remote-debugging-port=0`,
`disable-extensions`, and `user-data-dir=<exact profile path>`.

Chromiumoxide 0.9.1 intentionally treats the complete switch collection as an
unordered key map. The manifest order is therefore a digest/provenance promise,
not an argv-order promise. Unique disjoint keys make all admitted switches
order-independent. Before it starts the trusted executable, the internal
supervisor validates that the received normalized switch set is exactly the 28
manifest switches plus those three driver switches, replaces Chromiumoxide's
display-form `user-data-dir` value with the exact OS-native profile path from its
bound launch capsule, and forwards the otherwise unchanged received order. No
artifact or semantic oracle depends on that order.

The pinned measurement `BrowserConfig` uses the current layout binary as its
executable and applies exactly `with_head`, `disable_default_args`,
`disable_cache`, the attempt `user_data_dir`, the 28 normalized manifest
strings, and the one launch-capsule environment entry. Chromiumoxide 0.9.1's
remaining builder fields stay at port zero, sandbox true, no extensions/window
size/incognito/hidden/HTTPS-first disabling/request interception, 20-second
launch timeout, 30-second request timeout, default viewport, ignored invalid
events, and ignored HTTPS errors. These are builder inputs, not promises that
the manifest cannot request the corresponding Chromium behavior. In particular,
manifest keys such as `no-sandbox`, `disable-setuid-sandbox`, `headless`,
`incognito`, `window-size`, `disable-blink-features`, and `disable-features` are
manifest-owned unless already forbidden above; their exact values are pinned by
the raw launch digest and forwarded unchanged. Operator trust covers that pinned
configuration. The builder itself adds only the three driver switches stated
above. A focused config adapter test passes permutations through the pinned
builder and supervisor parser, proves the exact admitted manifest-plus-driver
set, and never treats builder defaults as effective-command overrides.

The real browser environment is fixed and cleared. Both version and measurement
commands use `Command::env_clear`; set `HOME`, `TMPDIR`, `TMP`, `TEMP`,
`XDG_CONFIG_HOME`, `XDG_CACHE_HOME`, and `XDG_DATA_HOME` to precreated directories
beneath the attempt profile named respectively `home`, `tmp`, `tmp`, `tmp`,
`xdg-config`, `xdg-cache`, and `xdg-data`; use the profile root as cwd; set
`PATH=/usr/bin:/bin`, `TZ=UTC`, `LANG=C`, and `LC_ALL=C`; set
`HTTP_PROXY`, `HTTPS_PROXY`, `ALL_PROXY`, `http_proxy`,
`https_proxy`, and `all_proxy` to the empty string; and set `NO_PROXY` and
`no_proxy` to `*`. No inherited, manifest-supplied, or operator-supplied entry
reaches the real browser. The generator's internal supervisor receives one
private launch-capsule environment value from its parent; it is authenticated
against the held lease/profile journal, contains OS paths as lowercase hex bytes,
is removed by `env_clear`, and is not an operator configuration surface.

Synthetic tests cover both ancestor directions, missing suffixes, case/symlink
aliases, source object-store overlap, browser-cache/corpus overlap, replacement
between preflight and protected revalidation, the one retained-authored import
partition, authored/Taffy collision rejection, switch normalization/set
permutations including manifest-owned browser settings, launch-capsule rejection,
the exact cleared environment map, and outside sentinels that remain
byte-identical after generator-owned operations.
They do not make a containment claim about the trusted executable, which tests
never launch.

### SR-03.3 Browser supervisor, profile journal, and terminalization

Each zero-based batch/retry attempt has a fresh profile token and one exact
journal root:

```text
.surgeist-generator/profiles/layout/
  active-<lease-token>-version-<profile-token>/
  active-<lease-token>-batch-<batch>-retry-<retry>-<profile-token>/
    intent.json
    transition.lock
    profile/
    profile.json
    running.json              # present only after supervisor registration
```

The children shown apply to either active form; `retry` is `0` or `1`.
Parent/layout/journal directories are private mode `0700`;
metadata and the immutable lock are mode `0600`; `transition.lock` contains only
the existing `surgeist-generator-lock-v1` header. Metadata is compact canonical
JSON plus one LF with fields in the shown order. `intent.json` contains
`schema_version: 1`, purpose (`version` or `measurement`), authority key, parent
PID, lease/profile tokens, nullable batch/retry ordinals, owner-relative browser
path, browser `HeldIdentity`, raw browser SHA-256, and launch-profile SHA-256.
Version has null ordinals; measurement has the path-matching integers.
`profile.json` contains `schema_version: 1`,
profile token, strict journal-relative profile path, and the profile directory's
`HeldIdentity`. `HeldIdentity` retains its existing canonical field order
`kind`, `device`, `inode`, `fsid: {words}`, `mode`, `owner`, `link_count`.
`running.json` contains `schema_version: 1`, profile token, parent PID,
supervisor PID, and process-group ID; supervisor PID and process-group ID must be
equal and nonzero.

The byte shapes are exactly:

```json
{"schema_version":1,"purpose":"<version-or-measurement>","authority_key":"<authority-key>","parent_pid":1,"lease_token":"<lease-token>","profile_token":"<profile-token>","batch_ordinal":null,"retry_ordinal":null,"browser_path":"<owner-relative-browser-path>","browser_identity":{"kind":"regular","device":1,"inode":1,"fsid":{"words":[1,1]},"mode":493,"owner":1,"link_count":1},"browser_sha256":"<64-lowercase-hex>","launch_profile_sha256":"<64-lowercase-hex>"}
{"schema_version":1,"profile_token":"<profile-token>","profile_path":"profile","identity":{"kind":"directory","device":1,"inode":1,"fsid":{"words":[1,1]},"mode":448,"owner":1,"link_count":null}}
{"schema_version":1,"profile_token":"<profile-token>","parent_pid":1,"supervisor_pid":2,"process_group_id":2}
```

Angle-bracket strings and numeric identity values are grammar examples. Version
uses null ordinals; measurement serializes nonnegative `u64` batch and `0`/`1`
retry numbers. All tokens use the existing lowercase token grammar; all identity
numbers are the observed held values. Unknown/duplicate fields, a nonmatching
name/purpose/ordinal, or any noncanonical byte representation is invalid.

Before any external launch, the parent creates and syncs the complete prefix
through `profile.json`. The sole private environment key is
`SURGEIST_LAYOUT_LAUNCH_CAPSULE`. Its value is this compact canonical JSON with
no final LF:

```json
{"schema_version":1,"owner_root_hex":"<lowercase-even-width-hex-os-bytes>","corpus_root_hex":"<lowercase-even-width-hex-os-bytes>","journal_path":"<strict-relative-journal>","intent_sha256":"<64-lowercase-hex>","profile_sha256":"<64-lowercase-hex>","parent_pid":1,"profile_token":"<profile-token>","browser_path":"<owner-relative-browser-path>","purpose":"<version-or-measurement>","launch_strings":["<ordered-normalized-string>"]}
```

Thus the private launch capsule contains
schema 1, lowercase-hex owner/corpus OS-path bytes, journal relative path,
intent/profile-record SHA-256 values, parent PID, profile token, owner-relative
browser path, purpose, and the ordered normalized launch strings (`version` only
for the version purpose; the semantic switch set for measurement). The supervisor
accepts it only when all bytes, digests, identities, tokens, parent PID, and the
held layout-mutex state match the journal. A forged, stale, incomplete, or
operator-supplied capsule exits without launching a browser.

The parent first runs a version-purpose supervisor directly, bounds it at five
seconds, caps each output stream at 64 KiB, validates exit success and normalized
stdout, terminalizes its group/profile, and only then starts measurement. Timeout
or excess output group-kills and reaps through the normal lifecycle.
Normalization is exact UTF-8 `split_whitespace().collect::<Vec<_>>().join(" ")`;
an empty result cannot match the required nonempty manifest value. Successful
stderr is retained only for diagnostics and does not enter provenance.
Immediately before every later measurement-purpose journal, it reopens the cache
root/executable and requires the same held identity and raw SHA-256 recorded by
the successful version attempt; drift is `SourceVerification` before that
supervisor is created.

Chromiumoxide still owns measurement websocket discovery and protocol connection.
It launches the current layout binary as the measurement-purpose internal
supervisor, supplies the capsule only to that child, reads the trusted browser's
forwarded stderr until the DevTools websocket URL appears, connects, and returns
its normal `Browser`/`Handler`. Either-purpose supervisor performs this exact
transition while holding `transition.lock`
exclusively: validate the capsule and parent PID; become a new process-group
leader; publish/sync `running.json`; spawn the exact verified browser in that
group with the fixed cwd/environment and validated purpose-specific argv; then
release the transition lock. It copies browser stderr byte-for-byte without unbounded
buffering; in version mode it also copies stdout under the parent's stated cap;
in measurement mode browser stdout is null. It waits for the browser and exits
with the browser status. A spawn failure leaves a dead recorded group and exits
nonzero. Thus no real browser can start before its durable group record, and
killing the supervisor alone cannot make an extant browser group look absent.

The `Browser::launch` future alone is wrapped in
`AssertUnwindSafe(...).catch_unwind()` because Chromiumoxide 0.9.1 uses `expect`
while killing/waiting its child on launch failure. The handler task and every
direct Chromiumoxide future that consumes browser/process/protocol data—page
creation, evaluation, close, and command replies—use the same narrow dependency
boundary; repository-owned mapping, accounting, and invariant code is outside
those wrappers. A caught dependency panic is converted to `Process` with the
panic payload rendered safely, then the recorded whole group/profile is
terminalized. The outer worker panic boundary remains for repository-owned
invariant failures and resumes those payloads as specified below; external
browser behavior cannot reach that outer boundary.

The normal owner terminalizes an attempt before constructing an artifact plan:

1. stop scheduling work and close/drop every page;
2. for measurement, while the handler still runs, allow five seconds for
   `Browser::close`; version proceeds directly to wait;
3. allow five seconds for the browser/supervisor group to exit and be reaped;
4. if still live, verify the recorded group against the owned supervisor child,
   send `SIGKILL` to that whole group through safe `rustix`, and allow five
   seconds to reap the supervisor;
5. abort and await the handler task, bounded by five seconds, after the connection
   is closed;
6. prove the recorded process group is absent, acquire `transition.lock`
   exclusively, rename `active-...` to the same `cleanup-...` suffix, sync the
   profiles parent, erase that tree, and sync again;
7. only then release the layout lease or build/publish artifacts.

`Browser::kill` is not used because its child is the supervisor, not the whole
group. Close failure does not skip group kill/wait. The first job/launch/handler
error remains primary only if all terminalization succeeds. A live or
inconclusive group after the forced path returns `Process` with its I/O/process
source and preserves the active journal/profile; a dead group whose durable
rename/erase fails returns `ArtifactTransaction` containing both the primary and
cleanup contexts and preserves the cleanup journal. During an unexpected panic,
the same terminalization runs; the original panic payload is resumed afterward,
including when cleanup evidence must remain for the next acquisition.

After abrupt parent death, the next exclusive layout acquisition first inspects
and classifies profile journals read-only before protected-source revalidation or
new profile creation. A held transition lock or live/permission-inconclusive
recorded process group returns `LeaseActive` without signaling or deleting
anything. This deliberately treats PID/group reuse as a safe false positive. A
provably dead complete active journal, already-renamed cleanup journal, or
interrupted pre-`running.json` creation prefix becomes a deterministic pending
cleanup plan with every journal/profile identity held; no rename, unlink, or
directory removal occurs yet. Unknown or corrupt metadata, replacement, a mount,
or unsafe planned erasure returns `ArtifactTransaction` with byte-identical
evidence. Recovery never sends a signal; operators terminate an orphaned trusted
browser group and retry.

Only after protected-source closing revalidation succeeds does acquisition
revalidate every held profile/journal identity and execute that pending plan: a
dead complete active journal is renamed and erased, an existing cleanup journal
resumes erasure, and a validated interrupted prefix is erased only while the
transition lock remains free. Identity drift between classification and cleanup,
or any rename/erase/sync failure, returns `ArtifactTransaction` and preserves the
exact recoverable evidence; no owner-record install begins. Thus revalidation
failure leaves even dead profile evidence byte-identical, while successful
cleanup is terminal before owner installation and guard return.

At most one active, cleanup, or interrupted-prefix journal may exist because a
new version/measurement journal is never created until the preceding one is
terminal. Recovery first enumerates and validates the complete journal-level
inventory without mutation. A second journal or any unknown member is
`ArtifactTransaction` with every byte preserved; one valid dead journal is then
held as the pending plan and, only after closing revalidation, recovered through
its deterministic primitive trace.

The profile subtree is opaque browser output only below the bound `profile/`
directory. A dedicated descriptor-relative eraser enumerates raw OS names,
never follows symlinks, holds/revalidates every opened directory identity,
refuses mount/device changes, unlinks non-directories, removes directories
deepest-first, and supports non-UTF-8 names. It is unavailable for any other
namespace. Unknown journal-level members remain errors. Read-only checks never
traverse or recover profiles; any active/cleanup/prefix profile state is
`Verification`.

The SR-04.2 observer also traces every profile directory/metadata temporary,
write, sync, publication, transition-lock boundary, running publication,
active-to-cleanup rename, raw entry unlink/rmdir, and parent sync. Fresh-process
recovery is interrupted at every recorded prefix; the oracle is either a live
group with byte-identical evidence or a dead group with complete idempotent
cleanup, never guessed deletion.

Named synthetic tests are
`layout_profile_normal_close_is_terminal`,
`layout_profile_launch_failure_is_terminal`,
`layout_profile_forced_group_kill_is_terminal`,
`layout_profile_parent_crash_live_group_blocks`,
`layout_profile_parent_crash_dead_group_recovers`,
`layout_profile_revalidation_failure_preserves_dead_journal`,
`layout_profile_cleanup_begins_only_after_revalidation`,
`layout_profile_identity_drift_after_classification_preserves_evidence`,
`layout_profile_transition_lock_closes_launch_race`,
`layout_profile_cleanup_every_prefix_recovers`,
`layout_profile_opaque_entries_never_escape`,
`layout_profile_cleanup_failure_preserves_evidence`,
`layout_dependency_panic_maps_to_process`,
`layout_profile_panic_resumes_after_cleanup`, and
`layout_profile_panic_retains_cleanup_evidence`. They use a crate-owned fake
browser mode in the current test binary to exercise real supervisor processes,
process groups, locks, restart, and repeated recovery; deterministic injected
clock/probe adapters avoid wall-clock timeout waits and cover
permission/inconclusive probes. They never launch Chromium or any unowned
executable.

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
cleanup paths, plus `install_owner_record`, `recover_owner_transactions`,
`run_rename_probe`, and `recover_probe_journals`, with an instance-scoped
`#[cfg(test)]` observer. Production construction has no observer and no
interrupted return.

The observer emits one event after every recovery-distinct mutation or
durability operation, not only after high-level phases. Events contain a stable
phase, primitive, strict relative path, and per-phase ordinal. They include:

- every directory creation;
- every temporary-file create, partial/full write, file sync, publication rename,
  and containing-directory sync inside `publish_file_exclusive`;
- every stage directory and individual staged-file creation and each deepest-
  first directory sync;
- every direct handle partial/full write, flush, file sync, identity validation,
  and drop boundary used by owner-record staging;
- every exclusive/swap/claim/completed-journal rename and each parent sync;
- every individual receipt-bound file/directory removal, receipt removal,
  journal removal, probe-member removal, owner-journal removal, and parent sync.

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

#### Owner-record and rename-probe oracles

The complete normative exclusive-acquisition order is:

1. open/bootstrap and exclusively hold the domain mutex;
2. run `TransactionEngine::recover_all`;
3. run `recover_owner_transactions`;
4. run `recover_probe_journals`, then `run_rename_probe`;
5. for layout only, read-only classify SR-03.3 profile journals and hold any
   valid dead-journal cleanup plan;
6. run the protected-source closing revalidation callback;
7. for layout only, revalidate the held profile identities and execute the
   pending dead-profile cleanup plan;
8. only after all preceding steps succeed, run `install_owner_record`; and
9. return the live guard.

The same normative observer drives the owner-record and rename-probe protocols,
but their positions are intentionally different: prerequisite probe execution and
read-only profile classification are before closing revalidation; profile cleanup
and owner installation are after it. A probe/profile-classification/revalidation/
profile-cleanup failure creates no owner transaction and leaves the historical
owner byte-identical. Revalidation failure also leaves the classified profile
byte-identical. An owner-install failure follows the owner visibility oracle below
and returns no guard. The mutex is released on every failed acquisition after
durable recovery/evidence decisions complete.

Owner-record install runs twice: once with no prior `owner.json` and once with a
valid old record. Its trace includes active-journal creation; every intent,
registration, prepared, committed, and recovered-aborted temporary/write/sync/
publication; empty stage creation; each stage write prefix, flush, sync, and
identity check; exclusive or swap commit; leases-parent sync; old-stage removal;
and every simple-journal member/directory removal. Before the exclusive/swap
commit event, the visible owner remains respectively absent or byte-identical old;
at and after commit, the complete new canonical record is visible. A fresh
`recover_owner_transactions` after every install prefix preserves that oracle,
removes all valid residue, and is idempotent.

Owner recovery is itself recorded and interrupted at every primitive for four
seeds: absent/old before commit and absent/old after commit. It may publish only
the outcome marker dictated by actual owner visibility, never change old/new
choice, and must leave a prefix another fresh production recovery completes
idempotently. The fixture injects deterministic token, PID, clock, roots, and
metadata so bytes are assertable. Unknown members, malformed markers, digest or
identity replacement, and owner bytes matching neither outcome return
`ArtifactTransaction`, preserve evidence, and do not alter the visible owner.

Rename-probe install traces active-journal/intent publication, left/right probe
directory creation, exclusive left-to-moved rename, moved/right swap, each
post-swap removal, directory sync, intent removal, and active-journal removal.
No prefix may change a domain artifact or historical owner record. After every
install prefix, fresh `recover_probe_journals` removes only validated
intent/temporary/probe members; a subsequent production `run_rename_probe`
succeeds and leaves no residue. Probe recovery is also interrupted after every
individual removal and sync and must finish idempotently on the next fresh call.
An unknown member, wrong type/mode/identity, alias, mount, or replaced probe
directory returns `ArtifactTransaction` and preserves the complete evidence.

Two deterministic test-only capability faults cover the production failure
branches. `FailExclusiveRename` rejects left-to-moved before it mutates either
name; cleanup removes left then right, syncs the journal, and outer cleanup
removes intent and the active journal. `FailSwapRename` succeeds at
left-to-moved, rejects moved/right swap before that mutation, then removes moved
and right in that order, syncs, and performs the same outer cleanup. Every
individual failure-cleanup and recovery primitive is interruptible. Complete
cleanup returns `UnsupportedPlatform` after mutex acquisition with the historical
owner and domain artifacts unchanged and no probe residue. Any cleanup failure
returns `ArtifactTransaction` with the capability failure plus cleanup source and
preserves the exact valid active journal. The next acquisition first resumes
`recover_probe_journals`; after successful recovery it reruns the probe and again
returns `UnsupportedPlatform` if the injected/platform limitation remains.

Named production-path tests are
`owner_record_install_every_prefix_recovers_absent`,
`owner_record_install_every_prefix_recovers_swap`,
`owner_record_recovery_every_prefix_is_idempotent`,
`owner_record_corruption_preserves_evidence`,
`rename_probe_install_every_prefix_recovers`,
`rename_probe_exclusive_unsupported_every_prefix_recovers`,
`rename_probe_swap_unsupported_every_prefix_recovers`,
`rename_probe_unsupported_cleanup_failure_preserves_evidence`,
`rename_probe_recovery_every_prefix_is_idempotent`,
`rename_probe_corruption_preserves_evidence`, and
`lease_acquisition_recovers_owner_and_probe_prefixes`. The acquisition boundary
also has `lease_revalidation_failure_preserves_historical_owner`,
`lease_owner_install_begins_only_after_revalidation`, and
`lease_owner_install_begins_only_after_profile_recovery`.

### SR-04.5 Shared publication state and error matrix

Every domain mutation command uses these exact `ArtifactPlan` semantics. In this
section, **absent** means the final publication root does not exist; **current**
means its structure, provenance, digests, and report relationships all match the
currently validated inputs; **stale** means every entry is classifiable but at
least one required artifact is absent or has old provenance, bytes, digest, or
report linkage; and **invalid** means at least one entry cannot be classified by
the validated historical-plus-desired inventory authority below.

| Initial publication | Command form | Successful result |
| --- | --- | --- |
| final root absent | full import/generate | exclusive commit of one complete desired root |
| structurally valid current root | full import/generate | swap to one complete desired root |
| classifiable but stale current root | full import/generate | swap to the new complete root; only classified stale entries disappear |
| unknown/unclassifiable entry | any mutation | `InvalidInventory` before transaction intent; final tree unchanged |
| final root absent | filtered generate | `Verification`; no partial corpus is established |
| structurally classifiable current root whose historical full report owns every selected output the command could publish | filtered generate | selected artifacts updated; reports and every other entry preserved |
| selected output the command could publish is absent from historical full-report authority | filtered generate | `Verification` before lease; a full generation is required so no ownerless path is created |

The exact command-level state transitions are:

| Command | Admitted initial publication state | Successful return and resulting state | Required follow-up |
| --- | --- | --- | --- |
| Layout `import-taffy` | manifest-authored HTML is exact; the Taffy-owned portion is absent/current/stale; downstream XML is absent/current/stale | `Ok`; HTML becomes current. If the canonical Taffy sidecar digest changed, existing classifiable downstream becomes stale and absent downstream remains absent; if it did not change, downstream retains its prior state. | A changed import requires full layout generate before `check-corpus` can return `Ok`; an unchanged import does not manufacture staleness. |
| Layout full `generate` | XML absent/current/stale and current HTML/sidecar | Clean completion returns `Ok` and XML/reports become current. A complete diagnostic publication returns `Generation` and leaves a classifiable stale diagnostic generation. | Diagnostic output requires another successful clean full generate. |
| Layout filtered `generate` | classifiable XML current/stale; every possible scheduled selected XML path is owned by the validated current-schema historical full report | `Ok`; selected generated XML is replaced, selected paths now classified unsupported are removed, and every unselected XML/report byte is preserved. The resulting full state is current only if the preserved reports still validate against all resulting XML; otherwise it is stale. | If selection is unowned, return `Verification` before lease and require full generation. If the result is stale, run a successful clean full generate before `check-corpus` can return `Ok`. |
| Layout `check-taffy-corpus` | any HTML state plus explicit source | `Ok` only when the checkout pin/snapshot and imported sidecar/files match; otherwise `SourceVerification` for checkout pin/object/snapshot drift, `Verification` for absent/stale known imported state, or `InvalidInventory` for unknown/malformed corpus shape. No bytes change. | Correct or re-import the named source; the command never repairs it. |
| Layout `check-corpus` | HTML and XML current/stale/absent/invalid | `Ok` only when both are current; `Verification` for absent/stale/diagnostic known state; `InvalidInventory` for unknown entries or malformed known artifacts. No bytes change. | Run the indicated import or clean full generate; the command never repairs state. |
| CSS `import-csstree` | import root absent/current/stale; downstream expectations absent/current/stale | `Ok`; import root becomes current. If the canonical CSSTree sidecar digest changed, existing classifiable downstream becomes stale and absent downstream remains absent; if it did not change, downstream retains its prior state. | A changed import requires full CSS generate before `check-corpus` can return `Ok`; an unchanged import does not manufacture staleness. |
| CSS full `generate` | expectation root absent/current/stale and current import sidecar/files | `Ok`; expectations/report become current. | None. |
| CSS filtered `generate` | classifiable expectations current/stale; every selected expectation is owned by the validated historical full report | `Ok`; selected expectations are replaced and every other expectation/report byte is preserved. The resulting full state is current only if the preserved report still validates against all resulting expectations; otherwise it is stale. | If selection is unowned, return `Verification` before lease and require full generation. If the result is stale, run a successful full generate before `check-corpus` can return `Ok`. |
| CSS `check-corpus` | import and expectation roots current/stale/absent/invalid | `Ok` only when both are current; `Verification` for absent/stale known state; `InvalidInventory` for unknown entries or malformed known artifacts. No bytes change. | Run the indicated import or full generate; the command never repairs state. |

#### Historical downstream inventory authority

An existing downstream root is never classified solely from the newly imported
source sidecar. Its durable ownership authority is the previously published full
report retained inside that downstream root. Historical validation is a strict
ownership mode, not a freshness waiver: it requires canonical bytes, exact
schema/generator, duplicate/unknown-field rejection, internally matching summary
and buckets, unique strict source/output paths, domain mapping rules, and every
recorded output digest to have exact lowercase SHA-256 grammar. It recomputes
visible outputs: equality is current for that artifact, while a missing file or
digest mismatch is classifiable absent/stale state and does not invalidate path
ownership. It does not require the old source file or old source sidecar to remain
and does not compare old manifest/source provenance values with current ones. A
malformed or missing historical authority for a nonempty downstream root is
`InvalidInventory` and no path is guessed.

For layout, `xml/generation-reports/all.json` is the primary authority. Every
generated entry maps its recorded `html/<group>/<stem>.html` and variant to the
one exact `xml/<group>/<stem>__<variant>.xml` path and binds that file's raw
SHA-256 for freshness checking. Other historical report files must be
one-component `.json` names,
parse under the same metadata schema, name a unique nonempty filter, and be exact
subsets of `all.json`; this classifies old scoped reports even if the current
manifest no longer declares them. One migration-only authority shape is also
accepted: the exact preserved schema-2 full/scoped report and generated-comment
grammar from `legacy_generator.rs`, whose generated entries omit output SHA.
That shape can establish path ownership only, is always stale, forbids filtered
publication, and must be replaced by one complete `CleanFull` or
`DiagnosticFull` publication in the current schema. A report directory must be
uniformly legacy or current; any mixed legacy/current set is
`InvalidInventory`.

For CSS, the exact shared `GenerationReport` at
`<expectation_root>/generation-reports/all.json` is the authority. Each
`ReportArtifact` must map its strict `<import_root>/<relative>.json` source to
exactly `<expectation_root>/<relative>.json`, carry the output digest used for
freshness checking, use the CSS generator/schema/provenance grammar, and have
positive case count. The
reserved report collision remains invalid in both current and historical modes.

Before any generation plan, the command constructs `historical ∪ desired`:
historical output/report paths from the validated old authority plus output/report
paths derived from the current source/manifest. Every visible downstream entry
must belong to that union or an implied directory. Clean full publication retains
only the new desired set, so removed/renamed historical outputs disappear;
additions are created. Filtered publication writes no authority and therefore may
replace only output paths already recorded by the validated current-schema
historical full report; an absent report, migration-only report, failed/non-output
entry, or newly desired path cannot authorize filtered creation. It preserves all
other union members. A layout `DiagnosticFull` publishes the complete current
report set, retains only successfully generated current XML, and removes every
other historical-union XML/report path; failed current jobs have report entries
but no XML. It therefore leaves one complete current-schema authority even when
membership changes. Read-only checking returns `Verification` for a structurally
valid historical authority that is stale against current inputs, and
`InvalidInventory` for a malformed authority or an entry outside the union.

Named cross-generation tests are
`layout_historical_inventory_removal_rename_addition_regenerates`,
`layout_legacy_report_requires_complete_report_migration`,
`layout_historical_inventory_rejects_malformed_authority`,
`layout_membership_delta_diagnostic_replaces_authority`,
`layout_filtered_add_then_remove_requires_full_before_creation`,
`css_historical_inventory_removal_rename_addition_regenerates`,
`css_historical_inventory_rejects_malformed_authority`, and
`css_filtered_add_then_rename_requires_full_before_creation`.
The two filtered-addition tests execute the complete sequence: publish `A` by a
full run; import desired `B`; prove filtered `B` returns `Verification` and creates
no bytes because the `A` report does not own it; import a removal/rename of `B`;
then prove the next full run succeeds and leaves one current, fully owned root.

#### Exact filtered selection and ownership

A filter is a nonempty strict `RelativePath` relative to the domain input root;
it never includes the physical root component (`html` or the manifest import-root
name). Request construction and CLI parsing reject empty/dot paths, trailing
separators, backslashes, absolute/parent/prefix components, and the domain's exact
reserved path as `Cli` without I/O. Layout reserves `.surgeist-source.json`; CSS
reserves `generation-reports/all.json`. No filesystem `is_file`/`is_dir` probe
chooses matching mode.

Layout uses exact-file mode when the final component ends in `.html`; otherwise
it uses directory-prefix mode. The current fixture inventory is exactly the union
of manifest-owned Surgeist case sources and Taffy HTML paths in the validated
import sidecar; Taffy compatibility case records retain their SR-06.1 no-filter
effect. Exact mode matches one inventory path, relative to `html`, by equal
normalized components. Prefix mode matches every such path whose component vector
starts with the filter's complete component vector; `grid/a` does not match
`grid/ab/case.html`. This validated manifest-plus-sidecar inventory—not historical
XML and not directory existence—is the match authority. Every matching Surgeist
case remains in the selection ledger. Each matched Taffy fixture and each active
or expected-fail Surgeist case schedules its exact four variant paths;
expected-fail additionally contributes its preserved disposition entry. A
manifest-unsupported case contributes the preserved `variant = "manifest"`
unsupported entry and no browser job; a quarantined case contributes its status
entry and no browser job. A selection with no fixture is `Verification` before
lease. A nonempty selection containing only manifest-unsupported/quarantined
fixtures returns `Ok` before lease as a byte-identical no-op, preserving the
legacy disposition-only result. For filtered publication, all four possible paths
for every scheduled fixture must already be generated entries in the historical
full report, even if a recorded file is
currently missing/stale; otherwise the whole request is `Verification` before
lease and publishes nothing.

CSS uses exact-file mode when the final component ends in `.json`; otherwise it
uses component-prefix mode with the same whole-component rule. Its match authority
is the current validated import sidecar. Exact/prefix selection includes whole
fixture artifacts and every derived case/disposition inside each fixture. A
selection with no fixture is `Verification` before lease. Every selected mapped
expectation path must already be a `ReportArtifact` in the historical full report;
otherwise the whole request is `Verification` before lease and publishes nothing.

Precedence is exact: syntax/reserved-filter `Cli`; then manifest, current import/
sidecar, and complete downstream authority/inventory validation with their owning
error kinds; then zero-match/unowned-selection `Verification`; then
lease acquisition. A partial string component never counts as a match, and a
syntactically valid absent exact file is the zero-match case. Layout manifest
scoped-report filters use the same matcher over the complete full-run outcome
ledger. Each scoped report is the exact matched subset, including every
disposition/failure bucket, while an interface filtered run never writes a report.
CSS has no scoped reports.

Named selection tests are `layout_filter_exact_file`,
`layout_filter_component_prefix`, `layout_filter_rejects_partial_component`,
`layout_filter_rejects_reserved`, `layout_filter_absent_is_verification`,
`layout_filter_disposition_only_is_noop`,
`layout_filter_expected_fail_schedules_and_accounts`,
`layout_filter_manifest_unsupported_has_no_job`,
`layout_filter_matches_taffy_sidecar_fixture`,
`layout_scoped_report_uses_filter_matcher`, `css_filter_exact_file`,
`css_filter_component_prefix`, `css_filter_rejects_partial_component`,
`css_filter_rejects_reserved`, and `css_filter_absent_is_verification`.

For both filtered commands, an absent final root returns `Verification` before
transaction intent. For every mutation command, an invalid current root returns
`InvalidInventory` before transaction intent. A mutation whose required import
sidecar/files are absent or stale returns `Verification` before lease; an
explicit checkout pin, object, or immutable-snapshot mismatch returns
`SourceVerification` before lease.

Read-only checks acquire and finish only `GenerationCheck`. An active exclusive
lease, live profile group, transition lock, abandoned/resumable journal, malformed
coordination metadata, or coordination appearing during the check returns
`Verification`, preserving the existing check-error classification. A check never
returns `LeaseActive` or `ArtifactTransaction` for coordination state and never
bootstraps, recovers, removes, signals, or otherwise repairs it.

Failure state is determined by the actual durable boundary, not by whether the
public call returned `Err`:

| Failure point | Visible final generation | Residue | Returned kind |
| --- | --- | --- | --- |
| CLI/manifest/inventory/source/namespace/capability or required-import validation before lease | prior absent/current tree | none created by the command | owning `Cli`, `InvalidManifest`, `InvalidInventory`, `SourceVerification`, `InvalidPath`, `UnsupportedPlatform`, or `Verification` kind |
| read-only coordination acquisition/finish | prior tree | byte-identical coordination/profile state | `Verification`; underlying safe I/O/parse context retained in its diagnostic/source |
| mutation lease/bootstrap/prerequisite transaction or coordination recovery, plus layout read-only profile classification | prior tree except the visibility dictated by a prerequisite recovery oracle | active/live/dead profile evidence byte-identical; valid transaction/coordination recovery either completes or retains its exact evidence | `LeaseActive` for a held lease/transition or live/permission-inconclusive recorded group; `ArtifactTransaction` for corrupt metadata or unclassifiable prerequisite recovery/profile state; safe source retained |
| exclusive- or swap-rename capability probe fails and all failure cleanup completes | prior tree | no probe residue; historical owner byte-identical; mutex released | `UnsupportedPlatform` with the original capability source |
| rename-probe recovery or failure cleanup cannot complete | prior tree | exact valid active probe journal retained; historical owner byte-identical; mutex released | `ArtifactTransaction` containing capability/recovery and cleanup context |
| protected closing revalidation rejects after prerequisite recovery/probe and read-only profile classification | prior tree except completed prerequisite recovery visibility | no owner transaction; historical owner and every profile journal byte-identical; mutex released | exact owning `InvalidPath`, `InvalidInventory`, or `SourceVerification` with safe source |
| post-revalidation dead-profile identity check or cleanup cannot complete | prior generation tree | no owner transaction; historical owner byte-identical; exact active/cleanup/prefix journal evidence retained | `ArtifactTransaction` with profile classification/cleanup context; no live guard returned |
| owner-record install fails after successful closing revalidation and any profile cleanup, but before domain publication intent | prior tree | owner remains absent/old before owner commit or complete new after it; valid resumable owner journal retained only when cleanup cannot finish; profile cleanup is already terminal | `ArtifactTransaction` under the owner install/recovery oracle; no live guard returned |
| after intent but before commit, with synchronous recovery successful | absent for exclusive or complete old for swap | no transaction residue | original owning error kind |
| commit completed but root sync/outcome/cleanup fails, with recovery successful | complete new | no residue after successful recovery | `ArtifactTransaction` because commit occurred |
| any recovery or cleanup cannot safely complete | absence/old/new dictated by the commit oracle | valid resumable evidence retained | `ArtifactTransaction` containing operation and recovery context |

Layout generation has this additional exact pre-publication matrix. Every row
retains the prior XML/report generation because artifact intent is constructed
only after browser/profile terminalization:

| Boundary/outcome | Returned behavior | Coordination/profile state at return | Lease/source precedence |
| --- | --- | --- | --- |
| launch-switch/profile grammar validation | `InvalidManifest` before lease | none | validation error is primary |
| browser cache/path/type/digest identity validation | `InvalidPath` for initial path/type/alias policy or `SourceVerification` for pre-spawn or post-terminalization drift from the verified executable | none | validation error is primary |
| profile-journal creation/registration fails before external launch | `ArtifactTransaction` with I/O source | exact valid prefix retained if it cannot be synchronously erased | lease released only after recovery/retention decision |
| `<browser> --version` cannot spawn, exits nonzero, times out, or emits non-UTF-8 | `Process` with process/I/O source | group absent; profile fully erased, or cleanup overrides as below | original `Process` survives successful cleanup |
| normalized version output differs from manifest | `SourceVerification` | group absent; profile fully erased, or cleanup overrides as below | mismatch survives successful cleanup |
| capsule/supervisor/browser launch, DevTools discovery/connection, or handler task fails | `Process` with original source | owned group is closed or group-killed/reaped and profile erased; otherwise active evidence remains | successful terminalization preserves `Process`; live/inconclusive terminalization remains `Process` with cleanup context |
| one job fails but retry/report classification completes | cleanable diagnostic state | group reaped and profile erased before any plan | full run may publish `DiagnosticFull` then returns `Generation`; filtered/incomplete diagnostic returns `Generation` without intent |
| measurement/report derivation fails before a complete diagnostic result | `Generation` | group reaped and profile erased | no publication intent |
| close/wait requires forced group kill and succeeds | original job/launch/handler result | no profile journal | forced fallback alone does not replace the primary result |
| group signal/wait/probe remains live, permission-denied, or inconclusive | `Process` with primary plus terminalization context | active journal/profile and group record preserved | lease is released; next mutation returns `LeaseActive` or `ArtifactTransaction`; checks return `Verification` |
| group is dead but active-to-cleanup rename or opaque erase fails | `ArtifactTransaction` with primary plus cleanup context and safe I/O source | cleanup journal preserved | cleanup failure overrides an ordinary semantic error; next mutation resumes cleanup |
| unexpected internal panic | resume original panic payload after the same terminalization attempt | clean, active, or cleanup evidence exactly as above | no `GeneratorError` is fabricated; next access observes retained evidence |

For all ordinary-error rows, if profile/group cleanup is complete the layout lease
is released before return. If durable evidence must remain, the lease is still
released only after that evidence is synced, so a later mutation can apply the
stated recovery rule. No browser/profile residue can coexist with artifact
transaction intent.

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
domain mutex, one private runtime thread, one handler task and internal supervisor
process per browser, bounded batches from the manifest, and one profile per
batch/retry attempt. Successful/ordinary-error returns are terminal; an
unresolved process group or durable cleanup failure follows SR-03.3 and blocks or
is recovered by the next mutation. CSS adds no dependency or runtime thread.

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

For an interface invocation, `run_from_env` reads `args_os` and no operator
configuration environment variable. The layout front door first detects the one
private SR-03.3 launch-capsule key; it enters supervisor mode only after the
capsule validates against a live journal and otherwise treats that key as `Cli`
without external launch. Flag and command names must be UTF-8;
owner/corpus/source roots retain OS-native path bytes; browser/filter values must
convert to checked UTF-8 `RelativePath`.
Unknown/duplicate flags, missing values, repeated positionals, invalid command,
or command-option mismatch returns `Cli` before domain I/O. Filesystem absence,
canonicalization, identity, manifest, and source failures retain their domain
error kinds.

The packaged layout binary is also the only production supervisor host. Before
a `Generate` request acquires resources, `layout::run` requires the canonical
current executable's file name to be exactly `surgeist-layout-generate`; another
host returns `Generation`. `run_from_env` is therefore the normal production
entry for generation. Check/import requests remain directly library-callable,
and synthetic process tests use only a crate-private injected test host. No
third Cargo binary target or discoverable internal CLI mode exists.

Layout `run` spawns one named private worker thread. Spawn or Tokio runtime-build
failure is `Generation` before resource acquisition. The worker owns a
multi-thread Tokio runtime and a terminal-resource registry. External-input
errors never panic. An unexpected internal panic is caught inside the worker;
SR-03.3 terminalization runs idempotently; the original panic payload is resumed
on the caller after join; and any cleanup failure remains in its durable profile
journal for next-acquisition recovery. It is not mislabeled as success or an
input error. A normal join returns the exact SR-04.5 result only after cleanup.
CSS `run` is synchronous and threadless.

## SR-06 Layout Domain Contract

### SR-06.1 Exact commands and schema-2 manifest

The CLI is:

```text
surgeist-layout-generate --owner-root <path> --corpus-root <path> \
  <generate|check-corpus|check-taffy-corpus|import-taffy> \
  [--browser-path <owner-relative-path>] [--source-root <path>] \
  [--filter <html-relative-file-or-prefix>]
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
id = "<unique exact string; trimmed-nonempty only for surgeist>"
source_root = "<surgeist-or-taffy>"
source = "<unique strict relative path>"
generator = "constrained-html"
status = "active"
# reason is optional for every status
```

The angle-bracket strings above are grammar notation, not accepted literal
values. All objects deny unknown and duplicate fields. Schema 2 adds no required
manifest field relative to the preserved source. `batch_size` is a positive
`usize`; navigation timeout and polling interval are positive `u64` values;
disposition/report counts are nonnegative `usize` values; `retry_count` and all
lifecycle strings/booleans are exactly the shown values. `arguments` contains
exactly 28 entries whose normalized keys are unique, includes normalized
`use-mock-keychain`, and satisfies SR-03.2's switch restrictions. The two
excluded destination directories are exactly the shown unique set in either
order. The Taffy repository, revision, source directory, pre-exclusion count,
destination, source-root kinds/paths, and upstream commit are exact.

`provenance_format` contains `{version}` and
`{repository_relative_executable}` exactly once each and contains no other `{` or
`}` byte, so both generation substitution and offline parsing are unambiguous.

The full report's five counts are CleanFull acceptance expectations, not values
copied into an actual report summary. They retain the preserved nonnegative input
grammar. A nonzero declared `failed_to_generate` remains schema-2 compatible but
cannot be satisfied by `CleanFull`, which has no runtime failure, and therefore
cannot make a corpus current. A scoped declaration supplies only its shown clean
expected `generated` count; the other four scoped summary fields are actual
derived counts, not hidden manifest constants.

Paths are strict normalized relative paths. `cache_root` resolves beneath owner
and outside corpus; the CLI browser executable resolves beneath that exact cache
root. Report files are unique one-component JSON names; full is exactly
`all.json`; scoped filters/files are unique. Taffy and Surgeist share the `html`
physical root but have disjoint manifest ownership. Case IDs are unique by exact
UTF-8 bytes and case sources are unique after `RelativePath` parsing. A
`source_root = "surgeist"` ID must be nonempty after trimming, its source ends in
`.html`, and its record owns that authored file. A `source_root = "taffy"` record
may have any ID, status, reason, and strict source path; exactly as in the
preserved implementation, it is a compatibility/uniqueness record with no
fixture-ownership, filtering, disposition, or report effect. `source_root =
"html"` and every other value are rejected. `generator` is exactly
`constrained-html`; status is exactly `active`, `expected-fail`, `unsupported`, or
`quarantined`; reason is absent or any UTF-8 string. Imported Taffy ownership
comes only from the verified sidecar. Every non-Taffy HTML file appears exactly
once as a Surgeist case; unknown HTML entries fail inventory validation.

Layout disposition accounting remains in shared core through a crate-private
`PreservedLayoutDisposition` adapter; the stricter public
`CaseDispositionRecord` contract does not change. The adapter retains the exact
manifest ID/source/status. An active case ignores an optional declared reason.
For a non-active case, a present reason is emitted byte-for-byte, including an
empty or padded string; an absent reason becomes exactly `manifest marks case
expected-fail`, `manifest marks case unsupported`, or `manifest marks case
quarantined`. These are the preserved report rules. Case-array order has no
semantic effect; generation/report ordering remains fixture/variant sorted.

Compatibility is **canonical-schema-2 compatible**, not acceptance-set fully
backward-compatible. No required field, source-root spelling, lifecycle literal,
case ID/reason rule, Taffy compatibility-record behavior, launch digest, browser
pin field, or schema version changes. The complete and only deliberate
tightenings of inputs formerly accepted by the loose parser are:

1. case sources must already be strict normalized `RelativePath` strings; legacy
   spellings containing `.` components are rejected instead of normalized;
2. the Taffy exclusion vector contains each of the exact two values once;
3. launch strings undergo SR-03.2 normalization and reject duplicate normalized
   keys, non-printable/path-bearing strings, driver-owned or redirecting keys,
   and malformed leading-hyphen forms. One optional leading `--` is normalized
   before invocation while the raw manifest spelling remains in the digest;
4. `browser.provenance_format` requires each of its two preserved placeholders
   exactly once and rejects any other brace, closing the loose parser's ambiguous
   repeat/unknown-placeholder acceptance.

Named compatibility fixtures are
`layout_schema2_preserves_taffy_compatibility_records`,
`layout_schema2_preserves_raw_ids_and_reason_defaults`,
`layout_schema2_rejects_html_source_root`,
`layout_schema2_rejects_duplicate_ids_and_sources`,
`layout_schema2_rejects_only_declared_tightenings`,
`layout_schema2_launch_digest_preserves_manifest_order`, and
`layout_schema2_launch_switch_set_is_order_independent`. Each acceptance fixture
is run through both a preserved-parser test adapter and the new parser/effective
representation; rejection fixtures name the one deliberate divergence. A
full-field TOML golden fixes all domain values and launch digest.

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

No sorting is applied before this digest because raw manifest order is preserved.
Invocation uses only the normalized unordered semantic set from SR-03.2.

The Taffy sidecar is an artifact migration, not a manifest migration. A legacy
schema-2 corpus without it returns `Verification` from generate/check with the
instruction to run `import-taffy --source-root ...`; import atomically adds the
sidecar while preserving authored HTML. Sidecar-free import classification is
defined in SR-06.2 and does not guess from file extension or bytes alone. A future
layout-owned adoption must run, review, and commit that one corpus migration
before switching its scripts. It also runs the named schema-2 compatibility
fixtures against its owned manifest; if that manifest hits one of the three
declared tightenings, the layout-owned handoff must review and commit the
corresponding normalization before adoption. This repository never performs
either sibling migration.

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

Import first derives the exact desired Taffy destination set from the verified
pinned snapshot after exclusions and requires it to be disjoint from the exact
manifest-owned Surgeist set. Current `html` ownership is then classified in one
of two mutually exclusive modes:

1. **Sidecar mode:** the reserved sidecar must parse and validate; current
   Taffy-owned files are exactly its listed paths. A malformed sidecar never falls
   back to legacy mode.
2. **Sidecar-free legacy mode:** the sidecar is absent, the manifest is the
   accepted schema-2 contract, and the explicit source verifies at its exact pin.
   Current Taffy-owned paths are the intersection of present regular files with
   the derived desired destination set. Missing desired files and byte differences
   are classifiable stale state that import replaces; a present path outside the
   desired Taffy set and authored set is unknown and fails. No path may collide
   with an authored file, and aliases, symlinks, hard links, wrong modes, mounts,
   or non-file leaves fail before intent.

In either mode, admitted inventory is only the reserved sidecar when present,
classified Taffy files, exact manifest-owned authored files, and directories
implied by those disjoint file sets. Before publication every authored file is
read through the held rooted authority, retained byte-for-byte, and closing-
revalidated under SR-03.2. A missing/replaced authored file or any unknown entry
fails without mutation. The desired clean-full tree is the new Taffy snapshot,
new sidecar, and byte-identical authored files. Thus stale old Taffy files are
removed atomically, authored fixtures cannot be deleted or rewritten, and no
report/XML is touched. `check-taffy-corpus` requires the persisted sidecar and
performs the same source comparison read-only; an absent sidecar is
`Verification` with the import instruction. `check-corpus` likewise requires
only the persisted sidecar and verifies its pin/count/digests against manifest
and HTML without Git.

Named migration tests are
`layout_taffy_legacy_nonempty_migration_adds_sidecar`,
`layout_taffy_legacy_missing_and_stale_files_become_current`,
`layout_taffy_legacy_unknown_file_is_not_guessed`,
`layout_taffy_authored_destination_collision_is_rejected`,
`layout_taffy_malformed_sidecar_never_falls_back`, and
`layout_taffy_authored_revalidation_precedes_import_intent`. The nonempty fixture
contains at least two authored files, three Taffy files, one stale Taffy file,
and one initially missing desired file.

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
mapping. Its first line is the generated-by comment with attributes in this exact
order: `schema`, `source`, `source-sha256`, optional
`linked-resource-sha256`, `helper-sha256`, optional `base-style-sha256`,
`browser`, `browser-executable-sha256`, `launch-profile-sha256`,
`corpus-manifest-sha256`, `taffy-revision`, and
`taffy-sidecar-sha256`. Required digests are lowercase 64-hex; the Taffy revision
is the exact 40- or 64-hex sidecar revision. `browser` is the exact manifest
`provenance_format` after replacing `{version}` and
`{repository_relative_executable}`. Raw browser SHA-256 is therefore persisted
separately rather than implied by that string.

The preserved attribute renderer replaces, in order, `&` with `&amp;`, `"` with
`&quot;`, and `<` with `&lt;`; it does not replace `>`. Preserved text rendering
replaces only `&` and then `<`; quotes and `>` remain literal. The linked-resource
value, when nonempty, is the strictly path-sorted comma-joined list
`<strict-relative-path>=<sha256>` and is omitted when empty. The constrained
contract currently produces the empty list. `base-style-sha256` exists exactly
when the snapshotted source contains the literal `test_base_style.css`. The
checker requires the comment as the first line, rejects duplicate/unknown/
misordered attributes, and requires exactly the applicable optional fields.
Unsupported measured variants have no XML and are reported with reason.

Generation computes `browser-executable-sha256` from the held executable at the
validated launch boundary. `check-corpus` is intentionally offline: it receives
no browser path, never opens the owner cache/executable, and does not claim to
authenticate that historical byte digest. It recomputes every corpus-derived
comment/report value—source and linked-resource digests, helper/base-style,
launch-profile, manifest, Taffy revision/sidecar, and complete XML output digest.
It requires report browser source/version to equal the manifest, parses the
recorded provenance through the manifest format into one strict owner-relative
executable beneath `cache_root`, requires lowercase SHA-256 grammar for the
recorded executable digest, and requires exact browser provenance/digest equality
between full/scoped reports and every XML comment. These values are a historical
generation attestation only: an absent, replaced, or byte-drifted cache executable
does not change offline check results, and a canonical self-consistent rewrite of
every full/scoped report, XML attestation, and resulting XML output digest is
accepted. README/rustdoc state this non-authentication boundary; generation
remains the command that revalidates and rehashes the actual executable.

This is the complete-byte zero-layout golden, including its one final LF (the
repeated digits are valid example digest/revision values):

```xml
<!-- generated-by: surgeist-layout-generate schema=2 source="html/group/case.html" source-sha256="0000000000000000000000000000000000000000000000000000000000000000" helper-sha256="0000000000000000000000000000000000000000000000000000000000000000" browser="Chrome 1 cache/chrome" browser-executable-sha256="0000000000000000000000000000000000000000000000000000000000000000" launch-profile-sha256="0000000000000000000000000000000000000000000000000000000000000000" corpus-manifest-sha256="0000000000000000000000000000000000000000000000000000000000000000" taffy-revision="1111111111111111111111111111111111111111" taffy-sidecar-sha256="0000000000000000000000000000000000000000000000000000000000000000" -->
<test name="case__border_box_ltr" use-rounding="false">
  <viewport width="0px" height="0px"/>
  <input>
    <div/>
  </input>
  <expectations>
    <node x="0" y="0" width="0" height="0"/>
  </expectations>
</test>
```

Full generation serializes all manifest-declared report files beneath
`xml/generation-reports/`. `all.json` has exactly `metadata`, `filter: null`,
`summary`, `generated`, `unsupported`, `expected_fail`, `quarantined`, and
`failed_to_generate` in that order. Metadata fields are exactly, in order:
`schema_version: 2`, `generator: "surgeist-layout-generate"`,
`browser_source`, `browser_version`, `browser_provenance`,
`browser_executable_sha256`, `launch_profile_sha256`, `helper_sha256`,
`base_style_sha256`, `corpus_manifest_sha256`, `taffy_revision`, and
`taffy_sidecar_sha256`. Every digest/revision has the grammar above.

Generated entries are exactly
`{name, source, output, output_sha256, variant}`; `output_sha256` is the lowercase
raw SHA-256 of the complete XML bytes including the final LF. Unsupported entries
are `{name, source, variant, reason}`; expected-fail, quarantined, and failed
entries are `{name, source, reason}`, each in shown field order. All buckets are
deterministically sorted by `(source, name, variant-or-empty, output-or-empty,
output_sha256-or-empty, reason-or-empty)`. The report decoder's duplicate-member
prepass and typed objects reject duplicate or unknown fields at every level. Each
scoped/diagnostic report carries the same digest for each generated entry and
identical metadata. Every summary field always equals the exact length of its
corresponding bucket. A full result has one complete fixture outcome ledger. Each
Taffy or active Surgeist fixture has either one failed entry and no generated/
unsupported variant entry, or exactly four variant entries partitioned between
generated and unsupported. Each expected-fail Surgeist fixture has exactly one
expected-fail entry plus that same failed-or-four-variant job outcome. Each
manifest-unsupported fixture has one unsupported entry with
`variant = "manifest"` and no job outcome; each quarantined fixture has one
quarantined entry and no job outcome.

A `CleanFull` ledger has no failed entry, matches all five full clean expectations,
and each scoped report is the exact SR-04.5 matcher subset whose generated count
matches its one declared clean expectation. Its other scoped summary fields remain
their actual bucket lengths. A `DiagnosticFull` ledger has at least one failed
entry; its full and scoped summaries are actual bucket lengths and are never
rewritten to manifest expectations. For the full diagnostic, the manifest
`generated`, `unsupported`, and `failed_to_generate` CleanFull expectations are
not compared; `expected_fail` and `quarantined` still match exactly because runtime
job failure cannot change manifest dispositions. A scoped diagnostic with no
failed browser-job fixture must still meet its declared generated expectation. If
that scope contains at least one failed browser-job fixture, its declared
generated value is not compared and remains a clean-run expectation only;
failures outside the scope do not relax it. Every scoped bucket is the exact
sorted subset of the full ledger under the same component matcher. Any other
count/coverage discrepancy is an incomplete diagnostic result: return
`Generation` without publication. Serde
pretty JSON uses two-space indentation and one final LF.

The complete single-generated report golden binds the XML golden above; its
`output_sha256` is the SHA-256 of that exact fenced XML payload:

```json
{
  "metadata": {
    "schema_version": 2,
    "generator": "surgeist-layout-generate",
    "browser_source": "chrome-for-testing",
    "browser_version": "1",
    "browser_provenance": "Chrome 1 cache/chrome",
    "browser_executable_sha256": "0000000000000000000000000000000000000000000000000000000000000000",
    "launch_profile_sha256": "0000000000000000000000000000000000000000000000000000000000000000",
    "helper_sha256": "0000000000000000000000000000000000000000000000000000000000000000",
    "base_style_sha256": "0000000000000000000000000000000000000000000000000000000000000000",
    "corpus_manifest_sha256": "0000000000000000000000000000000000000000000000000000000000000000",
    "taffy_revision": "1111111111111111111111111111111111111111",
    "taffy_sidecar_sha256": "0000000000000000000000000000000000000000000000000000000000000000"
  },
  "filter": null,
  "summary": {
    "generated": 1,
    "unsupported": 0,
    "expected_fail": 0,
    "quarantined": 0,
    "failed_to_generate": 0
  },
  "generated": [
    {
      "name": "case__border_box_ltr",
      "source": "html/group/case.html",
      "output": "xml/group/case__border_box_ltr.xml",
      "output_sha256": "04dd77a3fca470f65858a35b059a34a146031adbc5dd80931dd8cbe508dacb6a",
      "variant": "border_box_ltr"
    }
  ],
  "unsupported": [],
  "expected_fail": [],
  "quarantined": [],
  "failed_to_generate": []
}
```

Named complete-byte tests are `layout_xml_provenance_complete_golden`,
`layout_xml_optional_provenance_complete_golden`,
`layout_xml_preserved_escape_complete_golden`,
`layout_report_generated_digest_complete_golden`, and
`layout_provenance_rejects_duplicate_unknown_or_misordered_fields`.
The preserved-escape golden contains `&`, `"`, `<`, and `>` in attribute and text
inputs and proves the exact replacements and literal `>` bytes above. Diagnostic
count/byte tests are `layout_diagnostic_full_failure_inside_scope_golden`,
`layout_diagnostic_full_failure_outside_scope_golden`,
`layout_diagnostic_summary_is_actual_bucket_lengths`, and
`layout_diagnostic_rejects_unexplained_count_divergence`.

Offline-attestation tests are `layout_check_ignores_absent_browser_cache`,
`layout_check_ignores_replaced_browser_identity`,
`layout_check_ignores_browser_byte_drift`,
`layout_check_rejects_cross_artifact_browser_digest_mismatch`, and
`layout_check_accepts_self_consistent_historical_browser_attestation_rewrite`.

`check-corpus` recomputes each generated entry's digest before accepting its XML
provenance/body. A body edit with an unchanged valid first-line comment is stale
`Verification`, not current. Filtered generation preserves reports: if every
selected variant remains generated with the recorded digest, the prior full
report may remain current; any changed selected digest or selected transition to
unsupported/removal makes the composed corpus stale until a clean full run
replaces the report. Named behavior tests are
`layout_report_rejects_tampered_xml_body`,
`layout_filtered_digest_change_makes_preserved_report_stale`, and
`layout_filtered_unsupported_removes_owned_xml_and_stales_report`.

The publication matrix is exact:

| Run outcome | Policy/result | Artifacts | Stale/unknown behavior |
| --- | --- | --- | --- |
| Full, every browser job reaches a classified generated/unsupported result, no lifecycle failure occurs, and all full/scoped CleanFull expectations match | `CleanFull`, then `Ok` | all generated XML plus every full/scoped report | remove historical-union members absent from the desired set; unknown entry fails before intent |
| Full, one or more jobs exhaust retry but SR-03.3 terminalization succeeds and a complete diagnostic report exists | `DiagnosticFull`, install, then return `Generation` | successful current XML plus all current diagnostic reports; failed entries have no XML | remove every historical-union XML/report absent from that installed set, including superseded output for failed or removed jobs; unknown entry fails before intent |
| Filtered, every possible scheduled selected output is historically owned and all selected jobs succeed/classify | `Filtered`, then `Ok` | replace selected generated XML and remove selected historically owned XML now classified unsupported | write no report; preserve every unselected XML/report; remove no unselected path; an unowned scheduled output is `Verification` before lease |
| Validation before lease | no plan/install; exact pre-lease kind from SR-04.5 | none | prior generated tree remains; no profile exists |
| Version/launch/handler/filtered-job/incomplete-report failure with successful profile terminalization | no plan/install; `SourceVerification`, `Process`, or `Generation` exactly per SR-04.5 | none | prior generated tree remains; no profile residue |
| Browser/profile terminalization failure | no plan/install; `Process` or `ArtifactTransaction` exactly per SR-04.5 | none | prior generated tree remains; active/cleanup evidence retained |
| Unexpected internal panic | no plan/install; original payload resumed after terminalization attempt | none | prior generated tree remains; any cleanup evidence retained |
| Artifact transaction fails before commit and synchronous recovery succeeds | error per SR-04.5 | none installed | prior absent/old tree remains; no transaction residue |
| Artifact transaction commits but root sync/outcome/cleanup reports failure | return `ArtifactTransaction` | complete new XML/report set remains visible | recovery completes or retains valid resumable evidence; never restore old |

Desired XML is the four exact paths for every fixture in the current validated
Surgeist-case-plus-Taffy-sidecar inventory; historical XML/report ownership comes
only from the validated SR-04.5 authority. The classifiable set is their union.
Retained desired XML is only
successfully generated variants, and current report paths are exactly the
manifest-declared full/scoped set. Unsupported/quarantined and removed/renamed
historical paths are therefore removed only by a complete `CleanFull` or
`DiagnosticFull` publication. Filtered publication cannot write reports. Check commands acquire/
finish a shared guard, create/recover nothing, recompute every XML output digest,
and verify exact union inventory, report relationships, provenance, and absence
of unknown entries.

Browser-independent synthetic adapters and byte goldens cover HTML injection,
URLs, four-variant mapping, measurement conversion, XML, reports, retry,
clean/diagnostic/filtered matrices, the full SR-03.3 supervisor/profile lifecycle,
and CLI errors. No test launches or downloads a browser or reads a sibling
corpus.

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
derived case. A derived case without an override defaults to active. The
expectation-relative path `generation-reports/all.json` is reserved exclusively
for the report and can never be derived from a source fixture.

### SR-07.2 Import sidecar and publication

Import verifies the explicit checkout and immutable snapshot under
`fixture_root`, accepts only regular Git mode `100644` JSON, requires exact file
count, and rejects both the reserved root `.surgeist-source.json` and the fixture
path `generation-reports/all.json`. That collision is `InvalidInventory` before
any import intent even though the upstream file is otherwise valid JSON. The
canonical sidecar contains schema 1, full source pin, object format, exact file
count, and sorted path/Git-mode/blob-ID/SHA-256 records. It and snapshot bytes are
published as one clean-full `import_root` transaction.

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
order and shape, except that the reserved report-relative path is rejected on
every sidecar/import read and is never mapped:

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
| Filtered generate success after every selected expectation is proven historically owned | `Filtered` | selected expectations only; no report write/removal and no stale removal; unowned selection is `Verification` before lease |
| Validation/derivation failure before plan | no install | prior expectation tree remains |
| Artifact failure before commit with successful recovery | no completed install | prior absent/old expectation tree remains |
| Artifact failure after commit | return `ArtifactTransaction` | complete new expectation/report root remains; cleanup completes or retains resumable evidence |

Desired classification is expectations mapped from the current validated import
sidecar plus its fixed report; historical classification comes only from the
validated SR-04.5 full report. Any entry outside their union fails before intent.
A persisted old/malformed sidecar or historical authority that lists
`generation-reports/all.json` as a fixture/output is `InvalidInventory`; full
generation, filtered generation, and checking all fail before interpreting that
path as either an expectation or report. A full clean run removes stale
historical expectations absent from the new exact set; it never guesses
ownership from extension alone. `check-corpus` performs no Git or mutation and
validates manifest, sidecar/files, every expectation byte/schema, counts, report
relationship, hashes, and the exact historical-plus-desired inventory.

Byte-golden tests cover ordinary/error cases, escaping, canonical options,
default/override dispositions, repeated source paths with unique IDs, duplicate
members, malformed/empty fixtures, sidecar drift, report provenance/counts,
unknown entries, stale full removal, filtered preservation, and the named
`css_import_rejects_report_path_collision`,
`css_full_generate_rejects_persisted_report_path_collision`,
`css_filtered_generate_rejects_persisted_report_path_collision`, and
`css_check_rejects_persisted_report_path_collision` cases.

## SR-08 Errors, Documentation, And Verification

### SR-08.1 Errors and CLI proof

The existing non-exhaustive `GeneratorErrorKind` set remains stable. Malformed
syntax/option matrices map to `Cli`; manifest schema to `InvalidManifest`;
fixture/current-tree shape to `InvalidInventory`; pin/snapshot drift to
`SourceVerification`; unsupported mutation target to `UnsupportedPlatform`;
mutation contention/live recorded groups to `LeaseActive`; browser spawn,
version execution, DevTools, handler, signal, and wait failures to `Process`;
job/diagnostic derivation failures to `Generation`; artifact/profile corruption
or failed durable recovery/cleanup to `ArtifactTransaction`; and known stale,
absent, active, or malformed coordination observed by a read-only check to
`Verification`. A version-output mismatch is `SourceVerification`; an ordinary
filesystem operation outside a durable transaction remains `Io`. Exact
post-commit and cleanup precedence is SR-04.5. Safe I/O/process sources are
preserved. External input never asserts or panics.

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
with a fixed cleared environment, explain internal supervisor/profile recovery
and the operator action for a still-live orphan group, and explain the one-time
Taffy-sidecar/schema-tightening adoption checks. They do not call the crate a
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
documentation/handoff normalization until both features are executable. The
`surgeist-agent` skill is the sole execution and publication authority; this
specification does not restate or redefine that gate.

The terminal tree retains canonical `plans/.gitkeep`, the already committed
baseline review, and only the reviewed specification, implementation-sequence,
and cycle-plan paths required by the candidate handoff. Planning-review verdicts,
worker/reviewer transcripts, command evidence, and handoff chatter remain
task-local and are embedded in the canonical candidate record; they are not
persisted as additional planning documents. The handoff names each canonical
planning path and reviewed revision together with the immutable published leaf
SHA and exact feature/command/verification contract. Root and sibling adoption
remain separate future work. Execution-resource cleanup remains solely governed
by `surgeist-agent`.
