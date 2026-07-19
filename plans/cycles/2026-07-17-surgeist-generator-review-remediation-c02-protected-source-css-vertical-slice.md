# C02 Protected Source And CSS Vertical Slice

## Header

- Cycle ID: `C02`
- Owning repository: `/Users/codex/Development/surgeist-generator`
- Status: `draft`
- Cycle base: `efcb868905025270c875fc683d82cfba3c029080`
- Immutable implementation-series source base:
  `1e545530fb992e2e749f26a1ba7ac0d77ed85dd6`
- Published prerequisite: C01 candidate
  `efcb868905025270c875fc683d82cfba3c029080`; local `main`, `origin/main`, and
  authority-remote `main` were equal at the C01 readback.
- Reviewed specification:
  `plans/specs/2026-07-17-surgeist-generator-review-remediation.md`, CSS and
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
  `plans/sequences/2026-07-17-surgeist-generator-review-remediation.md`, entry
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
- The exact cumulative ignored inventory remains the 15 C01 diagnostics. C02
  adds no ignored test. Ordinary task and final matrices compile and skip them;
  the only permitted ignored command also contains `--list` and compares the
  exact names. No ignored body runs before the one initiative-final C04 command.
- `private_front_doors_are_linked` remains until T08 proves every represented
  CSS core path has a real caller; T08 then removes only artificial references
  made obsolete by the production front door.
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

### C02-T02 — Validate CSS manifests and canonical import sidecars

- Files/area: private feature-gated `src/css/` module, manifest and import-model
  source, plus focused tests.
- Intended behavior/outcome: implement the exact SR-07.1 schema-1 manifest and
  strict SHA-1/SHA-256 import-sidecar model without yet publishing a corpus.
- RED evidence: first add manifest matrix tests plus
  `css_import_sidecar_sha1_golden`, `css_import_sidecar_sha256_golden`, and
  reserved-report collision cases. They fail because no CSS domain/schema exists.
- Acceptance criteria: unknown/duplicate fields, invalid pins/counts/roots/cases,
  unmatched overrides, and either reserved collision are rejected with the exact
  owning kind; canonical sidecars have sorted unique records, fixed object-ID
  widths, exact compact bytes, and one final LF.
- Commands:
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus --lib css_manifest_`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus --lib css_import_sidecar_`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus`
  - `cargo test --locked --offline -p surgeist-generator --all-features --lib -- --ignored --list`
  - `cargo clippy --locked --offline -p surgeist-generator --features css-corpus --all-targets -- -F unsafe-code -D warnings`
  - `cargo fmt --check`
- Dependencies: C02-T01 is task-clean.
- Intended commit: `feat(css): validate manifest and import proof`.

### C02-T03 — Import CSSTree atomically from a protected checkout

- Files/area: CSS import executor plus necessary private source, lease,
  inventory, and artifact integration.
- Intended behavior/outcome: verify the explicit checkout and immutable
  `fixtures/ast` snapshot, capture only regular Git `100644` JSON, run protected
  closing revalidation, and publish the sidecar plus exact snapshot as one
  clean-full import-root transaction while preserving downstream bytes.
- RED evidence: first add `css_import_rejects_report_path_collision`, source
  pin/snapshot/replacement cases, stale-file removal, unchanged-import, and
  downstream-preservation tests. They fail before any import intent because no
  CSS executor consumes protected source snapshots or publication primitives.
- Acceptance criteria: exact count/mode/path/digest proof is enforced; unknown
  old import entries fail; stale known files disappear atomically; sidecar digest
  change makes existing downstream stale without mutating it; unchanged import
  retains freshness; pre/post-commit errors follow SR-04.5 and leave resumable
  evidence only when recovery cannot finish.
- Commands:
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus --lib css_import_`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus --lib css_source_`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus`
  - `cargo test --locked --offline -p surgeist-generator --all-features --lib -- --ignored --list`
  - `cargo clippy --locked --offline -p surgeist-generator --features css-corpus --all-targets -- -F unsafe-code -D warnings`
  - `cargo fmt --check`
- Dependencies: C02-T02 is task-clean.
- Intended commit: `feat(css): import protected csstree fixtures`.

### C02-T04 — Derive neutral expectations deterministically

- Files/area: CSS fixture prepass, typed case derivation, disposition resolution,
  canonical options, expectation model/serialization, and focused tests.
- Intended behavior/outcome: turn each validated imported fixture into the exact
  SR-07.3 neutral expectation without retaining AST, diagnostic prose, offsets,
  comments, recovery data, or source object-member order.
- RED evidence: first add `css_expectation_case_order_golden`, ordinary/error
  byte goldens, JSON-pointer escaping, duplicate-depth, malformed/empty fixture,
  options ordering, default/override, repeated-source, and count-mismatch tests.
- Acceptance criteria: the streaming prepass rejects duplicate decoded members
  at every depth and trailing values; IDs/context/outcomes are exact; cases sort
  by final escaped ID; options recurse deterministically; dispositions/reasons
  classify each case once; pretty JSON field order and final LF are exact.
- Commands:
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus --lib css_expectation_`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus --lib css_fixture_`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus`
  - `cargo test --locked --offline -p surgeist-generator --all-features --lib -- --ignored --list`
  - `cargo clippy --locked --offline -p surgeist-generator --features css-corpus --all-targets -- -F unsafe-code -D warnings`
  - `cargo fmt --check`
- Dependencies: C02-T03 is task-clean.
- Intended commit: `feat(css): derive neutral expectations`.

### C02-T05 — Publish full CSS generations with historical authority

- Files/area: CSS full-generation orchestration, report/historical validation,
  inventory classification, artifact planning, and focused tests.
- Intended behavior/outcome: validate current import proof, derive the complete
  desired set, validate the old full report as the sole historical authority,
  and atomically publish all expectations plus one exact shared report.
- RED evidence: first add
  `css_historical_inventory_removal_rename_addition_regenerates`,
  `css_historical_inventory_rejects_malformed_authority`, report
  provenance/count/digest cases, stale-root cleanup, unknown-entry, and
  `css_full_generate_rejects_persisted_report_path_collision` tests.
- Acceptance criteria: `historical ∪ desired` classifies every visible entry;
  malformed/missing nonempty authority fails before intent; CleanFull removes
  only classified stale outputs; report artifacts/counts/provenance bind every
  fixture/case/output exactly; any derivation failure publishes nothing; durable
  failures follow the C01 commit oracle.
- Commands:
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus --lib css_historical_inventory_`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus --lib css_full_generate_`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus`
  - `cargo test --locked --offline -p surgeist-generator --all-features --lib -- --ignored --list`
  - `cargo clippy --locked --offline -p surgeist-generator --features css-corpus --all-targets -- -F unsafe-code -D warnings`
  - `cargo fmt --check`
- Dependencies: C02-T04 is task-clean.
- Intended commit: `feat(css): publish full expectation generations`.

### C02-T06 — Filter only historically owned CSS expectations

- Files/area: CSS filter matching, selection ledger, historical ownership gate,
  filtered artifact planning, and focused tests.
- Intended behavior/outcome: select current sidecar fixtures by exact `.json` or
  complete component prefix and update only expectations already owned by the
  validated current-schema historical report.
- RED evidence: first add `css_filter_exact_file`,
  `css_filter_component_prefix`, `css_filter_rejects_partial_component`,
  `css_filter_rejects_reserved`, `css_filter_absent_is_verification`,
  `css_filtered_add_then_rename_requires_full_before_creation`, and persisted
  report-collision tests.
- Acceptance criteria: syntax/reserved errors precede I/O; zero match and
  unowned selection are `Verification` before lease; absent final root cannot be
  filtered; selected whole fixtures/cases update atomically; every unselected
  expectation/report byte is preserved; filtered runs never write/remove reports
  or prune stale outputs.
- Commands:
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus --lib css_filter_`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus --lib css_filtered_`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus`
  - `cargo test --locked --offline -p surgeist-generator --all-features --lib -- --ignored --list`
  - `cargo clippy --locked --offline -p surgeist-generator --features css-corpus --all-targets -- -F unsafe-code -D warnings`
  - `cargo fmt --check`
- Dependencies: C02-T05 is task-clean.
- Intended commit: `feat(css): filter owned expectations`.

### C02-T07 — Verify CSS corpora without mutation

- Files/area: CSS current-state reader, sidecar/expectation/report/inventory
  verifier, read-only coordination integration, and focused tests.
- Intended behavior/outcome: make `check-corpus` validate the manifest, current
  sidecar/files, every expectation byte/schema, counts, hashes, provenance,
  report relationships, exact inventory, and coordination without Git or repair.
- RED evidence: first add current/stale/absent/unknown/malformed state tests,
  active/resumable/malformed coordination tests, outside-sentinel snapshots, and
  `css_check_rejects_persisted_report_path_collision`.
- Acceptance criteria: current returns `Ok`; known absent/stale state and any
  coordination state return `Verification`; malformed artifacts/authority or
  unknown inventory return `InvalidInventory`; the command never bootstraps,
  recovers, leases exclusively, removes, or changes any byte/identity.
- Commands:
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus --lib css_check_`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus --lib css_read_only_`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus`
  - `cargo test --locked --offline -p surgeist-generator --all-features --lib -- --ignored --list`
  - `cargo clippy --locked --offline -p surgeist-generator --features css-corpus --all-targets -- -F unsafe-code -D warnings`
  - `cargo fmt --check`
- Dependencies: C02-T06 is task-clean.
- Intended commit: `feat(css): verify corpus state read only`.

### C02-T08 — Expose the exact CSS API and thin real binary

- Files/area: `src/lib.rs`, CSS request/dispatch/parser front, Cargo binary
  target, `src/bin/surgeist-css-generate.rs`, public API/process integration
  tests, and obsolete artificial linkage only.
- Intended behavior/outcome: expose exactly SR-05.3 behind `css-corpus`; parse the
  exact SR-07.1 CLI matrix; route all three commands to their real domain paths;
  and package a binary of at most 15 physical lines with exact prefix/exit code.
- RED evidence: first add feature public-API construction/trait/accessor/matrix
  tests and a packaged invalid-syntax process test requiring no stdout, exact
  the `surgeist-css-generate: ` prefix followed by the rendered error on stderr,
  exit 64, and no filesystem access.
  They fail because no public module, parser, target, or real binary exists.
- Acceptance criteria: `CssRequest::new` is I/O-free and rejects the exact option
  mismatches; `run` is synchronous/threadless and calls real import/generate/check;
  `run_from_env` uses only `args_os`; invalid syntax is `Cli`; public surface is
  exact/additive with acquisition-free rustdoc; no core implementation type leaks;
  Cargo metadata has exactly the required CSS target; artificial linkage is gone
  only where real CSS callers replace it; default/layout-only builds remain clean.
- Commands:
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus --test public_api`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus --test css_cli`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features`
  - `cargo test --locked --offline -p surgeist-generator --all-features --lib -- --ignored --list`
  - `cargo metadata --locked --offline --no-deps --format-version 1`
  - `cargo clippy --locked --offline -p surgeist-generator --all-features --all-targets -- -F unsafe-code -D warnings`
  - `cargo fmt --check`
- Dependencies: C02-T07 is task-clean.
- Intended commit: `feat(css): expose generator interface`.

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
  matrix; preservation digest; clean authority-remote readback; and an explicit
  statement that C03 alone may begin next while C04 alone owns ignored runtime.
- Unresolved blocker: none. Missing installed cache/tool, unsupported declared
  host behavior, inability to preserve protected-source or atomic-publication
  invariants, a required dependency/lockfile change, or contradiction with the
  reviewed specification stops implementation for plan correction.
