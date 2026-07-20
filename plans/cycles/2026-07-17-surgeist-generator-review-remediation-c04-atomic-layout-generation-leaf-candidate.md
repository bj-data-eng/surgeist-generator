# C04 Atomic Layout Generation And Leaf Candidate

## Header

- Cycle ID: `C04`
- Owning repository: `/Users/codex/Development/surgeist-generator`
- Status: `draft`
- Cycle base: `65ae2af6e3b1a2e9640decfa186dc2bf37ae4f7a`
- Immutable implementation-series source base:
  `1e545530fb992e2e749f26a1ba7ac0d77ed85dd6`
- Published prerequisite: C03 candidate
  `65ae2af6e3b1a2e9640decfa186dc2bf37ae4f7a`; local `main`, `origin/main`, and
  authority-remote `main` were equal at the C03 readback.
- Reviewed specification:
  `plans/specs/2026-07-17-surgeist-generator-review-remediation.md`; all
  remaining SR-01 and SR-02 obligations; layout-generation browser/cache,
  executable, launch, environment, profile, and closing-revalidation clauses
  of SR-03.1 and SR-03.2; all of SR-03.3; remaining layout generation,
  historical-authority, clean/diagnostic/filtered publication, and error clauses
  of SR-04.5; final linkage and quality clauses of SR-04.6; SR-05.1, the
  generation capability set in SR-05.2, and the remaining SR-05.4 clauses;
  generation portions of SR-06.1, remaining SR-06.3, and SR-06.4; remaining
  SR-08.1, all of SR-08.2 and SR-08.3; and SR-09; at commit
  `d2fbbedb033177731af5487d3498ba7f14b721d8`, normalized semantic-content
  SHA-256
  `faa4320f1e06ad9c003f2525fcf7171e387458eacc4ec3fd0d2d88f7c0e1eb71`,
  review `CLEAN`.
- Reviewed implementation sequence:
  `plans/sequences/2026-07-17-surgeist-generator-review-remediation.md`, entry
  C04, at commit `faad9c1406b0cda68d9ce087a8cc3e06e6205360`, normalized
  semantic-content SHA-256
  `590c79d705cd9657a649b2a303e01437beda6facb538f08d85f86ae87392e3f6`,
  review `CLEAN`.
- Bounded outcome: add layout generation as one complete production vertical
  slice through the existing public layout module and packaged binary; retire
  the mapped preservation source and every artificial linkage; finish exact
  dependency, license, advisory, documentation, target, and handoff evidence;
  then, only after a clean holistic review and explicit user authorization, run
  the cumulative exhaustive diagnostics once before publishing the terminal
  leaf candidate.

## Boundary

- Mutate, commit, review, and publish only this repository. Root `surgeist`,
  sibling crates, their corpora, Git checkouts, gitlinks, scripts, generated
  artifacts, and API audit artifacts remain outside read, mutation, and test
  scope. Root/sibling adoption starts a separate owning-repository workflow.
- C01 durable transaction/coordination/recovery, C02 protected-source and CSS
  vertical-slice behavior, and C03 browser-free layout import/check/schema/
  historical-authority behavior remain stable foundations. C04 may extend
  crate-private core/profile integration only where the reviewed generation
  contract requires it; it may not weaken path, identity, disjointness,
  inventory, source, durability, cleanup, lease, error-precedence, or read-only
  checking guarantees.
- Production generation is atomic at the task boundary. The feature dependency
  edge, public `Generate` variant/constructor/accessors, CLI matrix, trusted
  browser validation, worker/runtime, authenticated supervisor, durable profile
  recovery, Chromium measurement, XML/report derivation, and clean/diagnostic/
  filtered publication land together in C04-T01. No committed production
  backend-only, dependency-only, placeholder, provisional public request, or
  `Generate` arm that cannot execute the complete domain path precedes it.
- The exact additive public generation surface under `layout-browser` is:
  `LayoutCommand::Generate`;
  `LayoutRequest::generate(CorpusLocation, RelativePath,
  Option<RelativePath>) -> Result<Self>`; `browser_path() ->
  Option<&RelativePath>`; and `filter() -> Option<&RelativePath>`. Existing
  browser-free signatures and semantics remain unchanged. Private request
  storage makes browser/filter payloads constructible only for `Generate` and
  source roots constructible only for Taffy commands. Constructors remain
  I/O-free.
- Final layout CLI syntax is one of `generate`, `check-corpus`,
  `check-taffy-corpus`, or `import-taffy`. Generate requires exactly one
  `--browser-path`, forbids `--source-root`, and optionally accepts one strict
  HTML-relative `--filter`; every other command forbids browser/filter and
  retains the C03 source-root matrix. `run_from_env` reads only `args_os` plus
  the one authenticated private launch capsule. Operator-supplied, forged,
  stale, or incomplete capsule state is `Cli` and starts no process. The binary
  remains at most 15 physical lines and no third target/internal CLI mode is
  added.
- The exact heavy edge is feature-isolated and offline-resolved:
  ```toml
  layout-browser = ["dep:chromiumoxide", "dep:futures", "dep:tokio", "dep:url"]

  chromiumoxide = { version = "=0.9.1", default-features = false, features = ["bytes"], optional = true }
  futures = { version = "=0.3.31", optional = true }
  tokio = { version = "=1.48.0", features = ["fs", "io-util", "macros", "process", "rt-multi-thread", "sync", "time"], optional = true }
  url = { version = "=2.5.7", optional = true }
  ```
  Package version 0.1.0, edition 2024, Rust 1.97, MIT, `default = []`,
  `css-corpus = []`, shared dependencies, target-specific `rustix = 1.1.4`, and
  the two existing required-feature binary targets remain exact. The lockfile is
  regenerated only with `cargo generate-lockfile --offline`; any cache miss,
  MSRV conflict, or inability to resolve without network stops C04.
- C04 adds `deny.toml` with `confidence-threshold = 0.8` and only this license
  allow set: `0BSD`, `Apache-2.0`, `Apache-2.0 WITH LLVM-exception`,
  `BSD-2-Clause`, `BSD-3-Clause`, `BSL-1.0`, `CC0-1.0`, `ISC`, `MIT`,
  `MPL-2.0`, `OpenSSL`, `Unicode-3.0`, `Unicode-DFS-2016`, `Unlicense`, and
  `Zlib`. There are no exceptions, clarifications, or private-crate bypasses.
  Already-installed `cargo-deny 0.19.4` and `cargo-audit 0.22.1` may run
  offline/no-fetch; they are never installed or updated by this cycle. A
  reported advisory or unapproved/unknown license stops work for reviewed plan
  correction rather than policy relaxation.
- No implementation or verification command downloads, installs, launches, or
  acquires Chromium; clones/fetches Git; reads a sibling corpus; or uses a
  production fixture. Production accepts one explicit existing browser path
  beneath the manifest cache root. Tests use synthetic owner/corpus trees,
  injected browser/clock/probe/measurement adapters, and the crate's current
  test executable in fake-browser/supervisor modes. They launch no unowned
  executable. Existing local Cargo caches, compiler components, `cargo-deny`,
  and the installed advisory database are the only external inputs.
- The explicitly supplied browser is a trusted external capability, not a
  sandboxed resource. Generation validates cache containment, executable type,
  single-link identity, raw SHA-256, manifest version output, switch set, and
  closing identity/digest; it uses a fixed cleared environment and supervises
  the recorded process group. It does not claim the executable benign, prevent
  its ordinary writes/spawns outside generator namespaces, prove atomic
  execute-from-held-descriptor behavior on macOS, or control a trusted process
  that deliberately daemonizes/detaches. README and rustdoc state that boundary.
- Generate proves the browser cache is outside the complete corpus root and the
  exact executable is beneath that cache. It protects manifest, HTML, helper
  assets, validated Taffy sidecar/files, complete cache, and executable; writes
  only `xml` and `.surgeist-generator` transaction/lock/profile namespaces; and
  revalidates every protected identity before profile creation/process launch
  and the executable identity/digest before each spawn and after terminalization.
  Unknown current XML/report entries fail before lease or profile creation.
- The manifest parser retains, in one effective representation, the exact
  version output, all 28 raw launch strings, their ordered digest, their unique
  normalized semantic switch set, batch/time/retry values, browser/cache/
  provenance fields, and report expectations. It does not reparse or duplicate
  schema ownership. The driver adds exactly `remote-debugging-port=0`,
  `disable-extensions`, and the OS-native attempt `user-data-dir`; validates the
  exact manifest-plus-driver set at the supervisor; and preserves manifest-owned
  switches and order-independent semantics without treating builder defaults as
  hidden overrides.
- Both version and measurement supervisors use the exact cleared environment,
  cwd, proxy, locale, and profile-owned HOME/TMP/XDG directories from SR-03.2.
  The sole private `SURGEIST_LAYOUT_LAUNCH_CAPSULE` is canonical schema-1 JSON
  without LF, authenticates native owner/corpus path bytes as lowercase hex,
  journal/record digests, parent PID, profile token, browser path, purpose, and
  ordered switches, and is removed by the child's `env_clear` before the trusted
  browser starts.
- Profile state lives only below
  `.surgeist-generator/profiles/layout/`. Version and batch/retry active journal
  names, canonical `intent.json`, immutable `transition.lock`, profile directory,
  canonical `profile.json`, and post-registration `running.json` use the exact
  schemas/modes/order from SR-03.3. No browser starts before the supervisor is a
  process-group leader and the durable running record exists. One journal at a
  time is terminalized or classified before the next is created.
- Normal, failure, timeout, dependency-panic, and repository-panic paths apply
  the exact close/wait/group-kill/reap/handler-abort/group-absence/active-to-
  cleanup/opaque-erase lifecycle. Recovery classifies profile state read-only
  before protected revalidation; signals nothing; treats live, locked, or
  permission-inconclusive groups as active; and mutates one proven-dead plan only
  after closing revalidation. Cleanup identity drift, corruption, or failure
  preserves exact evidence. Artifact intent cannot coexist with profile residue.
- Generation snapshots corpus-owned helper/base-style and HTML/Taffy authority,
  derives the exact full or filtered job ledger, and preserves deterministic
  batching, HTML/base-URL injection, DOM-ready polling, grid-template-area
  capture, four exact variant keys/paths, retry classification, measurement
  conversion, XML escaping/bytes, provenance comments, report schemas/digests,
  disposition accounting, sorting, scoped subsets, and count/coverage rules.
  The crate embeds no helper asset or production corpus revision/count.
- Full clean generation uses `CleanFull`; a complete full diagnostic installs
  `DiagnosticFull` then returns `Generation`; an incomplete diagnostic or
  filtered job failure publishes nothing; a successful historically-owned
  filter uses `Filtered`, writes no report, preserves unselected bytes, removes
  only selected now-unsupported owned XML, and may leave the whole corpus stale.
  Legacy authority requires a complete full migration. Unknown/unowned entries
  fail before intent. Every browser/profile is terminal before any plan is built.
- C03 `check-corpus` remains fully offline and never opens the cache/executable.
  Browser provenance/digest in XML/reports remains a historical attestation.
  Generate is the only command that authenticates/rehashes the actual executable.
  Browser-free import/check and CSS behavior remain unchanged and feature-isolated.
- The 4,626-line preservation copy remains immutable at SHA-256
  `d2f5ca87cea6b36826e9172e2d7ba7a99196c375e2ca53f8a84a075200e70a9f`
  through C04-T01 and its clean task review. Only the assigned C04-T02 worker,
  after rechecking that digest, may open the exact file to build the final
  responsibility map. It may never format, compile, `include!`, or test the
  source. After every retained responsibility maps to compiled code/focused
  tests or a corpus-owned input and every rejected legacy mechanism maps to the
  reviewed replacement, C04-T02 deletes the file rather than retaining a second
  implementation. Other workers/reviewers use the supplied mapping and Git
  deletion metadata; they do not inspect the source.
- C04 replaces every exact artificial reference at the C03 handoff:
  `ArtifactPlan::install`, `ArtifactPlan::artifact_digest`,
  `PublicationPolicy::DiagnosticFull`, `GenerationLease::acquire`,
  `InventoryEntry::{symlink,length,link_target,link_count}`,
  `InventoryPolicy::Private`, `ProtectedSource::verified`, and
  `private_front_doors_are_linked`. Each private item must have a genuine domain/
  profile caller or be removed as unnecessary; no identity function reference,
  fake caller, lint allowance, dead-code suppression, or test-only reachability
  closes linkage.
- Ordinary tests compile and skip exhaustive real durability/byte/recovery/
  process-prefix diagnostics. C04 adds exactly one ignored library diagnostic,
  `layout::profile_tests::layout_profile_cleanup_every_prefix_recovers`, to the
  existing 15. Every task and ordinary/final matrix may use `--ignored` only
  together with `--list` and must observe exactly these 16 names:
  ```text
  core::transaction::tests::production_recovery::transaction_install_every_prefix_recovers
  core::transaction::tests::production_recovery::transaction_recovery_every_prefix_is_idempotent
  core::coordination::tests::bootstrap_header_every_byte_prefix_recovers
  core::coordination::tests::bootstrap_uncontended_every_prefix_recovers
  core::coordination::tests::bootstrap_winner_held_every_prefix_recovers
  core::coordination::tests::bootstrap_winner_released_every_prefix_recovers
  core::coordination::tests::owner_record_install_every_prefix_recovers_absent
  core::coordination::tests::owner_record_install_every_prefix_recovers_swap
  core::coordination::tests::owner_record_recovery_every_prefix_is_idempotent
  core::coordination::tests::rename_probe_install_every_prefix_recovers
  core::coordination::tests::rename_probe_exclusive_unsupported_every_prefix_recovers
  core::coordination::tests::rename_probe_swap_unsupported_every_prefix_recovers
  core::coordination::tests::rename_probe_unsupported_cleanup_failure_preserves_evidence
  core::coordination::tests::rename_probe_recovery_every_prefix_is_idempotent
  core::lease::tests::lease_acquisition_recovers_owner_and_probe_prefixes
  layout::profile_tests::layout_profile_cleanup_every_prefix_recovers
  ```
- The only body invocation is reviewed here as:
  `cargo test --locked --offline -p surgeist-generator --all-features --lib -- --ignored --test-threads=1`.
  It is not a task command or an ordinary/holistic command. After all tasks,
  fixes, clean task reviews, final ordinary gates, a clean holistic review, and
  the post-holistic list-only inventory, the coordinator stops and blocks the
  goal before this command. It runs exactly once only after the user prepares
  the machine and explicitly authorizes it. Failure stops publication; there is
  no automatic rerun. Any later code/test/plan commit invalidates the evidence
  and requires a new clean review sequence plus fresh user authority before a
  rerun.
- Supported mutation/generation remains Apple-Silicon macOS. The default shared
  value/read library remains portable and warning-clean on native and
  `wasm32-unknown-unknown`; CSS-only builds do not resolve/link the browser graph.
  No executable unsafe is permitted; `#![forbid(unsafe_code)]` remains.

## Impacts

- Public API: additive generation capability under `layout-browser` only;
  default, CSS, and the C03 browser-free layout signatures remain unchanged.
- Dependencies/features/lockfile: add only the four exact optional direct
  dependencies and their offline-resolved graph; refresh `Cargo.lock`; add the
  exact license policy. No fetcher/downloader/TLS/archive feature is enabled.
- Runtime: layout generation adds one private worker thread, one multi-thread
  Tokio runtime, one handler task, one internal supervisor process and profile
  per version/batch/retry attempt, and bounded manifest-owned batches/timeouts.
  CSS remains synchronous/threadless.
- Generated artifacts/fixtures: no production corpus, helper, browser, generated
  XML, report, fixture, or cache is committed. All behavioral artifacts are
  temporary synthetic test roots.
- Documentation/examples: layout rustdoc and final README/AGENTS explain the
  exact two-driver boundary, explicit roots, trusted-browser boundary, fixed
  environment, profile recovery, offline attestation, Taffy adoption, target
  support, and final offline command inventory.
- Preservation/linkage: the reviewed mapping precedes deletion of
  `src/layout/legacy_generator.rs`; all artificial private-front linkage is gone.
- Root/API artifacts: none; the root facade owns later gitlink/API work.
- Unsafe: no executable `unsafe`; safe `rustix` APIs own process-group signaling.

## Tasks

### C04-T01 — Land the complete atomic layout generation vertical slice

- Files/area: exact feature/dependency/policy entries in `Cargo.toml`, refreshed
  `Cargo.lock`, new `deny.toml`; existing public layout front, CLI, manifest,
  case/check/report modules; new private generation, browser/config,
  supervisor/profile, measurement/HTML, rendering, selection, and publication
  modules; genuine shared-core integration; thin-binary/public/process tests;
  `src/layout/profile_tests.rs` and other focused test modules. The preservation
  file remains uninspected and unchanged.
- Intended behavior/outcome: expose the exact generation API/CLI only together
  with a working end-to-end domain path. Against explicit synthetic roots and
  injected owned adapters, validate the trusted browser boundary; execute the
  authenticated supervisor/profile lifecycle; derive, measure, render, account,
  and atomically publish clean/diagnostic/filtered generations; preserve all
  C03 offline/import/check behavior; and replace every retained artificial core
  reference that has a genuine generation/profile owner.
- RED evidence: first add compile/API tests for `Generate`, `generate`,
  `browser_path`, and `filter`; exact CLI option/error/exit and real synthetic
  generate-process tests; metadata/feature assertions; browser cache/executable,
  launch switch/config/environment/capsule invalid matrices; the 15 named
  ordinary profile lifecycle tests; the ignored profile-prefix diagnostic;
  measurement/HTML/four-variant/XML/report goldens; retry and ledger coverage;
  historical clean/diagnostic/filtered state sequences; and panic/error/
  terminalization tests. At the C03 base they fail because the public capability,
  dependency edge, supervisor/profile, measurement, serializers, and generation
  orchestrator do not exist. Capture focused failing predicates before production
  edits; dependency resolution itself uses only the offline cache.
- Acceptance criteria — package and public boundary:
  - exact SR-05.1 dependencies/features/versions and `deny.toml` are present;
    offline lock generation succeeds; metadata has only the existing two
    required-feature binaries; default/CSS do not activate browser dependencies;
    direct/transitive license, advisory, MSRV, and no-fetch policy gates pass;
  - `LayoutCommand`, `LayoutRequest`, accessors, `run`, `run_from_env`, CLI,
    binary prefix/exit, private supervisor entry, and request invalid matrix are
    exactly SR-05.2/SR-05.4; constructors are I/O-free; non-generation behavior
    is unchanged; another generation host is `Generation`; worker/runtime build
    failure occurs before resource acquisition; unexpected internal panic is
    terminalized and resumed, never mapped to an input error;
  - no public Tokio/Chromiumoxide/URL/descriptor/lease/transaction/domain type,
    extra trait, target, CLI mode, environment configuration surface, or managed
    acquisition path exists.
- Acceptance criteria — browser and profile boundary:
  - cache/corpus disjointness, exact owner-relative executable resolution,
    executable regular/single-link identity, raw digest snapshot, version output,
    switch normalization, exact builder/supervisor set, fixed cleared environment,
    and closing/post-terminal drift checks produce the exact owning error kinds;
  - canonical journal/capsule byte shapes, modes, digests, tokens, ordinals,
    identity records, transition lock, running publication, process-group
    registration, output caps, timeouts, and pre-spawn revalidation are exact;
    forged/operator capsule state starts nothing;
  - normal close, spawn/launch/handler/version failure, forced group kill,
    dependency panic, repository panic, parent crash, live/dead recovery,
    transition-lock race, cleanup failure, identity drift, opaque raw-name erasure,
    and next-acquisition behavior satisfy SR-03.3; recovery never signals; no
    artifact plan exists until every attempt is terminal;
  - the exact ordinary profile tests are
    `layout_profile_normal_close_is_terminal`,
    `layout_profile_launch_failure_is_terminal`,
    `layout_profile_forced_group_kill_is_terminal`,
    `layout_profile_parent_crash_live_group_blocks`,
    `layout_profile_parent_crash_dead_group_recovers`,
    `layout_profile_revalidation_failure_preserves_dead_journal`,
    `layout_profile_cleanup_begins_only_after_revalidation`,
    `layout_profile_identity_drift_after_classification_preserves_evidence`,
    `layout_profile_transition_lock_closes_launch_race`,
    `layout_profile_opaque_entries_never_escape`,
    `layout_profile_cleanup_failure_preserves_evidence`,
    `layout_dependency_panic_maps_to_process`,
    `layout_profile_panic_resumes_after_cleanup`, and
    `layout_profile_panic_retains_cleanup_evidence`; the fifteenth named profile
    test is the separately ignored exact-prefix diagnostic in the Boundary.
- Acceptance criteria — generation and publication:
  - manifest-owned helper/base-style/HTML/Taffy authority is held and revalidated;
    job derivation, exact filter matching/ownership, batching, per-attempt profile,
    retry class, HTML injection/base URL/polling/grid capture, four variant keys,
    measurement conversion, XML bytes/escaping/provenance, report bytes/digests/
    sorting/scopes/counts/dispositions, and browser-attestation consistency match
    SR-06.3 and existing offline validators;
  - CleanFull installs a complete current desired root and returns `Ok`;
    DiagnosticFull installs one complete diagnostic authority then returns
    `Generation`; incomplete diagnostics and filtered job/lifecycle failures
    publish nothing; Filtered updates only historically owned selected XML,
    removes selected now-unsupported XML, writes no report, and preserves every
    unselected byte; stale/unknown/historical-union and post-commit precedence are
    exact;
  - required cross-generation tests include
    `layout_historical_inventory_removal_rename_addition_regenerates`,
    `layout_membership_delta_diagnostic_replaces_authority`,
    `layout_filtered_add_then_remove_requires_full_before_creation`,
    `layout_filtered_digest_change_makes_preserved_report_stale`,
    `layout_filtered_unsupported_removes_owned_xml_and_stales_report`, every
    named layout filter test in SR-04.5, existing XML/report/diagnostic goldens,
    retry/four-variant/HTML adapter tests, and all three browser-free commands;
  - fake browser/supervisor process tests prove real group/journal behavior but
    launch only the crate-owned current test executable. No test attempts an
    installed Chromium, network, source import from a sibling, or production
    corpus read.
- Commands:
  - `cargo generate-lockfile --offline`
  - `RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --no-default-features`
  - `RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --features layout-browser`
  - `RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --features css-corpus`
  - `RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --all-features`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser --lib layout_profile_`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser --lib layout_browser_`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser --lib layout_generate_`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser --lib layout_filter_`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser --test public_api`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser --test layout_cli`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus`
  - `cargo test --locked --offline -p surgeist-generator --all-features`
  - `cargo test --locked --offline -p surgeist-generator --all-features -- --ignored --list`
  - `cargo metadata --locked --offline --no-deps --format-version 1`
  - `cargo deny --all-features --locked --offline check licenses`
  - `cargo audit --no-fetch --stale`
  - `cargo clippy --locked --offline -p surgeist-generator --all-features --all-targets -- -F unsafe-code -D warnings`
  - `cargo fmt --check`
  - `git diff --check`
- Dependencies: C03 is published/read back at the cycle base; all direct archives
  and required installed tools are present; no preservation inspection has begun.
- Intended commit: `feat(layout): generate atomically with chromium`.

### C04-T02 — Map and retire the preservation source and artificial linkage

- Files/area: the exact preservation file for one bounded read/mapping/deletion;
  compiled layout/core modules and focused tests for any discovered unmapped
  responsibility; `src/core/mod.rs` artificial link; negative documentation in
  `src/layout/mod.rs`; dead private helpers made obsolete by real generation.
- Intended behavior/outcome: prove every retained preservation responsibility is
  represented once by compiled code/focused tests or an explicit corpus-owned
  input; prove every intentionally rejected mechanism has the reviewed
  replacement; delete the preservation file; remove all artificial linkage and
  unnecessary private surface without changing the clean C04-T01 public/domain
  contract.
- RED evidence: before deletion, `test ! -e src/layout/legacy_generator.rs`
  fails and the exact C03 artificial-link inventory remains discoverable. The
  assigned worker first verifies the 4,626-line/digest identity, reads only that
  exact file, and builds a task-result responsibility table covering schema,
  browser launch/version, batching/retry, HTML/helper/base-style injection,
  DOM/measurement/four variants, XML escaping/provenance, report/disposition/
  scoped accounting, filtering, cleanup, and publication. Any unmapped retained
  responsibility receives a focused failing test before compiled correction.
- Acceptance criteria:
  - retained behavior maps to exact compiled files and named tests or to the
    manifest/corpus-owned helper inputs; managed browser/source acquisition,
    embedded production assets/constants, inherited/operator browser environment,
    process-global locks, direct writes, non-durable cleanup, and legacy report
    ownership are explicitly mapped to their reviewed replacements/rejections;
  - after mapping and any corrective tests pass,
    `src/layout/legacy_generator.rs` is deleted, not renamed/copied/generated/
    included; no compiled/test module contains a second implementation or a
    source-text copy; the task result retains the pre-deletion digest and map;
  - `private_front_doors_are_linked` and its call from identifier validation are
    gone. Each handed-off private item is reached by real production generation/
    profile behavior or removed if redundant. No test-only/no-op function
    reference, lint allowance, identity `map_err`, or dead-code suppression
    substitutes for real linkage;
  - every C03/C04 layout, CSS, default, API/CLI, profile, generation, and offline
    checking test remains clean; exact ignored inventory is 16 list-only; no
    ignored body runs.
- Commands:
  - `test ! -e src/layout/legacy_generator.rs`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus`
  - `cargo test --locked --offline -p surgeist-generator --all-features`
  - `cargo test --locked --offline -p surgeist-generator --all-features -- --ignored --list`
  - `RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --no-default-features`
  - `RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --features layout-browser`
  - `RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --features css-corpus`
  - `RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --all-features`
  - `cargo clippy --locked --offline -p surgeist-generator --all-features --all-targets -- -F unsafe-code -D warnings`
  - `cargo fmt --check`
  - `git diff --check`
- Dependencies: C04-T01 is task-clean; its complete generation behavior and
  focused evidence exist before the sole preservation read begins.
- Intended commit: `refactor(layout): retire preserved generator`.

### C04-T03 — Finalize guidance, policy evidence, and leaf handoff readiness

- Files/area: `README.md`, `AGENTS.md`, final rustdoc/examples and public/process
  tests, dependency/policy/metadata evidence, exact baseline-finding closure and
  terminal handoff inventory. No root/sibling artifact is generated.
- Intended behavior/outcome: describe the completed two-driver crate accurately;
  close stale-guidance/quality/tautological-test findings; prove every supported
  feature/target/policy combination offline; and leave a clean task-reviewed
  candidate ready for final holistic review and the separately authorized
  exhaustive gate.
- RED evidence: source/document assertions first identify the current stale
  scaffold/future-migration statements and incomplete command inventory. Add or
  strengthen rustdoc/doctest/process assertions for the trusted-browser,
  acquisition-free, offline-attestation, supervisor/orphan-recovery, Taffy
  adoption, and exact API/CLI boundaries before replacing guidance. Any policy or
  composed-matrix failure is corrected without weakening behavior or allowlists.
- Acceptance criteria:
  - README and AGENTS describe the small default shared core; exact feature and
    two-binary matrix; explicit corpus/source/browser roots; layout browser/HTML/
    XML and CSS CSSTree/neutral ownership; manifest-owned mutable pins/counts;
    acquisition-free resources; production crates' normal non-dependency;
    Apple-Silicon macOS mutation versus portable default value/read builds;
    browser trust and fixed environment; supervisor/profile recovery and operator
    action for a live orphan; offline browser attestation; one-time Taffy sidecar
    and four-tightening adoption checks; and exact offline verification commands;
  - no scaffold/future-migration claim, sibling-integration claim, managed-browser
    implication, containment overclaim, stale command, hidden environment option,
    or normal exhaustive-test instruction remains;
  - public rustdoc/examples expose only exact shared/CSS/layout contracts and
    state that only generation authenticates the current executable; real binary
    invalid syntax still prints one exact prefix, no stdout, and exits 64;
  - exact dependency graph, feature isolation, metadata, offline lock
    reproducibility, license allowlist, advisory result/database staleness, MSRV,
    native/wasm checks, unsafe absence, preservation deletion, and 16-name
    list-only inventory are recorded in task evidence; every baseline finding is
    mapped closed; no extra planning/evidence document is persisted.
- Commands:
  - `command -v cargo-deny`
  - `command -v cargo-audit`
  - `cargo generate-lockfile --offline`
  - `RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --no-default-features`
  - `RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --features layout-browser`
  - `RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --features css-corpus`
  - `RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --all-features`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus`
  - `cargo test --locked --offline -p surgeist-generator --all-features`
  - `cargo test --locked --offline -p surgeist-generator --all-features -- --ignored --list`
  - `cargo clippy --locked --offline -p surgeist-generator --no-default-features --all-targets -- -F unsafe-code -D warnings`
  - `cargo clippy --locked --offline -p surgeist-generator --features layout-browser --all-targets -- -F unsafe-code -D warnings`
  - `cargo clippy --locked --offline -p surgeist-generator --features css-corpus --all-targets -- -F unsafe-code -D warnings`
  - `cargo clippy --locked --offline -p surgeist-generator --all-features --all-targets -- -F unsafe-code -D warnings`
  - `RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --target wasm32-unknown-unknown --no-default-features --lib`
  - `cargo metadata --locked --offline --no-deps --format-version 1`
  - `cargo deny --all-features --locked --offline check licenses`
  - `cargo audit --no-fetch --stale`
  - `cargo fmt --check`
  - `git diff --check`
  - `test ! -e src/layout/legacy_generator.rs`
- Dependencies: C04-T02 is task-clean and the responsibility-map/deletion/
  artificial-linkage evidence is available.
- Intended commit: `docs: finalize generator guidance and policy`.

## Completion

- Observable acceptance: both thin binaries and feature-gated public modules are
  complete; layout executes four real commands and CSS three against explicit
  roots; generation uses only the explicit trusted executable and exact fixed
  profile/supervisor boundary; synthetic adapters prove measurement and durable
  terminalization without Chromium; clean/diagnostic/filtered XML/report
  publication is deterministic and recoverable; browser-free checks remain
  offline; dependency/policy/docs/target matrices are clean; preservation and
  artificial linkage are gone; no production artifact or sibling state changed.
- After C04-T01 through T03 are individually task-clean, the coordinator changes
  this plan to `complete`, commits only that status transition, and runs the
  ordinary final command list below. A fresh holistic reviewer receives the
  exact cycle range from `65ae2af6e3b1a2e9640decfa186dc2bf37ae4f7a` through
  that status commit, every ordered task/fix range, and the full responsibility,
  dependency, policy, and ignored-inventory evidence. Every holistic finding
  enters the normal fresh-worker/fresh-reviewer loop; the plan is reopened while
  fixes are active. The ignored body remains prohibited throughout that loop.
- Final ordinary and post-holistic command list:
  - `command -v cargo-deny`
  - `command -v cargo-audit`
  - `cargo generate-lockfile --offline`
  - `RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --no-default-features`
  - `RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --features layout-browser`
  - `RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --features css-corpus`
  - `RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --all-features`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus`
  - `cargo test --locked --offline -p surgeist-generator --all-features`
  - `cargo test --locked --offline -p surgeist-generator --all-features -- --ignored --list`
  - `cargo clippy --locked --offline -p surgeist-generator --no-default-features --all-targets -- -F unsafe-code -D warnings`
  - `cargo clippy --locked --offline -p surgeist-generator --features layout-browser --all-targets -- -F unsafe-code -D warnings`
  - `cargo clippy --locked --offline -p surgeist-generator --features css-corpus --all-targets -- -F unsafe-code -D warnings`
  - `cargo clippy --locked --offline -p surgeist-generator --all-features --all-targets -- -F unsafe-code -D warnings`
  - `RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --target wasm32-unknown-unknown --no-default-features --lib`
  - `cargo metadata --locked --offline --no-deps --format-version 1`
  - `cargo deny --all-features --locked --offline check licenses`
  - `cargo audit --no-fetch --stale`
  - `cargo fmt --check`
  - `git diff --check`
  - `test ! -e src/layout/legacy_generator.rs`
  - verify `cargo generate-lockfile --offline` left a clean worktree and the
    committed lock graph has exactly the reviewed direct dependencies/features;
  - run the failure-propagating owned-Rust manifest/unsafe scan used by C03,
    now requiring no preservation path and no executable unsafe match.
- After the clean holistic verdict and the repeated post-holistic ordinary list,
  the coordinator verifies the worktree and `main` are unchanged, re-runs only
  the all-target list command, compares the exact 16 names, and blocks the active
  goal. It does not start the expensive command while waiting.
- On explicit user authorization, execute exactly once and sequentially:
  ```sh
  cargo test --locked --offline -p surgeist-generator --all-features --lib -- --ignored --test-threads=1
  ```
  A pass is terminal exhaustive evidence. A failure stops publication and is not
  rerun. No source, test, plan, lockfile, or documentation commit may occur after
  a pass; otherwise fresh review and fresh user authority are required.
- Publication after that pass is lease-protected: capture clean candidate SHA and
  expected authority-remote `main`; independently read the remote; push only with
  an explicit `refs/heads/main:<expected>` lease; independently read back exact
  candidate equality; and require local `HEAD`, tracking `origin/main`, and
  authority-remote `main` to match with a clean worktree.
- Required handoff: immutable published C04 SHA; canonical baseline review,
  reviewed spec/sequence/C01-C04 plan revisions; every ordered task/fix range and
  clean task/holistic verdict; exact two-driver feature/API/CLI contract;
  dependency/license/advisory/MSRV/target evidence; trusted-browser and profile
  boundary; generation/publication/offline-check evidence; preservation digest/
  responsibility/deletion map; artificial-link closure; exact 16-name list and
  one authorized sequential pass; unsafe absence; clean remote readback; and an
  explicit statement that root gitlink/API and sibling adoption remain future
  owning-repository work.
- Unresolved blocker: none at plan authoring. Any offline cache miss, missing
  installed policy tool, unapproved license/advisory/MSRV conflict, need to launch
  or acquire Chromium, inability to terminalize/recover profiles safely,
  unclassifiable preservation responsibility, dependency panic outside the
  narrow boundary, unsupported declared host behavior, or invariant conflict
  stops implementation for plan correction. User authorization remains a
  deliberate required blocker immediately before the one exhaustive body.
