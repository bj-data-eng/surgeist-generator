# C01 Shared Corpus Drivers

## Header

- Cycle ID: `C01`
- Owning repository: `/Users/codex/Development/surgeist-generator`
- Status: `in_progress`
- Cycle base: `a8b8c6d1cbfe0480ca11a5d5f530ae5b06572412`
- Reviewed specification: `plans/specs/2026-07-15-surgeist-generator-shared-corpus-drivers.md`, sections `SG-01` through `SG-14`, semantic SHA-256 `ab25f8edbc99d3ae9108437772e5dcc41bcf9740cad035483dca9e347031aaa0`, commit `d0692c55a8835391e14203e7c39f4a54ef54481e`, review `CLEAN`
- Implementation sequence: not required; this is one cycle in one repository
- Bounded outcome: preserve an audited copy of the layout generator, then deliver the shared generation core and thin feature-gated layout and CSS interfaces with synthetic offline verification.

## Boundary

- Mutate, commit, land, and publish only this repository. The layout, CSS, and root repositories are read-only evidence.
- Do not edit, format, test, fetch, commit, or push `surgeist-layout`; do not edit or test `surgeist-css`; preserve its existing untracked planning work.
- Copy only layout source lines 1–4626 from commit `92054de23b7c4d431556ef7e42e2226dd1788f1f` to the transient audited destination. Do not copy its inline tests, wrapper, helpers, corpus, manifests, reports, or expectations.
- Do not run either real corpus, browser acquisition, or Taffy acquisition path. Tests use synthetic temporary corpora, injected browser boundaries, and local temporary Git repositories.
- Root workspace/gitlink/API integration, sibling script rewiring, real-corpus migration, and removal of the layout-owned generator are handoffs.
- Use only already-present dependencies and `--locked --offline` Cargo verification. Keep Rust 1.97 and edition 2024.
- The reviewed specification owns all model, schema, CLI, error, compatibility, and rollback decisions; this plan does not vary them.

## Impacts

- Public API: additive shared semantic types and feature modules; preserve `CRATE_NAME`.
- Dependencies/features: small exact-version default core; optional browser stack only in `layout-browser`; CSS driver only in `css-corpus`; `default = []`; track `Cargo.lock`.
- Generated artifacts: no owned corpus output is committed; only synthetic test output is created in temporary directories.
- Documentation/examples: update `README.md` and repository discovery in `AGENTS.md`; no runnable real-corpus example.
- MSRV: unchanged at 1.97.
- Root follow-up: required after leaf publication; no root change in this cycle.
- Unsafe: owned Rust remains free of executable `unsafe` and keeps compiler enforcement.

## Tasks

### C01-T01 — Preserve the verified layout production prefix

- Files/area: add only `src/layout/legacy_generator.rs`.
- Outcome: create one byte-for-byte, deliberately unreferenced copy of source lines 1–4626, including the final newline, before any transformation.
- RED evidence: `test ! -e src/layout/legacy_generator.rs`; independently hash the authoritative prefix before copying.
- Acceptance: destination SHA-256 is `d2f5ca87cea6b36826e9172e2d7ba7a99196c375e2ca53f8a84a075200e70a9f`; byte comparison passes; the task commit changes only that path; layout status remains clean; evidence records source/parent/task SHAs and paths.
- Commands:
  - `git -C /Users/codex/Development/surgeist-layout show 92054de23b7c4d431556ef7e42e2226dd1788f1f:tests/bin/surgeist-layout-generate/generator.rs | sed -n '1,4626p' | shasum -a 256`
  - `shasum -a 256 src/layout/legacy_generator.rs`
  - `cmp src/layout/legacy_generator.rs <(git -C /Users/codex/Development/surgeist-layout show 92054de23b7c4d431556ef7e42e2226dd1788f1f:tests/bin/surgeist-layout-generate/generator.rs | sed -n '1,4626p')`
  - `git diff-tree --no-commit-id --name-status -r HEAD`
  - `git -C /Users/codex/Development/surgeist-layout status --short`
- Dependencies: reviewed specification and reviewed cycle plan only.
- Intended commit: `extract(layout): preserve generator source`.

### C01-T02 — Implement shared validation and provenance contracts

- Files/area: `Cargo.toml`, `Cargo.lock`, `.gitignore`, `src/lib.rs`, `src/error.rs`, and `src/core/{mod,case,corpus,hash,manifest,report,source}.rs` with focused tests.
- Outcome: expose the SG-03 through SG-08 and SG-12 shared API for explicit corpus locations, strict relative paths, run scopes, versioned manifests, exact clean Git pins, sorted inventory, dispositions/reasons, hashes, provenance, and reports.
- RED evidence: add named contract tests first and record failure before implementing `RelativePath`, `CorpusLocation`, `PinnedSource`, `CaseDisposition`, and hash/report validation.
- Acceptance: default core has no optional domain dependencies; all invalid external input returns semantic errors; path/source/inventory/disposition/hash/report tests cover SG-13.2; lockfile is tracked and exact cached versions are used.
- Commands:
  - `cargo test --locked --offline -p surgeist-generator --no-default-features strict_paths_reject_invalid_components`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features pinned_source_requires_exact_clean_revision`
  - `cargo check --locked --offline -p surgeist-generator --no-default-features`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features`
  - `cargo clippy --locked --offline -p surgeist-generator --no-default-features --all-targets -- -F unsafe-code -D warnings`
  - `cargo fmt --check`
- Dependencies: `C01-T01`; do not transform or remove the transient copy yet.
- Intended commit: `feat(core): add corpus generation contracts`.

### C01-T03 — Implement transactional generation lifecycle

- Files/area: `src/core/{artifact,lease}.rs`, related reexports, report/scope integration, and focused tests.
- Outcome: provide corpus-keyed exclusive leases and deterministic staged artifact synchronization with full-only report/stale mutation, filtered diagnostic isolation, rollback, and cleanup per SG-09 and SG-10.
- RED evidence: add contention, filtered-write prohibition, stale-removal, and injected rollback tests first and record their failure.
- Acceptance: full success installs complete output and prunes stale files; filtered work cannot replace reports or prune; every injected pre-install/install failure restores prior artifacts; lease metadata and drop cleanup are deterministic.
- Commands:
  - `cargo test --locked --offline -p surgeist-generator --no-default-features artifact_transaction_restores_prior_tree`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features filtered_scope_cannot_publish_or_prune`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features generation_lease_contends_by_corpus`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features`
  - `cargo clippy --locked --offline -p surgeist-generator --no-default-features --all-targets -- -F unsafe-code -D warnings`
  - `cargo fmt --check`
- Dependencies: `C01-T02`.
- Intended commit: `feat(core): add transactional generation lifecycle`.

### C01-T04 — Deliver the extracted layout browser driver

- Files/area: layout optional dependencies/features and binary target in `Cargo.toml`/`Cargo.lock`; `src/layout/{mod,browser,manifest,xml}.rs`; `src/bin/surgeist-layout-generate.rs`; layout-focused tests; remove `src/layout/legacy_generator.rs` only after behavior is represented.
- Outcome: adapt the preserved production logic to the explicit-root schema-2 layout contract, runtime corpus helpers, manifest-derived Taffy pin, injected browser boundaries, existing XML/provenance/report formats, and a plumbing-only binary.
- RED evidence: first add named schema, explicit-CLI, helper-hash, representative XML, and injected-browser tests that fail against the core-only state.
- Acceptance: `layout-browser` alone exposes every SG-11.1 command; binary is at most 15 nonblank logical lines; no `include_str!` corpus helper or hard-coded Taffy revision remains; no browser/corpus acquisition runs; synthetic schema/XML/report/drift tests pass; transient copy is removed after extraction.
- Commands:
  - `cargo test --locked --offline -p surgeist-generator --no-default-features --features layout-browser layout_cli_requires_explicit_roots`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features --features layout-browser layout_xml_preserves_schema_two_shape`
  - `cargo check --locked --offline -p surgeist-generator --no-default-features --features layout-browser --all-targets`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features --features layout-browser --all-targets`
  - `cargo clippy --locked --offline -p surgeist-generator --no-default-features --features layout-browser --all-targets -- -F unsafe-code -D warnings`
  - `cargo fmt --check`
  - `git -C /Users/codex/Development/surgeist-layout status --short`
- Dependencies: `C01-T03` and the audited `C01-T01` copy evidence.
- Intended commit: `feat(layout): extract browser corpus driver`.

### C01-T05 — Deliver neutral CSS corpus generation and operator docs

- Files/area: CSS feature/binary target; `src/css/{mod,manifest,neutral}.rs`; `src/bin/surgeist-css-generate.rs`; CSS-focused tests; `README.md`; `AGENTS.md`.
- Outcome: implement the SG-05.3 schema, no-network exact-pin JSON import, neutral CSSTree flattening, full/filtered generation and offline checking, a plumbing-only binary, and complete ownership/feature/CLI/handoff documentation.
- RED evidence: first add named exact-pin import, neutral omission, full/filtered, and drift tests that fail before the CSS module exists.
- Acceptance: deterministic JSON-only import and schema-1 expectation/report generation match SG-07; ASTs and diagnostic details are omitted; filtered runs never publish/prune; offline checks detect all named drift; binary is at most 15 nonblank logical lines; docs accurately describe both drivers and non-ownership of corpora.
- Commands:
  - `cargo test --locked --offline -p surgeist-generator --no-default-features --features css-corpus css_import_requires_exact_clean_pin`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features --features css-corpus neutral_expectations_omit_engine_specific_details`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features --features css-corpus filtered_css_generation_does_not_publish`
  - `cargo check --locked --offline -p surgeist-generator --no-default-features --features css-corpus --all-targets`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features --features css-corpus --all-targets`
  - `cargo clippy --locked --offline -p surgeist-generator --no-default-features --features css-corpus --all-targets -- -F unsafe-code -D warnings`
  - `cargo check --locked --offline -p surgeist-generator --no-default-features --features layout-browser,css-corpus --all-targets`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features --features layout-browser,css-corpus --all-targets`
  - `cargo clippy --locked --offline -p surgeist-generator --no-default-features --features layout-browser,css-corpus --all-targets -- -F unsafe-code -D warnings`
  - `cargo fmt --check`
- Dependencies: `C01-T04`.
- Intended commit: `feat(css): add neutral corpus driver`.

## Completion

- Acceptance: all five tasks have exact reviewed commit spans; the one-file copy proof is retained in task evidence; shared/default, layout-only, CSS-only, and combined behavior satisfies SG-01; worktree and read-only sibling statuses are unchanged/clean as originally observed.
- Final commands: run every command in SG-13.3 exactly, then run `git ls-files -co --exclude-standard -- '*.rs' | LC_ALL=C sort -u` as the owned-Rust manifest and run the canonical unsafe regex from the Surgeist gate over every listed file, expecting no executable match.
- Handoff: publish the reviewed descendant to generator `origin/main`, verify local/tracking/remote SHA equality, and report the candidate contract for separate root, layout, and CSS owning cycles listed in SG-14.
- Genuine unresolved blocker: none. A changed authority remote, unavailable cached dependency, or sibling mutation stops the affected action for coordinator adjudication rather than widening scope.
