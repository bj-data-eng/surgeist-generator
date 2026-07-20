# C03 Browser-Free Layout Corpus Interface

## Header

- Cycle ID: `C03`
- Owning repository: `/Users/codex/Development/surgeist-generator`
- Status: `in_progress`
- Cycle base: `90958976f171f2153d01efd993b958071d052831`
- Immutable implementation-series source base:
  `1e545530fb992e2e749f26a1ba7ac0d77ed85dd6`
- Published prerequisite: C02 candidate
  `90958976f171f2153d01efd993b958071d052831`; local `main`, `origin/main`, and
  authority-remote `main` were equal at the C02 readback.
- Reviewed specification:
  `plans/specs/2026-07-17-surgeist-generator-review-remediation.md`, the
  browser-free layout clauses of SR-01 and the layout portion of SR-02
  **Missing domain surface**, **Tautological CLI test**, and affected quality
  obligations; layout import/check clauses of SR-03.1 and SR-03.2; SR-04.5
  layout import, read-only checking, historical authority, inventory, and
  scoped-report selection clauses; SR-04.6 browser-free layout linkage; the
  layout target without its later heavy dependency edge in SR-05.1; the
  browser-free capability set in SR-05.2; browser-free interface clauses of
  SR-05.4; SR-06.1, SR-06.2, and the offline inventory/XML/report verification
  clauses of SR-06.3; and corresponding SR-08.1 and affected SR-08.3 clauses;
  at commit `d2fbbedb033177731af5487d3498ba7f14b721d8`, normalized semantic-content
  SHA-256
  `faa4320f1e06ad9c003f2525fcf7171e387458eacc4ec3fd0d2d88f7c0e1eb71`,
  review `CLEAN`.
- Reviewed implementation sequence:
  `plans/sequences/2026-07-17-surgeist-generator-review-remediation.md`, entry
  C03, at commit `faad9c1406b0cda68d9ce087a8cc3e06e6205360`, normalized
  semantic-content SHA-256
  `590c79d705cd9657a649b2a303e01437beda6facb538f08d85f86ae87392e3f6`,
  review `CLEAN`.
- Bounded outcome: ship one complete production-reachable browser-free layout
  capability set and thin binary containing `import-taffy`, `check-taffy-corpus`,
  and `check-corpus`, with canonical schema-2 parsing, manifest-owned Taffy
  pins/counts, partition-safe import, exact historical inventory, and offline
  HTML/XML/report attestation.

## Boundary

- Mutate, commit, and publish only this repository. Root `surgeist`, sibling
  crates, their corpora, Git checkouts, gitlinks, scripts, and generated
  artifacts remain outside read, mutation, and test scope.
- C01 transaction/coordination/recovery and C02 protected-source/inventory/report
  primitives remain the only mutation and source-verification foundations. C03
  may add layout-private adapters and validators, but may not weaken their path,
  identity, durability, historical-authority, error, or recovery guarantees.
- `src/layout/legacy_generator.rs` remains exactly 4,626 lines with SHA-256
  `d2f5ca87cea6b36826e9172e2d7ba7a99196c375e2ca53f8a84a075200e70a9f`.
  It is not edited, formatted, placed in the module graph, compiled, or tested.
  Compatibility tests use an independently written test-only preserved-contract
  adapter; they do not `include!` or otherwise compile the preservation source.
  C03 maps the browser-free responsibilities it replaces and the exact
  generation-only remainder for C04, but only C04 may remove the file.
- C03 exposes only the independently usable browser-free SR-05.2 capability set:
  non-exhaustive `LayoutCommand::{ImportTaffy, CheckTaffyCorpus, CheckCorpus}`;
  private-field `LayoutRequest`; its three command-specific constructors;
  `location`, `command`, and `source_root`; and `run`/`run_from_env`. No
  `Generate` variant, `generate` constructor, `browser_path`/`filter` accessor,
  browser backend, Tokio runtime, supervisor, profile, HTML measurement, XML
  renderer, generation serializer, filtered-generation path, artificial caller,
  placeholder arm, or provisional public API exists in this cycle.
- `layout-browser = []` gains real dependency-free browser-free code and one
  `surgeist-layout-generate` target with
  `required-features = ["layout-browser"]`. C03 adds no dependency, `deny.toml`,
  browser feature edge, or lockfile change. Package version 0.1.0, edition 2024,
  Rust 1.97, MIT, default features, shared dependencies, CSS target, and CSS
  behavior remain unchanged. The heavy dependency edge and policy gates belong
  atomically to C04 generation.
- Verification uses explicit synthetic owner/corpus/source roots and
  already-installed tooling only. No network, Git clone/fetch, sibling corpus,
  production fixture, browser executable, external launch, dependency/toolchain/
  target acquisition, import into a real corpus, or system-wide mutation is
  permitted. Test-owned local Git repositories exercise the existing sanitized
  read-only source runner.
- The schema-2 parser preserves the reviewed acceptance set and applies only its
  four declared tightenings. The corpus manifest owns both identical lowercase
  40- or 64-hex Taffy revision fields and the positive pre-exclusion source
  count. Generator code owns their grammar/equality, canonical repository/source/
  destination/exclusion policy, explicit-checkout proof, and sidecar/report
  binding; it contains no production Taffy revision or inventory-count constant.
- `import-taffy` treats `html` as one complete partition-aware publication root.
  Before lease acquisition it derives exact authored/current-Taffy/desired-Taffy
  sets, rejects every collision or unknown entry, proves writable/protected
  disjointness, snapshots and identity-binds authored files and the immutable
  source tree, and recognizes exactly sidecar mode or sidecar-free legacy mode.
  Under the held mutex it revalidates every protected authority before intent
  and atomically installs byte-identical authored files plus only the desired
  Taffy files and canonical sidecar. It never writes XML/reports.
- `check-taffy-corpus` is a read-only comparison of the explicit checkout,
  manifest pin/count, sidecar, and imported files. `check-corpus` is fully offline:
  it opens no Git checkout or browser cache/executable and validates manifest,
  helpers, authored/Taffy inventory, current or migration-only historical report
  authority, XML comment/body/output digests, report relationships, dispositions,
  counts, scoped subsets, provenance, and coordination state. Both use only
  `GenerationCheck`, create/recover/remove nothing, and leave every byte and
  identity unchanged.
- Error precedence remains exact: request/CLI syntax; manifest; required current
  import and complete historical/downstream inventory with their owning kinds;
  source and namespace proof where applicable; selection/freshness
  `Verification`; then lease/check acquisition. Source pin/object/snapshot drift
  is `SourceVerification`; absent or classifiable stale state is `Verification`;
  malformed authority or an unknown entry is `InvalidInventory`; read-only
  coordination state is always `Verification`, never a repairing lease or
  transaction error.
- A successful import changes downstream freshness only through the canonical
  sidecar digest: changed sidecar plus existing classifiable XML/report state is
  stale until C04 full generation; unchanged sidecar preserves prior freshness;
  absent downstream remains absent. Current-schema full/scoped reports are exact
  ownership/freshness authority. Uniform legacy schema-2 reports establish only
  stale ownership, forbid later filtered publication, and require C04 full
  migration; mixed legacy/current reports or unknown paths are invalid.
- Offline browser fields are historical attestations, not authentication of an
  installed executable. `check-corpus` recomputes every corpus-derived value and
  XML digest, requires exact cross-artifact browser provenance/digest equality,
  and accepts a self-consistent canonical historical rewrite while ignoring an
  absent/replaced/drifted cache executable. C03 implements validators only, not a
  production XML/report generator.
- The exact cumulative ignored inventory is the following 15 fully qualified
  names. C03 adds no ignored test. Every ordinary matrix compiles and skips them;
  every inventory command is list-only and must equal this set exactly:
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
  No C03 command may contain `--ignored` without `--list`. Only C04, after every
  initiative implementation task is complete, may execute the one authorized
  sequential all-features ignored body.
- Real layout browser-free callers replace only the artificial core references
  they actually reach. `private_front_doors_are_linked` and every exact
  generation-only reference, including `PublicationPolicy::DiagnosticFull`,
  remain explicitly inventoried for C04; C03 uses no fake call, lint allowance,
  or dead-code suppression to claim generation linkage.
- Supported mutation remains Apple-Silicon macOS. Default and feature-isolated
  public contracts remain warning-clean on their declared portable checks; no
  target-specific mutation helper is widened merely to satisfy compilation.

## Impacts

- Public API: additive only under `layout-browser`; exactly the C03 browser-free
  subset of SR-05.2; C02 CSS and default public surfaces are unchanged.
- Dependencies/features/lockfile: no dependency or lockfile change; the existing
  `layout-browser` feature gains real browser-free code and one required-feature
  binary target.
- Generated artifacts/fixtures: no committed corpus or generated output; every
  fixture is a test-owned temporary tree.
- Documentation/examples: acquisition-free layout rustdoc/examples are added for
  the three commands and the offline-attestation boundary. Repository README and
  AGENTS final-state guidance remain allocated to C04.
- MSRV/target: Rust 1.97 unchanged; mutation remains Apple-Silicon macOS only.
- Root/API artifacts: none; root composition, audit, sibling adoption, and gitlink work remain excluded.
- Unsafe: no executable `unsafe`; `#![forbid(unsafe_code)]` remains.

## Tasks

### C03-T01 — Expose and execute partition-safe Taffy import

- Files/area: `Cargo.toml`, `src/lib.rs`, the real feature-gated public `layout`
  front and private manifest/case/sidecar/import modules, the thin binary,
  public/process tests, and C02 protected-source/publication integration.
- Intended behavior/outcome: expose the non-exhaustive `ImportTaffy` command,
  final-signature `LayoutRequest::import_taffy` plus applicable accessors, and
  real `run`/`run_from_env`/binary paths. Parse the complete schema-2 contract,
  verify the manifest-owned pin and immutable `test_fixtures` snapshot, derive
  exclusions/counts, construct the canonical sidecar, classify sidecar and
  sidecar-free modes, retain authored HTML by held identity/bytes, and publish
  one complete partition-safe `html` tree through `CleanFull`.
- RED evidence: first add public construction and packaged invalid-syntax tests;
  full-field schema goldens and declared-tightening matrices; SHA-1/SHA-256
  sidecar goldens; authored/Taffy collision, malformed-sidecar, unknown-entry,
  stale/missing legacy fixture, source-pin/snapshot/replacement, downstream-
  preservation, and pre/post-commit import tests. They fail because the layout
  front, schema parser, sidecar, partition classifier, and importer do not exist.
- Acceptance criteria: every new production item is reached through the public
  import path with no placeholder or artificial linkage; request/CLI parsing is
  I/O-free through syntax validation; the binary is at most 15 lines with exact
  prefix/exit behavior; both revision fields match and any valid pin/positive
  count drives the same compiled contract; source count is pre-exclusion and file
  records are strictly sorted with exact modes/object widths/digests; authored
  bytes/identity survive exactly; stale Taffy disappears; unknown/alias/link/
  mode/collision state fails before intent; XML/reports remain untouched; C01/C02
  durability, closing revalidation, and error semantics remain exact.
- Commands:
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser --lib layout_schema2_`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser --lib layout_taffy_sidecar_`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser --lib layout_taffy_`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser --test public_api`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser --test layout_cli`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features`
  - `cargo test --locked --offline -p surgeist-generator --all-features -- --ignored --list`
  - `cargo metadata --locked --offline --no-deps --format-version 1`
  - `cargo clippy --locked --offline -p surgeist-generator --features layout-browser --all-targets -- -F unsafe-code -D warnings`
  - `cargo fmt --check`
- Dependencies: published C02 candidate and reviewed C03 plan only.
- Intended commit: `feat(layout): import protected taffy fixtures`.

### C03-T02 — Add explicit read-only Taffy source checking

- Files/area: additive `CheckTaffyCorpus` command/constructor/front dispatch,
  source-sidecar-file comparison, read-only coordination integration, pin-update
  transitions, and focused public/domain/process tests.
- Intended behavior/outcome: make `check-taffy-corpus` a complete reachable
  command that verifies the explicit checkout pin/object format/immutable
  snapshot and manifest-owned pre-exclusion count against the canonical sidecar
  and imported files without lease recovery, import, repair, or mutation.
- RED evidence: first add checkout revision/object/snapshot drift, absent/stale
  known import, malformed/unknown inventory, sidecar/file digest, SHA-1/SHA-256,
  coordination-state, outside-sentinel, and two-valid-pin/count transition tests.
  They fail because C03-T01 exposes only the mutation command.
- Acceptance criteria: a matching source/import returns `Ok`; pin/object/snapshot
  mismatch is `SourceVerification`; absent or classifiable stale import is
  `Verification`; malformed sidecar or unknown entry is `InvalidInventory`;
  active/resumable/malformed coordination is `Verification`; the command uses
  only `GenerationCheck`, never bootstraps, recovers, acquires exclusively,
  installs, removes, or changes bytes/identities; updating manifest pin/count
  makes the old sidecar stale until the same binary reimports the newly named
  checkout, with no generator source change.
- Commands:
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser --lib layout_check_taffy_`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser --lib layout_taffy_pin_and_count_`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser --lib layout_read_only_`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser --test public_api`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser --test layout_cli`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser`
  - `cargo test --locked --offline -p surgeist-generator --all-features -- --ignored --list`
  - `cargo clippy --locked --offline -p surgeist-generator --features layout-browser --all-targets -- -F unsafe-code -D warnings`
  - `cargo fmt --check`
- Dependencies: C03-T01 is task-clean.
- Intended commit: `feat(layout): verify taffy corpus read only`.

### C03-T03 — Add complete offline layout corpus checking

- Files/area: additive `CheckCorpus` command/constructor/front dispatch; helper,
  HTML, sidecar, XML/report, historical-inventory, scoped-subset, disposition,
  offline-attestation and coordination validators; and focused tests.
- Intended behavior/outcome: make `check-corpus` one complete reachable offline
  command. Validate the complete schema-2 corpus and current imported HTML;
  classify the exact historical-plus-desired XML/report union; validate uniform
  current or migration-only legacy ownership; recompute corpus-derived
  provenance, XML bytes/digests, summaries, buckets, mappings, and scoped subsets;
  and report current/stale/invalid state without opening Git or browser resources.
- RED evidence: first add current/absent/stale/diagnostic/legacy/mixed/unknown
  inventory cases; malformed historical authority and membership-delta cases;
  XML comment duplicate/unknown/order/optional-field/body-tamper cases; report
  digest/count/disposition/variant/scoped-subset cases; active/resumable/malformed
  coordination and outside-sentinel cases; and the five named offline browser-
  attestation tests. They fail because no corpus checker, layout authority model,
  XML validator, or report decoder exists.
- Acceptance criteria: exact current state returns `Ok`; missing/stale/diagnostic
  or uniform legacy-owned state returns `Verification`; unknown entries, mixed
  authority schemas, malformed known artifacts, impossible mapping/coverage, or
  invalid report/comment structure return `InvalidInventory`; every generated
  entry binds one exact four-variant output and recomputed complete-byte digest;
  full/scoped metadata/provenance/buckets/counts are canonical and coherent;
  migration-only reports establish stale ownership only; missing/replaced/drifted
  browser cache bytes do not affect checking, but cross-artifact attestation drift
  fails and a self-consistent canonical rewrite passes; the command performs no
  source invocation, browser/cache access, generation, serialization of production
  XML/reports, coordination repair, lease mutation, or byte/identity change.
- Commands:
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser --lib layout_check_corpus_`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser --lib layout_historical_inventory_`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser --lib layout_legacy_report_`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser --lib layout_xml_`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser --lib layout_report_`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser --lib layout_provenance_`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser --test public_api`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser --test layout_cli`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser`
  - `cargo test --locked --offline -p surgeist-generator --all-features`
  - `cargo test --locked --offline -p surgeist-generator --all-features -- --ignored --list`
  - `cargo clippy --locked --offline -p surgeist-generator --features layout-browser --all-targets -- -F unsafe-code -D warnings`
  - `cargo fmt --check`
- Dependencies: C03-T02 is task-clean.
- Intended commit: `feat(layout): verify corpus attestations offline`.

### C03-T04 — Close schema compatibility, transitions, and the exact interface

- Files/area: final browser-free layout public/front integration, test-only
  preserved-contract adapter, cross-command synthetic state fixtures, Cargo
  metadata, rustdoc/examples, process tests, layout-replaced artificial core
  references, and an exact C04 responsibility/linkage handoff.
- Intended behavior/outcome: prove the accumulated surface is exactly the
  browser-free SR-05.2 set and CLI subset, all three commands route to their real
  domain paths, schema-2 compatibility is bounded to the four declared
  tightenings, and import/check state transitions compose without introducing
  generation-only code.
- RED evidence: first add full feature API trait/accessor/invalid-matrix tests;
  exact CLI command/option/error/exit tests; all named schema-2 compatibility
  fixtures through both the independent preserved adapter and new representation;
  unchanged-versus-changed import freshness, sidecar-free migration, pin/count
  update/reimport, authored/Taffy membership change, legacy/current authority,
  coordination precedence, and exact error-rendering process sequences. They
  expose any mismatch left by the individually complete command increments.
- Acceptance criteria: request constructors are I/O-free and cannot construct a
  browser/filter or mismatched source payload; `run_from_env` uses only `args_os`
  and no operator environment configuration; invalid syntax is `Cli`; the binary
  remains at most 15 lines and all three real process commands execute synthetic
  roots; surface/rustdoc/examples are exact and no private/core type leaks; the
  manifest accepts any grammar-valid matching Taffy pin/count without a source
  constant; only the four reviewed parser tightenings diverge from preserved
  behavior; metadata has exactly the CSS and layout required-feature binaries;
  no generation command/dependency/code exists; real layout callers replace only
  reached artificial references; the exact retained C04-only references and
  preservation responsibilities are enumerated; default/CSS/layout/all-feature
  matrices remain clean.
- Commands:
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser --test public_api`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser --test layout_cli`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser`
  - `cargo test --locked --offline -p surgeist-generator --features css-corpus`
  - `cargo test --locked --offline -p surgeist-generator --all-features`
  - `cargo test --locked --offline -p surgeist-generator --all-features -- --ignored --list`
  - `cargo metadata --locked --offline --no-deps --format-version 1`
  - `cargo clippy --locked --offline -p surgeist-generator --all-features --all-targets -- -F unsafe-code -D warnings`
  - `cargo fmt --check`
- Dependencies: C03-T03 is task-clean.
- Intended commit: `feat(layout): close browser-free interface`.

## Completion

- Observable acceptance: the production library and packaged layout binary
  execute exactly the three browser-free commands against explicit synthetic
  roots; schema compatibility, corpus-owned pin/count updates, partitioned
  import, source proof, exact current/historical inventory, XML/report/digest
  relationships, stale-state classifications, offline browser-attestation
  boundary, and read-only coordination behavior satisfy the affected matrix;
  CSS/default behavior stays clean; no dependency, lockfile, browser generation,
  committed corpus/output, or preservation change occurs; ordinary matrices are
  clean and exactly 15 diagnostics remain skipped.
- Final command list:
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
  - `cargo fmt --check`
  - `git diff --check`
  - `git diff --quiet 90958976f171f2153d01efd993b958071d052831..HEAD -- Cargo.lock`
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
- Required handoff: immutable published C03 SHA; exact ordered task ranges and
  clean task/holistic reviews; exact 15-name deferred inventory and proof no
  ignored body ran; browser-free layout API/binary/metadata evidence; synthetic
  schema, import, source-check, offline-check, pin/count update, historical
  authority, XML/report, and CLI evidence; final matrix; preservation digest;
  clean authority-remote readback; exact mapped generation-only preservation and
  artificial-linkage inventory for C04; and an explicit statement that C04 alone
  may add the generation capability/heavy edge, retire preservation, finalize
  guidance, and run the single authorized ignored body.
- Unresolved blocker: none. A required dependency/lockfile change, need to compile
  or mutate the preservation source, inability to preserve authored partition or
  read-only checking invariants, unsupported declared host behavior, or conflict
  with the reviewed specification stops implementation for plan correction.
