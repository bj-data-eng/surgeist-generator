# C02 Protected Source And CSS Vertical Slice

## Header

- Cycle ID: `C02`
- Owning repository: `/Users/codex/Development/surgeist-generator`
- Status: `complete`
- Cycle base: `efcb868905025270c875fc683d82cfba3c029080`
- Immutable implementation-series source base:
  `1e545530fb992e2e749f26a1ba7ac0d77ed85dd6`
- Published prerequisite: C01 candidate
  `efcb868905025270c875fc683d82cfba3c029080`; local `main`, `origin/main`, and
  authority-remote `main` were equal at the C01 readback.
- Reviewed specification:
  `plans/specs/surgeist-generator-review-remediation.md`, CSS and
  shared clauses of SR-01 and SR-02; SR-03.1 and SR-03.2 shared/CSS clauses;
  SR-04.5 CSS source, historical-inventory, selection, publication, and error
  clauses; SR-04.6 CSS linkage; the CSS edge of SR-05.1, SR-05.3, and CSS
  clauses of SR-05.4; SR-07.1 through SR-07.4; CSS clauses of SR-08.1 and the
  affected SR-08.3 matrix; at commit
  `0c56de73f0280993761314f514626951de56cfb7`, normalized semantic-content
  SHA-256
  `9a1a148c198d39145017db769b9ab91025f1cc37719efc6d3c68ab998da7c524`,
  review `CLEAN`.
- Reviewed implementation sequence:
  `plans/sequences/surgeist-generator-review-remediation.md`, entry
  C02, at commit `c7d3f010faf5769960df7c42c937924ebb4d2a39`, normalized
  semantic-content SHA-256
  `7bb79d1c0f9bd7c78593bff7ac9dbf60b3b5372f7852486b678e9ed4e31316c2`,
  review `CLEAN`.
- Bounded outcome: finish the protected-source/disjointness boundary and ship
  one complete `css-corpus` vertical slice with deterministic CSSTree import,
  neutral expectation generation, checking, filtering, historical ownership,
  atomic stale removal, and a thin real CSS binary.

## Boundary

- Mutate, commit, and publish only this repository. Root `surgeist`, sibling
  crates, their corpora, gitlinks, and artifacts remain outside read, mutation,
  and test scope.
- C01 transaction, coordination, recovery, lease, and publication primitives
  remain the only mutation foundation. This cycle may extend their private
  protected-revalidation seam but may not weaken their policy, visibility,
  recovery, capability, or corruption guarantees.
- No layout module, layout binary, browser backend, heavy dependency, profile,
  HTML/XML behavior, root integration, README/AGENTS update, or preservation
  retirement belongs to C02. `src/layout/legacy_generator.rs` remains exactly
  4,626 lines with SHA-256
  `d2f5ca87cea6b36826e9172e2d7ba7a99196c375e2ca53f8a84a075200e70a9f`
  and is not formatted, compiled, or tested.
- `css-corpus = []` remains dependency-free. C02 adds only the feature-gated
  CSS module and `surgeist-css-generate` target with
  `required-features = ["css-corpus"]`; dependencies, `Cargo.lock`, package
  version, edition, Rust 1.97, license, and default feature remain unchanged.
- Verification uses only synthetic explicit owner/corpus/source roots and
  already-installed tooling. No network, Git clone/fetch, external corpus,
  browser/source executable, dependency/toolchain/target acquisition, corpus
  generation, or system-wide mutation is permitted. Local test-owned Git
  repositories may be inspected through the existing sanitized read-only
  source runner.
- Source verification remains publicly compatible. A crate-private protection
  snapshot holds canonical worktree, per-worktree Git directory, common Git
  directory, primary object directory, and recursive local alternate identities;
  command preflight and closing revalidation consume it without widening
  `VerifiedSource`.
- Every CSS mutation computes its complete writable/protected namespace matrix
  before lease acquisition, rejects lexical/canonical/descriptor ancestry in
  either direction, revalidates protection under the held mutex before intent,
  and publishes only through the C01 transaction boundary. Checks are read-only
  `GenerationCheck` operations and never repair coordination.
- CSS manifests, sidecars, expectations, and reports use the exact canonical
  schemas, field order, final LF, path grammar, disposition rules, counts,
  provenance, historical authority, and error precedence in SR-07 and SR-04.5.
  No visible entry is classified from extension or the new desired set alone.
- The first compiled CSS task wires a real feature-gated public module and binary
  to one complete import command. Generate, filtered generation, and checking are
  added only as complete reachable increments in later tasks; no unreferenced
  private staging, placeholder command arm, artificial caller, or dead-code
  suppression is permitted. Unit tests remain inside the private CSS submodules;
  public/process tests use only the public front and packaged binary.
- The exact cumulative ignored inventory is the following 15 fully qualified
  names. C02 adds no ignored test. After every task and at the final gate, the
  list-only command output must equal this set with no extra, missing, or renamed
  entry:
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
  Ordinary matrices compile and skip them. No command may contain `--ignored`
  without `--list`; only C04, after every initiative task, executes the one
  sequential all-features ignored run.
- `private_front_doors_are_linked` may lose only references replaced by real CSS
  command paths. C02 retains the function and explicitly inventories every
  layout-only reference still required for warning-clean builds, including
  `PublicationPolicy::DiagnosticFull`, for C04 final-linkage closure; no CSS-only
  fake use, placeholder, or lint suppression may stand in for a layout caller.
- Supported mutation remains Darwin arm64. The default library remains portable
  under warning-denied `wasm32-unknown-unknown`.

## Impacts

- Public API: additive only under `css-corpus`; exact `css` module from SR-05.3.
- Dependencies/features/lockfile: no dependency or lockfile change; existing
  `css-corpus` gains real code and one required-feature binary target.
- Generated artifacts/fixtures: no committed outputs; all corpus/source and
  generated expectation fixtures are test-owned temporary trees.
- Documentation/examples: public CSS rustdoc and acquisition-free examples are
  added; repository guidance remains allocated to C04.
- MSRV/target: Rust 1.97 unchanged; mutation remains Apple-Silicon macOS only.
- Root/API artifacts: none; root composition and audit artifacts remain excluded.
- Unsafe: no executable `unsafe`; `#![forbid(unsafe_code)]` remains.

## Tasks

### C02-T01 — Bind protected sources and namespace disjointness

- Files/area: `src/core/source.rs`, `src/core/corpus.rs`, `src/core/fs.rs`,
  `src/core/lease.rs`, `src/core/mod.rs`, and one focused private protection
  module when needed.
- Intended behavior/outcome: keep the public source proof unchanged while a
  private verified-source result retains every canonical Git/object authority
  and held identity. Build the three-layer writable/protected disjointness proof,
  absent-suffix representation, and held-mutex closing callback used by CSS.
- RED evidence: first add
  `protected_source_snapshot_covers_git_and_recursive_alternates`,
  `namespace_disjointness_rejects_alias_ancestor_and_missing_suffix`, and
  `protected_source_closing_revalidation_rejects_identity_change`. They fail
  because current verification discards the private authorities and has no
  bidirectional descriptor-ancestry or closing-revalidation contract.
- Acceptance criteria: preflight performs no write; equality, both ancestor
  directions, case/symlink/mount aliases, object-store overlap, and unprovable
  absent suffixes are `InvalidPath`; disjoint roots pass; replacement after
  preflight fails before intent; public `VerifiedSource` bytes/API are unchanged.
- Commands:
  - `cargo test --locked --offline -p surgeist-generator --lib protected_source_`
  - `cargo test --locked --offline -p surgeist-generator --lib namespace_disjointness_`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus`
  - `cargo test --locked --offline -p surgeist-generator --all-features --lib -- --ignored --list`
  - `cargo clippy --locked --offline -p surgeist-generator --features css-corpus --all-targets -- -F unsafe-code -D warnings`
  - `cargo fmt --check`
- Dependencies: published C01 candidate and reviewed C02 plan only.
- Intended commit: `feat(core): bind protected source authorities`.

### C02-T02 — Expose and execute atomic CSSTree import

- Files/area: `Cargo.toml`, `src/lib.rs`, the real feature-gated public `css`
  front and private submodules, `src/bin/surgeist-css-generate.rs`, public/process
  tests, and protected source, lease, inventory, and artifact integration.
- Intended behavior/outcome: expose a non-exhaustive `CssCommand` containing
  `ImportCsstree`, the final-signature `CssRequest::new` and accessors, and real
  `run`/`run_from_env`/binary paths for import. Validate the exact manifest and
  sidecar, verify the immutable `fixtures/ast` snapshot, capture only regular Git
  `100644` JSON, close revalidation, then atomically publish sidecar plus snapshot
  as one CleanFull import while preserving downstream bytes.
- RED evidence: first add public construction and packaged invalid-syntax tests,
  manifest matrices, SHA-1/SHA-256 sidecar goldens, report-path collisions,
  source pin/snapshot/replacement cases, stale removal, unchanged import, and
  downstream-preservation tests. They fail because no CSS front or executor exists.
- Acceptance criteria: every new production item is reached by the public import
  path with no placeholder arm or artificial/dead-code linkage; constructor and
  parser are I/O-free through syntax validation; the binary is at most 15 lines
  with exact error/exit behavior; schema, count, mode, path, digest, and source
  proof are exact; unknown old entries fail; changed sidecar makes classifiable
  downstream stale without mutation; unchanged import retains freshness; C01
  publication/error/recovery semantics remain exact.
- Commands:
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus --lib css_manifest_`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus --lib css_import_`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus --test public_api`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus --test css_cli`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features`
  - `cargo test --locked --offline -p surgeist-generator --all-features --lib -- --ignored --list`
  - `cargo metadata --locked --offline --no-deps --format-version 1`
  - `cargo clippy --locked --offline -p surgeist-generator --features css-corpus --all-targets -- -F unsafe-code -D warnings`
  - `cargo fmt --check`
- Dependencies: C02-T01 is task-clean.
- Intended commit: `feat(css): import protected csstree fixtures`.

### C02-T03 — Add complete full CSS generation

- Files/area: additive `Generate` command/front dispatch, fixture prepass, typed
  derivation, disposition resolution, expectation/report models, historical
  inventory validation, full orchestration, artifact planning, and focused tests.
- Intended behavior/outcome: make unfiltered `Generate` a complete reachable
  command. Derive exact neutral SR-07.3 expectations, validate the old full report
  as sole historical authority, and atomically publish the complete expectation
  set plus report. The request and CLI reject filters until C02-T04 completes.
- RED evidence: first add expectation byte/order goldens, pointer escaping,
  duplicate-at-depth, malformed/empty fixture, options/default/override/count
  cases, historical removal/rename/addition and malformed-authority tests, report
  provenance/digest cases, stale cleanup, unknown entry, and persisted report
  collision. They fail because `Generate` and derivation do not exist.
- Acceptance criteria: streaming rejects duplicate decoded members and trailing
  values; IDs/context/outcomes/options/dispositions/reasons are exact and ordered;
  no AST, prose, offsets, comments, or recovery data persists; `historical ∪
  desired` classifies every visible entry; malformed authority fails before
  intent; CleanFull removes only classified stale outputs; any derivation failure
  publishes nothing; the real public/CLI command is warning-clean and threadless.
- Commands:
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus --lib css_expectation_`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus --lib css_historical_inventory_`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus --lib css_full_generate_`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus`
  - `cargo test --locked --offline -p surgeist-generator --all-features --lib -- --ignored --list`
  - `cargo clippy --locked --offline -p surgeist-generator --features css-corpus --all-targets -- -F unsafe-code -D warnings`
  - `cargo fmt --check`
- Dependencies: C02-T02 is task-clean.
- Intended commit: `feat(css): publish full neutral expectations`.

### C02-T04 — Filter only historically owned CSS expectations

- Files/area: additive Generate filter acceptance, CLI parsing, exact matching,
  selection ledger, historical ownership gate, filtered artifact planning, and
  focused public/domain/process tests.
- Intended behavior/outcome: complete the final optional-filter capability on the
  already real `Generate` command. Select current sidecar fixtures by exact
  `.json` or complete component prefix and update only expectations owned by the
  validated current-schema historical report.
- RED evidence: first add exact-file/component-prefix and partial/reserved/zero
  match cases, absent-root verification, add-then-rename requiring full creation,
  persisted report collision, and process option-matrix tests. They fail because
  C02-T03 deliberately rejects every filter.
- Acceptance criteria: syntax/reserved errors precede I/O; zero match and unowned
  selection are `Verification` before lease; absent final root cannot be filtered;
  selected whole fixtures update atomically; every unselected expectation/report
  byte is preserved; filtered runs never write/remove reports or prune stale
  outputs; the public accessor and CLI expose the completed behavior directly.
- Commands:
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus --lib css_filter_`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus --lib css_filtered_`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus --test css_cli`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus`
  - `cargo test --locked --offline -p surgeist-generator --all-features --lib -- --ignored --list`
  - `cargo clippy --locked --offline -p surgeist-generator --features css-corpus --all-targets -- -F unsafe-code -D warnings`
  - `cargo fmt --check`
- Dependencies: C02-T03 is task-clean.
- Intended commit: `feat(css): filter owned expectations`.

### C02-T05 — Add read-only CSS corpus checking

- Files/area: additive `CheckCorpus` command/front dispatch, current-state reader,
  sidecar/expectation/report/inventory verifier, read-only coordination integration,
  and focused public/domain/process tests.
- Intended behavior/outcome: make `check-corpus` a complete reachable command that
  validates manifest, current sidecar/files, exact expectation bytes/schema,
  counts, hashes, provenance, report relationships, inventory, and coordination
  without Git invocation, recovery, lease acquisition, repair, or mutation.
- RED evidence: first add public/CLI matrix tests, current/stale/absent/unknown/
  malformed states, active/resumable/malformed coordination, outside-sentinel
  snapshots, and persisted report collision. They fail because no check path exists.
- Acceptance criteria: current returns `Ok`; known absent/stale state and any
  coordination state return `Verification`; malformed artifacts/authority or
  unknown inventory return `InvalidInventory`; the command never bootstraps,
  recovers, leases exclusively, removes, invokes Git, or changes any byte/identity;
  the new enum variant and dispatch are real and warning-clean immediately.
- Commands:
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus --lib css_check_`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus --lib css_read_only_`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus --test public_api`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus --test css_cli`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus`
  - `cargo test --locked --offline -p surgeist-generator --all-features --lib -- --ignored --list`
  - `cargo clippy --locked --offline -p surgeist-generator --features css-corpus --all-targets -- -F unsafe-code -D warnings`
  - `cargo fmt --check`
- Dependencies: C02-T04 is task-clean.
- Intended commit: `feat(css): verify corpus state read only`.

### C02-T06 — Close the exact CSS interface and transition matrix

- Files/area: final CSS public/front integration, cross-command state fixtures,
  Cargo metadata, rustdoc/examples, process tests, CSS-replaced artificial core
  references, and an exact C04 handoff of retained layout-only references.
- Intended behavior/outcome: prove the accumulated surface is exactly SR-05.3 and
  the CLI exactly SR-05.4, with all three commands routed to their real domain
  paths and every import/generate/filter/check transition coherently composed.
- RED evidence: first add full feature API trait/accessor/invalid-matrix tests and
  source-change/freshness, import-to-generate, filtered-to-check, historical
  membership, collision-precedence, and exact error-rendering process sequences.
  They expose any mismatch left by the individually complete vertical increments.
- Acceptance criteria: `CssRequest::new` is I/O-free and enforces the final exact
  matrix; `run` is synchronous/threadless; `run_from_env` uses only `args_os`;
  invalid syntax is `Cli`; surface/rustdoc/examples are exact and no core type
  leaks; metadata has only the required CSS target; the binary remains at most 15
  lines; real CSS callers replace only the artificial references they reach;
  `private_front_doors_are_linked` and every enumerated layout-only reference,
  including `DiagnosticFull`, remain for C04; default/layout-only remain clean.
- Commands:
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus --test public_api`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus --test css_cli`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features`
  - `cargo test --locked --offline -p surgeist-generator --all-features --lib -- --ignored --list`
  - `cargo metadata --locked --offline --no-deps --format-version 1`
  - `cargo clippy --locked --offline -p surgeist-generator --all-features --all-targets -- -F unsafe-code -D warnings`
  - `cargo fmt --check`
- Dependencies: C02-T05 is task-clean.
- Intended commit: `feat(css): close generator interface`.

## Completion

- Observable acceptance: the shared protected-source boundary is identity-bound
  and revalidated; all three CSS commands operate through explicit synthetic
  roots with exact schemas, source proof, deterministic case bytes, current and
  historical inventory authority, full/filtered/check semantics, reports,
  atomic stale removal, error precedence, and real CLI behavior; no layout,
  dependency, lockfile, corpus, generated artifact, or preservation change
  occurs; ordinary matrices are clean and exactly 15 diagnostics remain skipped.
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
  - `cargo metadata --locked --offline --no-deps --format-version 1`
  - `cargo fmt --check`
  - `git diff --check`
  - `git diff --quiet efcb868905025270c875fc683d82cfba3c029080..HEAD -- Cargo.lock`
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
- Required handoff: immutable published C02 SHA; exact ordered task ranges and
  clean task/holistic reviews; exact 15-name deferred inventory and proof no
  ignored body ran; public CSS API/binary/metadata evidence; synthetic source,
  import, generation, filtering, checking, report, and CLI evidence; final
  matrix; preservation digest; clean authority-remote readback; exact remaining
  layout-only artificial-linkage inventory for C04 (including `DiagnosticFull`);
  and an explicit statement that C03 alone may begin next while C04 alone owns
  final-linkage closure and ignored runtime.
- Unresolved blocker: none. Missing installed cache/tool, unsupported declared
  host behavior, inability to preserve protected-source or atomic-publication
  invariants, a required dependency/lockfile change, or contradiction with the
  reviewed specification stops implementation for plan correction.
