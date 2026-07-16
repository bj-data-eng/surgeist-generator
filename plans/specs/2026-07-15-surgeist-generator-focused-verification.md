# surgeist-generator Focused Verification Contract

This normative companion to
`2026-07-15-surgeist-generator-shared-corpus-drivers.md` owns the complete
focused-test contract for the same semantic revision. Neither file is complete
without the other; the main specification owns behavior and final gates, while
this file owns the synthetic evidence required before those gates.

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
   the sentinel; a supported-host inventory test accepts the real developer-
   `usr`-confined `libexec/git-core/* -> ../../bin/git` links without fetching or
   allowing a unit escape; its recursively enumerated commit-tree snapshot includes fixtures below
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
   an accepted linked worktree inventories benign `config.worktree` records
   under the exact worktree scope/origin while leaving filter/diff/textconv
   sentinel programs unexecuted; with `extensions.worktreeConfig=true`, each
   worktree-scoped include/includeIf, URL rewrite, duplicate or wrong origin,
   credential helper, SSH command, custom helper, and protocol/ext sentinel is
   rejected before the sentinel runs, while a `config.worktree` file with the
   extension absent/false, a forged origin, or a config identity/inventory change
   before the closing recheck also fails closed. The test-only
   local acquisition transport fetches an exact object into a fresh
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
   final—never an unjournaled/empty/partial poisoned final. Exact interleavings
   cover a winner publishing after a dead loser created but did not register its
   zero stage, and a live loser observing a contended valid final, publishing
   `lost-contended`, cleaning only itself, and returning `LeaseActive`. The
   latter race proves release-before-marker: a third cleaner cannot claim while
   the internal stage lock is held, and after release exactly one of creator or
   third cleaner reacquires that lock then wins the name claim; a losing live
   creator performs no descriptor cleanup. Two
   simultaneous recoverers race over an empty pre-intent directory, every
   partial-intent temporary, and a dead `recovering` claimant; exactly one atomic
   name claimant mutates each state while the loser closes its descriptor and
   rescans. Injected claimant death after claim rename and after every cleanup
   unlink/sync is resumed by a new claimant without an advisory gate; plan install
   accepts only a still-live matching corpus/domain lease and rejects a released,
   foreign-domain, or foreign-corpus lease before probes or writes; read-only
   verification takes/contends on the shared gate/mutex without creating state,
   and holds a pre-bootstrap shared gate for the complete check. Two distinct
   corpora sharing one canonical owner-target and cache unit derive the same
   byte-exact cache key, contend on one key mutex, and recover only that key's
   journal before either corpus mutex; alias target identities converge or fail.
   Passing schema 1 to layout or schema 2 to CSS fails `InvalidManifest` before
   coordination, under-gate manifest replacement fails before domain bootstrap,
   and a persisted opposite-domain lease/transaction scaffold fails mutation as
   `ArtifactTransaction` and checking as `Verification` without adoption or
   removal;
8. artifact publication creates/syncs intent and old sidecar before registering
   an external stage, then deterministically builds and syncs one complete staged
   root, removes stale files only in a clean full staged state, and changes the
   final namespace with exactly one root transition; injected process death after
   active-directory creation, complete `intent.json` before absent/partial
   `old-inventory.json`, every metadata temporary write/sync/rename, stage
   registration, mid-file/tree construction, prepared publication, and before the swap
   leave the complete old final, process termination immediately before/after the
   swap recovers respectively to old/new, and termination during aborted-stage,
   committed-old-stage, or completed-journal-tombstone cleanup resumes from the
   exact recorded subset. Active `cleanup-complete`, completed rename, every
   metadata deletion, receipt deletion, and final `rmdir` are independently
   resumed. A partial or complete old sidecar without complete intent is rejected
   as unreachable and preserved. Empty pre-intent cleanup and a lone partial
   intent temporary cover death before/after the private cleanup-receipt
   temporary, receipt rename/sync, intent-temporary unlink/sync, receipt unlink/
   sync, directory removal, and parent sync; only a complete receipt can
   authorize removal of its exact listed subset. The intent-only or
   intent-plus-partial-old state recomputes/binds and publishes the complete old
   inventory, then records `aborted` and follows ordinary receipt cleanup.
   Marker or old-stage cleanup failure after the swap
   preserves the complete new final plus a read-only-detectable journal that the
   next lease clears; no test expects rollback after commit and no observed state
   is a mixed generation. Nested-directory cleanup crashes after every child
   removal and resumes despite the resulting parent `st_nlink` change; directory
   link count is absent from both sidecars/digests, while nondirectory counts and
   every directory identity/type/mode/mount/remaining-child check still bind.
   Descriptor-bound tests replace roots,
   components, stage/final names, journals, and destination identities before
   each transition and prove no escape, overwrite, or disputed-object removal;
   no public plan or lease exists, internal plans have no arbitrary output-root
   constructor, a `CorpusLocation` rooted at/below coordination is rejected, and generated, retained, artifact,
   report, clone, and stale paths reject filesystem-equivalent coordination or
   transaction names at all depths. Exact-parent case/normalization probes use
   different injected policies in a parent and child and reject only where the
   actual parent aliases the pair; injected macOS device/fsid changes stop
   traversal. Corpus mode tests require roots/directories `0755` and every
   imported/retained/generated regular file `0644`, preserve those modes through
   full and filtered clones, and reject executable Git fixtures, wrong existing
   modes, umask-altered creation, hard links, and special entries before commit.
   Cache tests repeat the state matrix under the matching key guard,
   verify immutable cache units publish only with `RENAME_EXCL`, reuse valid
   units, reject invalid units without replacement, accept only the browser's
   exact five link targets, and prove a corpus lease never recovers cache state.
   Two Taffy pins with the same revision/subdirectory but different repository
   URLs contend on one cache key; the second rejects the first unit's canonical
   `.surgeist-source.json` and cannot report/relabel it as its own source;
9. hash text validation and report counts/provenance detect drift, shared reports
   accept structurally valid failed/unsupported counts without fictitious
   artifacts while domain validators enforce their own artifact/count mapping,
   and every `GeneratorErrorKind` has the exact SG-12 exit code. Table-driven
   failures prove caller canonicalization remains `InvalidPath`, validated-input
   I/O remains `Io`, durable transaction failure overrides an earlier operation
   kind as `ArtifactTransaction`, browser terminal cleanup is `Generation` only
   without a durable dispute, and read-only drift is `Verification` while a read
   error remains `Io`. The same table covers probe capability versus unresolved-
   journal precedence, missing-object verification versus Git process failure,
   launch-time `Process` versus cleanup-time `Generation`, every
   `collect_regular_files` branch, and unmatched `RunScope::require_match`;
10. an unregistered external stage/tombstone is rejected even when its final target is
    absent; compile-time mutation selection is exactly
    `aarch64-apple-darwin`; every mutation entry point in the already-installed
    WASM nonmutation build fails before coordination/cache/import/artifact/report
    mutation; supported-host rename probe failure leaves no domain mutation and
   reports any private residue; test documentation states that non-cooperating
   namespace mutation while leased is unsupported; missing macOS device/fsid or
   an inconclusive exact-parent name probe fails `UnsupportedPlatform` after
   verified private-probe cleanup, while an identity change, cleanup failure, or
   durable probe journal returns `ArtifactTransaction` and preserves the residue;
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
   DevTools parsing. A byte-level logical-inventory golden hashes `dir` as
   `D/0755`, `dir/file` as `F/0644` with bytes `abc`, and `dir/link` as
   `L/0755 -> file` to exactly
   `808368d7905aedc20e4b8cf50df818d1f18b01abbcfea5db08a4b58b3764aae6`;
   substituting link mode `0777`, including file-type bits, or changing field
   endianness must not match. The compiled pin table asserts its exact evidenced
   `5ef8a535ec2e28729c989886a728517681a4f30c18819e98dd2cbe018bd3070a`
   row. Synthetic archives cover early links, descendants after
   links, path escapes, duplicates/conflicts, cycles, special/hard-link modes,
   corrupt CRC, limits, and a byte change that misses the content pin without
   creating an outside sentinel. Injected browser acquisition boundaries assert
   exact SG-12 kinds for Reqwest transport/body I/O, redirect/status/size, every
   ZIP/content-pin/cache defect, version spawn/status/output mismatch, launch/
   DevTools failure, and cleanup-precedence combinations. Sanitized fresh-bare Taffy acquisition crosses
   only the local sentinel boundary, without launching a browser, using the
   network, or checking out a worktree. Test-only Git children/helpers cover
   parent death before spawn, after `launching`, after child/group publication,
   while a helper still writes the registered stage, after leader exit, and at
   `owner-terminal-unrecorded`/`reaped`/`orphan-group-absent`; recovery never removes a stage while a recorded
   process/group exists, never signals from recovery, and resumes only terminal
   slots. Init/fetch timeout and output overflow exercise the one group signal,
   owned reap, group-absence wait, and no-detach rule. A restrictive `077` umask
   still leaves every Git directory traversable; injected death after each
   directory/file normalization step resumes a mixed `0700`/`0755` and
   pre-`0644`/`0644` construction subset, while injected owner-unsearchable
   directories and mode-`0000`/write-only files are preserved as disputed rather
   than entered, reopened, or removed. Caught-
   panic injection after storing the child slot, after child/group marker
   publication, while an init/fetch helper remains active, after each one-time
   group signal/reap transition, and while each output reader is draining proves
   the outer supervisor reaches the same terminal path. Reader spawn failure,
   reader I/O error/panic, overflow, and delayed EOF leave no child, helper,
   pipe, or thread handle detached; both drain joins resolve before any terminal
   live-owner process marker or cache-lock release;
6. representative XML shape, four variants, numeric formatting, layout fields,
   and unchanged generated-by provenance;
7. disposition accounting, full/filtered isolation, and offline drift rejection;
   a synthetic long cache phase lets a second `import-taffy` with another cache
   key acquire the `layout` mutex and swap HTML after generation's preliminary
   filter match; generation then acquires the mutex, recollects authoritative
   manifest/helper/HTML/case/count/filter state, and either uses only the new
   snapshot or fails before browser/profile/transaction work—never stale bytes;
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
   after all synthetic resources are registered and injected supervisor panics
   immediately before/after every registry take, close, timeout, signal, reap,
   abort, join, profile step, guard release, and runtime-shutdown transition, a successful close with delayed
   child exit, a hung page close, and a hung handler exercise the exact 2/5-second
   graceful deadlines, one kill/abort, and unbounded terminal wait. Deadline
   cases use Tokio's `test-util` support in a current-thread paused-time test
   runtime; the production worker remains a private multi-thread runtime, and a
   separate test retains the already-inside-a-runtime call coverage. A synthetic
   group whose leader exits before a surviving descendant proves the non-reaping
   leader reservation, one group signal, leader reap, and final group `ESRCH`.
   The test
   proves the public call remains blocked past the forced grace interval, then
   releases the synthetic resource and observes child reap and a resolved join.
   Every handler is completed or aborted-and-joined, the profile is removed or
   retained only with its durable run intent, and a
   distinct-owner contender can acquire the released same-corpus lease; the
   panic maps to `Generation`, no final artifact, stale output, or report changes,
   and no detached-task counter remains. Supervisor-transition panics resume only
   after the same zero-live-child/task/runtime counters and released guards are
   observed. No test relies on implicit drop for a live child/task; the final
   owned Tokio `Runtime` destructor runs only after all async handles are terminal,
   waits for zero workers without a timeout, and injected pre/post-drop panics
   never call `shutdown_timeout`/`shutdown_background`. Death after run intent, profile
   registration, `launching`, `spawn-failed`, child PID, `group-verified`,
   `group-mismatch`, live-owner `reaped`, recovery `orphan-group-absent`, profile
   cleanup, and run-tombstone transitions proves that uncertain child/group state
   is preserved while either exact terminal proof resumes cleanup. Profile
   cleanup fixtures cover every admitted file type/mode, regular hard links
   whose outside name/bytes remain untouched, arbitrary/escaping symlink targets
   that are unlinked without traversal, and every rejected owner/type/mode/mount
   state. Death after sidecar publication and each reverse-order unlink/sync/root-
   absent marker resumes the exact subset, including nested-directory parent
   link-count changes; an injected replacement or new name
   preserves the remainder. Item 7 separately covers recoverable
   case-assigned page/measurement failures.

CSS tests shall use official-shaped synthetic fixture JSON and local temporary Git
repositories to prove:

1. exact-pin snapshot import, deterministic JSON-only copying, and the exact
   canonical `.surgeist-source.json` golden with complete source/object format,
   fixture-only count, sorted paths, `100644` modes, blob IDs, and raw-byte
   SHA-256 values; a root-sidecar-name collision fails before lease/write. A same-directory-
   identity content change after snapshotting cannot change imported bytes,
   a protected-directory identity replacement before final revalidation that
   returns `InvalidPath` and publishes nothing, count validation, and stale source
   removal; crash injection around the root swap proves old fixtures never
   coexist with new provenance or conversely, and exact reimport is deterministic;
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
   provenance, count, and stale-inventory drift. After import, changing any
   manifest repository/revision/fixture-root pin makes full and filtered
   generation fail `SourceVerification` and checking fail `Verification` without
   writes. Editing any imported byte then asking generation to run fails
   `InvalidInventory`; generation cannot bless or rewrite the sidecar, including
   when the edited fixture is outside a filter. A verified reimport atomically
   repairs/replaces the unit before generation/check can pass. Missing,
   noncanonical, duplicate/unknown-field sidecars and wrong source, object
   format, count, order, path, mode, blob-ID width, digest, missing/extra entry,
   expectation import-provenance digest, or report `csstree-import` digest each
   fail with the phase-specific SG-12 kind.

No focused test reads or executes the real layout or CSS repository corpus.
