# C01 Rooted Core And Production Recovery

## Header

- Cycle ID: `C01`
- Owning repository: `/Users/codex/Development/surgeist-generator`
- Status: `in_progress`
- Cycle base: `cb341baf0f1f18877bedbc960878b1b7a37d9acb`
- Immutable implementation-series source base:
  `1e545530fb992e2e749f26a1ba7ac0d77ed85dd6`
- Reviewed specification:
  `plans/specs/2026-07-17-surgeist-generator-review-remediation.md`, sections
  SR-02 rows **Rooted lease-tree rejection**, **Model-only crash/bootstrap
  tests**, and the pre-existing portion of **Quality matrix**; SR-04.1 through
  SR-04.4; the transaction, lease, bootstrap, owner-record, rename-probe, and
  shared error clauses of SR-04.5; and the current-core clauses of SR-04.6; at
  commit `0c56de73f0280993761314f514626951de56cfb7`, normalized semantic-content
  SHA-256
  `9a1a148c198d39145017db769b9ab91025f1cc37719efc6d3c68ab998da7c524`,
  review `CLEAN`.
- Reviewed implementation sequence:
  `plans/sequences/2026-07-17-surgeist-generator-review-remediation.md`, entry
  C01, at commit `c7d3f010faf5769960df7c42c937924ebb4d2a39`, normalized
  semantic-content SHA-256
  `7bb79d1c0f9bd7c78593bff7ac9dbf60b3b5372f7852486b678e9ed4e31316c2`,
  review `CLEAN`.
- Bounded outcome: correct rooted absence handling, replace model-only
  transaction/bootstrap assurances with exhaustive failure injection through the
  real production recovery paths, prove owner-record and rename-probe recovery,
  and leave the shared-core baseline warning-, Clippy-, format-, and test-clean.

## Boundary

- Mutate, commit, and publish only this repository. Root `surgeist`, sibling
  crates, their corpora, and their artifacts remain outside read, mutation, and
  test scope.
- This cycle does not add either domain module or binary, change public API,
  alter dependencies/features/MSRV, update README or AGENTS, or remove the
  preservation copy. Those outcomes remain allocated to later cycles.
- `src/layout/legacy_generator.rs` remains byte-identical at 4,626 lines and
  SHA-256
  `d2f5ca87cea6b36826e9172e2d7ba7a99196c375e2ca53f8a84a075200e70a9f`.
  It is not formatted, compiled, or tested.
- No network is permitted during planning, implementation, or verification; no
  dependency/toolchain/target acquisition, browser/source process, corpus
  generation, or system-wide mutation is permitted. Canonical Git query, fetch,
  push, and readback against this repository's authority remote are reserved only
  for landing publication. All Cargo work uses installed Rust 1.97, installed
  targets, and already-present caches with `--locked --offline`.
- Owned Rust remains free of executable `unsafe`; strict exact-name, no-follow,
  same-mount, identity, type, ownership, and mode checks may not be weakened to
  make absence or recovery tests pass.
- The cycle base remains planning-only. C01-T01 and C01-T02 are task-clean at
  `20c55499a3e0485f1f8dabbd93b0459e05250720` and
  `135e1a6afabe09f739e16e1bde8395fd46ef7d4c`. C01-T03 has an unreviewed initial
  worker span `135e1a6afabe09f739e16e1bde8395fd46ef7d4c..3080113df920862302b397d7cfb84e28c72578c0`;
  after this plan is clean, a fresh worker must reconcile its ordinary exhaustive
  tests, append a new T03 correction span, and rerun every T03 acceptance command.
  A fresh task reviewer then reviews the complete ordered initial-plus-correction
  T03 range. No implementation test ran while this plan correction was authored.
- The supported mutation host is `Darwin arm64`; portable default-library
  verification is warning-denied `wasm32-unknown-unknown`. Unsupported targets
  retain their semantic stop before mutation.
- Production code has no observer, interruption return, global failure hook, or
  test sentinel. The exhaustive harness is instance-scoped and `#[cfg(test)]`;
  a test interruption unwinds only with a private sentinel so production error
  recovery cannot consume it as an ordinary failure.
- Every test that enumerates each real event, byte, or recovery prefix is marked
  `#[ignore = "exhaustive opt-in diagnostic"]`. Ordinary Cargo test matrices
  compile but never execute these diagnostics. C01 task and final gates compare
  libtest's ignored listing to the exact inventory below and execute none. The
  single sequential execution is owned by the initiative-final C04 gate.
- The 15 fully qualified names below are the exact C01
  ignored inventory. After T03, T04, T05, and T06, `--ignored --list` must equal
  respectively the applicable cumulative 2, 6, 9, and 15 names with no extra or
  missing test:
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
  ```
- The artificial `private_front_doors_are_linked` references remain until real
  domain callers replace them in later cycles. C01 may not delete them merely to
  satisfy dead-code checks.
- Local policy names no authoritative generated-Rust root in this leaf. The
  owned-Rust manifest therefore consists of every tracked and non-ignored
  untracked `*.rs` path, including the preservation copy, while dependency/vendor
  and build-cache roots are excluded.

## Impacts

- Public API: unchanged.
- Dependencies/features/lockfile: unchanged.
- Generated artifacts and fixtures: none committed; recovery fixtures exist only
  in test-owned temporary directories.
- Documentation/examples: unchanged in this cycle.
- MSRV and target policy: Rust 1.97 unchanged; mutation remains Apple-Silicon
  macOS only and default value/read code remains portable.
- Root/API-artifact follow-up: none in this cycle.
- Unsafe: no executable `unsafe` in owned Rust; `#![forbid(unsafe_code)]` remains.

## Tasks

### C01-T01 — Restore rooted acquisition and the reviewable core baseline

- Files/area: `src/core/fs.rs` and its focused `#[cfg(test)]` module; the exact
  shared-core files named by the baseline identity-map, target-warning, and
  rustfmt evidence for mechanical cleanup after the rooted regression is green.
- Intended behavior/outcome: give `RootedFs::exists` its SR-04.1
  existence-aware component traversal so a missing intermediate or leaf is
  `Ok(false)` without creating anything, while every present component retains
  exact-name, no-follow, mount, directory, identity, ownership, and mode checks.
  Generic mutation traversal and `open_parent` remain strict. After that
  behavior is green, remove the identity `map_err`, narrow target-specific
  imports/helpers, and apply rustfmt without changing semantics or artificial
  linkage so every later Rust task starts from a reviewable quality baseline.
- RED evidence: first add
  `rooted_exists_missing_intermediate_is_false`,
  `rooted_exists_missing_leaf_is_false`, and strict existing-component
  regression cases. The missing-intermediate case must reproduce the current
  `InvalidPath` before the implementation changes; fixture snapshots must prove
  both absence queries are non-mutating.
- Acceptance criteria: both absence depths return false; alias, symlink,
  non-directory, mount/policy, non-UTF-8, and permission/inconclusive cases retain
  errors; every existing lease/artifact test reaches its intended assertions and
  passes on the supported host; default tests and warning-denied Clippy, format,
  native check, and portable check are clean without lint suppression or
  behavior/test deletion.
- Commands:
  - `cargo test --locked --offline -p surgeist-generator --lib rooted_exists_missing`
  - `cargo test --locked --offline -p surgeist-generator --lib rooted_exists_preserves_strict`
  - `cargo test --locked --offline -p surgeist-generator --lib core::lease::tests`
  - `cargo test --locked --offline -p surgeist-generator --lib core::artifact::tests`
  - `RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --no-default-features`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features`
  - `cargo clippy --locked --offline -p surgeist-generator --no-default-features --all-targets -- -F unsafe-code -D warnings`
  - `RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --target wasm32-unknown-unknown --no-default-features --lib`
  - `cargo fmt --check`
- Dependencies: reviewed C01 plan only.
- Intended commit: `fix(core): restore rooted acquisition baseline`.

### C01-T02 — Instrument real durability primitives for exhaustive interruption

- Files/area: test-only support in `src/core/fs.rs`, plus the narrow observer
  attachment points required in `src/core/transaction.rs` and
  `src/core/coordination.rs`.
- Intended behavior/outcome: implement the instance-scoped SR-04.2 observer and
  private interruption sentinel. An unhooked production path records stable
  phase, primitive, strict relative path, and per-phase ordinal events after
  every recovery-distinct mutation/durability boundary; an armed run interrupts
  immediately after one selected event, unwinds past same-process recovery,
  drops held handles, and permits a fresh `RootedFs` to observe durable state.
- RED evidence: first add
  `rooted_observer_records_recovery_distinct_primitives` and
  `rooted_observer_interrupts_without_generator_error`. They must fail because
  the current production primitives expose neither a trace nor an interruption
  boundary, rather than because of fixture setup.
- Acceptance criteria: the observer covers the complete SR-04.2 primitive list,
  including partial/full writes, syncs, publications, renames, individual
  removals, and drop boundaries; ordinals and paths are deterministic; separate
  observed roots cannot interfere; ordinary production construction has no hook
  and no altered error behavior; no test relies on a process-global mutable
  fault switch.
- Commands:
  - `cargo test --locked --offline -p surgeist-generator --lib rooted_observer_records`
  - `cargo test --locked --offline -p surgeist-generator --lib rooted_observer_interrupts`
  - `cargo test --locked --offline -p surgeist-generator --lib core::fs::tests`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features`
  - `cargo clippy --locked --offline -p surgeist-generator --no-default-features --all-targets -- -F unsafe-code -D warnings`
  - `cargo fmt --check`
- Dependencies: C01-T01 is task-clean.
- Intended commit: `test(core): instrument durable mutation prefixes`.

### C01-T03 — Prove transaction install and recovery prefixes

- Files/area: `src/core/transaction.rs`, shared observed-root test support from
  C01-T02, and transaction-facing assertions in `src/core/artifact.rs` only when
  needed for the production entry path.
- Intended behavior/outcome: replace the hard-coded crash model as normative
  proof with SR-04.2/SR-04.3 execution of `TransactionEngine::install` and
  `recover_all` over a nested old/new fixture. Trace and interrupt exclusive and
  swap installation at every real event, then trace and interrupt recovery for
  the four pre/post-commit seeds; use fresh rooted authorities for recovery and
  assert actual bytes, complete owned residue, returned kind, and idempotence.
- RED evidence: first add
  `transaction_install_every_prefix_recovers`,
  `transaction_recovery_every_prefix_is_idempotent`,
  `transaction_corruption_preserves_evidence`, and
  `transaction_post_commit_failure_keeps_new_generation`. They must demonstrate
  that the current model never executes or inspects production durable state.
- Acceptance criteria: exclusive pre-commit visibility is absent, swap
  pre-commit visibility is byte-identical old, and both post-commit states are
  complete new; every valid prefix is accepted by fresh production recovery,
  leaves no owned residue, and is stable on repetition; corruption/unknown/
  identity-replacement evidence is preserved as `ArtifactTransaction`; a
  post-commit operational or cleanup failure cannot restore old output. The old
  protocol table may remain only as supplementary ordering documentation. Both
  exhaustive prefix enumerators are ignored diagnostics; corruption and
  post-commit tests remain ordinary. The accepted pre-deferral evidence covers
  10,547 install and 5,193 recovery prefixes; C01 does not rerun either ignored
  diagnostic after adding its classification attribute.
- Commands:
  - `cargo test --locked --offline -p surgeist-generator --lib transaction_corruption_preserves_evidence`
  - `cargo test --locked --offline -p surgeist-generator --lib transaction_post_commit_failure`
  - `cargo test --locked --offline -p surgeist-generator --lib core::artifact::tests`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features`
  - `cargo test --locked --offline -p surgeist-generator --lib -- --ignored --list`
  - `cargo clippy --locked --offline -p surgeist-generator --no-default-features --all-targets -- -F unsafe-code -D warnings`
  - `cargo fmt --check`
- Dependencies: C01-T02 is task-clean.
- Intended commit: `test(core): prove transaction recovery prefixes`.

### C01-T04 — Prove bootstrap success, contention, and recovery prefixes

- Files/area: bootstrap production and focused tests in
  `src/core/coordination.rs`, using the C01-T02 observer.
- Intended behavior/outcome: drive real `open_or_bootstrap_lock`, state
  validation, `recover_bootstrap`, header writing, and receipt cleanup through
  every SR-04.4 primitive/header prefix for uncontended publication, a held
  winner, and a released/adopted winner. A test-only instance liveness callback
  classifies only the synthetic abandoned owner; production keeps the process
  probe.
- RED evidence: first add
  `bootstrap_header_every_byte_prefix_recovers`,
  `bootstrap_uncontended_every_prefix_recovers`,
  `bootstrap_winner_held_every_prefix_recovers`, and
  `bootstrap_winner_released_every_prefix_recovers`. They must fail because the
  current test checks only a hard-coded step array and cannot compile the new
  production-path diagnostic support. The pre-support `--no-run` build is RED;
  ignored diagnostic bodies are not executed in C01.
- Acceptance criteria: incomplete local headers never publish; pre-rename
  prefixes recover to absence and post-rename prefixes to one exact immutable
  lock; held-winner loss returns `LeaseActive` and cleans only the loser while
  preserving the winner; released-winner adoption returns its held handle;
  corruption/live-owner evidence remains; every recoverable prefix completes and
  is idempotent without replacing the winner. All four exhaustive bodies compile
  and occupy exactly their cumulative ignored-inventory positions; execution is
  deferred to C04.
- Commands:
  - `cargo test --locked --offline -p surgeist-generator --no-default-features --no-run`
  - `cargo test --locked --offline -p surgeist-generator --lib core::lease::tests`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features`
  - `cargo test --locked --offline -p surgeist-generator --lib -- --ignored --list`
  - `cargo clippy --locked --offline -p surgeist-generator --no-default-features --all-targets -- -F unsafe-code -D warnings`
  - `cargo fmt --check`
- Dependencies: C01-T03 is task-clean.
- Intended commit: `test(core): prove bootstrap recovery prefixes`.

### C01-T05 — Prove owner-record visibility and acquisition ordering

- Files/area: owner-record install/recovery and acquisition orchestration in
  `src/core/coordination.rs`, with only necessary lease-facing assertions in
  `src/core/lease.rs`.
- Intended behavior/outcome: apply the normative observer to owner install with
  absent and old records and to recovery of all four pre/post-commit seeds. Prove
  the SR-04.4 absent/old/new visibility oracle and the acquisition order through
  protected revalidation before owner installation, with no guard returned and
  the mutex released on failure.
- RED evidence: first add the specified
  `owner_record_install_every_prefix_recovers_absent`,
  `owner_record_install_every_prefix_recovers_swap`,
  `owner_record_recovery_every_prefix_is_idempotent`,
  `owner_record_corruption_preserves_evidence`,
  `lease_revalidation_failure_preserves_historical_owner`, and
  `lease_owner_install_begins_only_after_revalidation` tests. They must fail on
  missing production-path observation/recovery support at the `--no-run` build,
  not on the already-corrected rooted traversal. Ignored diagnostic bodies are
  not executed in C01.
- Acceptance criteria: before commit the owner is absent or exact old and at/
  after commit it is exact new; fresh recovery of every install/recovery prefix
  removes valid residue and is idempotent; malformed/digest/identity/unknown
  states preserve evidence as `ArtifactTransaction`; a preceding recovery,
  probe, or revalidation failure creates no owner transaction, preserves the
  historical owner, releases the mutex, and returns no guard. The three
  exhaustive bodies compile and extend the ignored inventory from six to nine;
  execution is deferred to C04.
- Commands:
  - `cargo test --locked --offline -p surgeist-generator --no-default-features --no-run`
  - `cargo test --locked --offline -p surgeist-generator --lib owner_record_corruption_preserves_evidence`
  - `cargo test --locked --offline -p surgeist-generator --lib lease_revalidation_failure_preserves_historical_owner`
  - `cargo test --locked --offline -p surgeist-generator --lib lease_owner_install_begins_only_after_revalidation`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features`
  - `cargo test --locked --offline -p surgeist-generator --lib -- --ignored --list`
  - `cargo clippy --locked --offline -p surgeist-generator --no-default-features --all-targets -- -F unsafe-code -D warnings`
  - `cargo fmt --check`
- Dependencies: C01-T04 is task-clean.
- Intended commit: `test(core): prove owner record recovery prefixes`.

### C01-T06 — Prove rename-probe recovery and capability faults

- Files/area: rename-probe install/recovery and exclusive-acquisition integration
  in `src/core/coordination.rs`, with necessary focused assertions in
  `src/core/lease.rs`.
- Intended behavior/outcome: trace and interrupt real probe installation and
  recovery at every SR-04.4 primitive; inject deterministic pre-mutation
  exclusive-rename and swap-rename capability faults; prove complete-cleanup
  `UnsupportedPlatform`, cleanup-failure `ArtifactTransaction`, preserved
  historical owner/domain bytes, exact resumable evidence, prerequisite recovery,
  re-probe behavior, and mutex release.
- RED evidence: first add all specified `rename_probe_*` production-path tests
  plus `lease_acquisition_recovers_owner_and_probe_prefixes`. The unsupported
  cleanup test must first expose at the `--no-run` build the current missing
  support for interrupting/recovering each real cleanup primitive. Ignored
  diagnostic bodies are not executed in C01.
- Acceptance criteria: every install/recovery prefix is accepted by fresh
  production recovery and finishes idempotently; no probe prefix changes a
  domain artifact or owner record; complete failure cleanup leaves no residue and
  returns the original capability kind; failed cleanup retains the exact valid
  journal and both capability/cleanup context; unknown/type/mode/identity/alias/
  mount states preserve evidence; the next acquisition recovers before re-probe
  and owner installation. The six exhaustive bodies compile and complete the
  exact 15-name ignored inventory; execution is deferred to C04.
- Commands:
  - `cargo test --locked --offline -p surgeist-generator --no-default-features --no-run`
  - `cargo test --locked --offline -p surgeist-generator --lib rename_probe_corruption_preserves_evidence`
  - `cargo test --locked --offline -p surgeist-generator --lib core::lease::tests`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features`
  - `cargo test --locked --offline -p surgeist-generator --lib -- --ignored --list`
  - `cargo clippy --locked --offline -p surgeist-generator --no-default-features --all-targets -- -F unsafe-code -D warnings`
  - `cargo fmt --check`
- Dependencies: C01-T05 is task-clean.
- Intended commit: `test(core): prove rename probe recovery prefixes`.

## Completion

- Observable acceptance: SR-04.1 absence semantics are strict and non-mutating;
  transaction/bootstrap/owner/probe production-path diagnostic bodies encode the
  required visibility, residue, error, recovery, and idempotence oracles;
  the ten previously blocked lease/artifact tests reach their intended behavior;
  corrupt or unclassifiable evidence is retained; post-commit state is never
  rolled back; ordinary matrices are clean; the exact 15 diagnostics compile and
  are reported ignored without execution; accepted historical T03 evidence is
  recorded; and the C01 quality matrix is clean without public/domain changes.
- Final command list:
  - `RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --no-default-features`
  - `RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --features layout-browser`
  - `RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --features css-corpus`
  - `RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --all-features`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus`
  - `cargo test --locked --offline -p surgeist-generator --all-features`
  - `cargo test --locked --offline -p surgeist-generator --all-features --lib -- --ignored --list`
  - `cargo clippy --locked --offline -p surgeist-generator --no-default-features --all-targets -- -F unsafe-code -D warnings`
  - `cargo clippy --locked --offline -p surgeist-generator --features layout-browser --all-targets -- -F unsafe-code -D warnings`
  - `cargo clippy --locked --offline -p surgeist-generator --features css-corpus --all-targets -- -F unsafe-code -D warnings`
  - `cargo clippy --locked --offline -p surgeist-generator --all-features --all-targets -- -F unsafe-code -D warnings`
  - `RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --target wasm32-unknown-unknown --no-default-features --lib`
  - `cargo fmt --check`
  - `git diff --check`
  - `shasum -a 256 src/layout/legacy_generator.rs`
  - Run this failure-propagating owned-Rust manifest and unsafe scan:
    ```zsh
    (
      set -eu
      manifest="$(mktemp)"
      trap 'rm "$manifest"' EXIT
      git ls-files -co --exclude-standard -z -- '*.rs' ':(exclude)vendor/**' ':(exclude)target/**' >"$manifest"
      LC_ALL=C sort -zu "$manifest" -o "$manifest"
      test -s "$manifest"
      tr '\0' '\n' <"$manifest"
      pattern='#\s*\[\s*(?:unsafe\s*\(|no_mangle\b|export_name\b)|\bunsafe\s*(?:\{|fn\b|trait\b|impl\b|extern\b)|\bstatic\s+mut\b|\bextern\s*(?:"[^"]*")?\s*\{'
      while IFS= read -r -d '' rust_path; do
        if rg -n --pcre2 "$pattern" -- "$rust_path"; then
          exit 1
        else
          scan_exit=$?
          test "$scan_exit" -eq 1 || exit "$scan_exit"
        fi
      done <"$manifest"
    )
    ```
- Required handoff: immutable published C01 SHA; exact ordered task ranges and
  clean reviews; accepted T03 counts; exact deferred 15-name inventory; proof no
  ignored diagnostic executed in C01; final command evidence; preservation
  digest; clean worktree; authority-remote main readback; and an explicit
  statement that C02 alone may begin next and C04 alone owns runtime execution.
- Unresolved blocker: none. Any missing installed cache/tool, unsupported
  platform behavior on the declared supported host, inability to make a real
  primitive observable without changing production semantics, or contradiction
  with the reviewed specification stops implementation for plan correction.
