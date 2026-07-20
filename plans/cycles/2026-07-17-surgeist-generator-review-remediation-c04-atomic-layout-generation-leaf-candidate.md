# C04 Atomic Layout Generation And Leaf Candidate

## Header

- Cycle ID: `C04`
- Owning repository: `/Users/codex/Development/surgeist-generator`
- Status: `reviewed`
- Cycle base: `65ae2af6e3b1a2e9640decfa186dc2bf37ae4f7a`
- Published prerequisite: C03 at the cycle base, with local `main`, its authority
  tracking ref, and observed authority-remote `main` equal at readback.
- Reviewed specification:
  `plans/specs/2026-07-17-surgeist-generator-review-remediation.md` at
  `d2fbbedb033177731af5487d3498ba7f14b721d8`, normalized semantic SHA-256
  `faa4320f1e06ad9c003f2525fcf7171e387458eacc4ec3fd0d2d88f7c0e1eb71`,
  review `CLEAN`. C04 implements its remaining SR-01/SR-02, layout portions of
  SR-03.1/SR-03.2, all SR-03.3, remaining SR-04.5/SR-04.6, SR-05.1/SR-05.2/
  SR-05.4, generation portions of SR-06.1/SR-06.3/SR-06.4, SR-08, and SR-09.
- Reviewed sequence:
  `plans/sequences/2026-07-17-surgeist-generator-review-remediation.md` C04 at
  `faad9c1406b0cda68d9ce087a8cc3e06e6205360`, normalized semantic SHA-256
  `590c79d705cd9657a649b2a303e01437beda6facb538f08d85f86ae87392e3f6`,
  review `CLEAN`.
- Outcome: land one complete layout-generation vertical slice, retire the mapped
  preservation source and artificial linkage, finalize policy/docs/evidence,
  obtain clean task reviews and ordinary final evidence, then stop for explicit
  user authority before the one exhaustive diagnostic body; only its pass enables
  the final holistic review and publication sequence.

## Boundary

- Mutate, review, commit, test, and publish only this leaf. Root/sibling repos,
  corpora, Git checkouts, gitlinks, scripts, API artifacts, and production
  generated artifacts remain outside read, mutation, and test scope.
- C01-C03 durability, source, CSS, and browser-free layout contracts remain
  stable. C04 implements the reviewed specification rather than copying its
  design: SR-03 owns browser/environment/profile schemas and lifecycle; SR-04
  owns historical authority and publication modes; SR-05 owns exact API/CLI,
  dependency, license, and error contracts; SR-06 owns measurement/XML/report
  semantics; SR-08/SR-09 own final evidence and handoff.
- C04-T01 is atomic: dependency edge, `Generate` API/CLI, trusted-browser proof,
  authenticated supervisor/profile recovery, measurement/rendering, and clean/
  diagnostic/filtered publication land together. No dependency-only, backend-
  only, placeholder, provisional public request, or non-executable `Generate`
  commit may precede it.
- Exact additive public surface under `layout-browser` is
  `LayoutCommand::Generate`, `LayoutRequest::generate(CorpusLocation,
  RelativePath, Option<RelativePath>) -> Result<Self>`, `browser_path() ->
  Option<&RelativePath>`, and `filter() -> Option<&RelativePath>`. Constructors
  remain I/O-free and existing browser-free signatures remain unchanged.
- The CLI has only `generate`, `check-corpus`, `check-taffy-corpus`, and
  `import-taffy`. Generate requires one `--browser-path`, forbids
  `--source-root`, and permits one strict HTML-relative `--filter`; other modes
  forbid browser/filter and retain the reviewed source-root matrix. The binary
  remains no more than 15 physical lines.
- Exact optional edge:
  ```toml
  layout-browser = ["dep:chromiumoxide", "dep:futures", "dep:tokio", "dep:url"]
  chromiumoxide = { version = "=0.9.1", default-features = false, features = ["bytes"], optional = true }
  futures = { version = "=0.3.31", optional = true }
  tokio = { version = "=1.48.0", features = ["fs", "io-util", "macros", "process", "rt-multi-thread", "sync", "time"], optional = true }
  url = { version = "=2.5.7", optional = true }
  ```
  Other package/features/targets stay exact. Refresh `Cargo.lock` only with
  `cargo generate-lockfile --offline`; a cache/MSRV conflict stops work.
- Add only SR-05.1's exact `deny.toml`: confidence 0.8, its exact 15-license
  allowlist, and no exceptions/clarifications/private bypass. Use already
  installed `cargo-deny 0.19.4`, `cargo-audit 0.22.1`, and advisory DB only;
  missing tools, advisories, or unapproved licenses stop plan correction.
- No command downloads, installs, launches, or acquires Chromium; accesses a
  sibling/production corpus; or uses network except final canonical Git
  publication. Production accepts one explicit existing trusted executable
  below its manifest cache root. Tests use synthetic roots, injected adapters,
  and the crate-owned current test executable as fake browser/supervisor.
- Implement SR-03 exactly: fixed cleared environment, private authenticated
  capsule, executable identity/digest/version/switch revalidation, process-group
  supervision, durable profile journal/records/modes/order, terminalization,
  dead-only recovery, evidence preservation, and panic resumption. Recovery
  never signals. Every process/profile is terminal before artifact planning.
- Implement SR-06/SR-04 exactly: deterministic job ledger/batching/retry, corpus-
  owned HTML/helper/Taffy inputs, four variants, XML/report bytes/provenance,
  disposition/count/scope accounting, and `CleanFull`, `DiagnosticFull`, and
  `Filtered` behavior. Incomplete diagnostic/filter failure publishes nothing;
  successful filters preserve unselected bytes and write no report.
- Generate alone authenticates the live browser. Browser-free checks remain
  fully offline and treat browser provenance as historical attestation.
- `src/layout/legacy_generator.rs` remains unopened and byte-identical at SHA-256
  `d2f5ca87cea6b36826e9172e2d7ba7a99196c375e2ca53f8a84a075200e70a9f`
  through T01 and its clean review. T02's worker may verify/read only that exact
  blob to map and delete it; it may never compile, include, format, or test it.
  The fresh T02 task reviewer may independently verify and read only the exact
  digest-matched blob from T02's parent commit to compare the submitted map.
  Other reviewers use the map and clean T02 verdict without opening the source.
- Replace the exact C03 artificial references: `ArtifactPlan::{install,
  artifact_digest}`, `PublicationPolicy::DiagnosticFull`,
  `GenerationLease::acquire`, `InventoryEntry::{symlink,length,link_target,
  link_count}`, `InventoryPolicy::Private`, `ProtectedSource::verified`, and
  `private_front_doors_are_linked`. Each gets a genuine production caller or is
  removed; test/no-op reachability and lint suppression do not qualify.
- Supported mutation remains Apple-Silicon macOS. Default value/read code stays
  native/wasm portable; CSS-only builds do not activate browser dependencies;
  `#![forbid(unsafe_code)]` remains and no executable unsafe is allowed.

## Impacts

- API: additive layout `Generate` capability under `layout-browser`; existing
  shared, CSS, and browser-free layout signatures remain unchanged.
- Dependencies/features: add only the four reviewed optional browser-edge
  dependencies, their offline lock graph, and exact license policy; default and
  CSS-only builds remain isolated from that graph.
- Artifacts: commit no production corpus, browser, fixture, XML, report, or cache;
  tests use temporary synthetic roots. T02 deletes the mapped preservation copy.
- Docs/examples: T03 updates README, AGENTS, rustdoc, examples, and command
  guidance to the completed two-driver and trusted-browser contracts.
- MSRV/platform: retain Rust 1.97; mutation remains Apple-Silicon macOS while the
  default value/read library remains native/wasm portable.
- Root follow-up: no root gitlink, facade, API audit artifact, or sibling adoption
  changes; those remain later owning-repository work.
- Unsafe: none; executable owned Rust remains forbidden from using unsafe.

## Reusable Task-Clean Gate

Every T01-T03 worker runs this exact ordinary gate before handoff; every fresh
task reviewer evaluates its results and may rerun focused commands. No command
here executes an ignored body.

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

The list-only result must be C03's exact 15 names plus only
`layout::profile_tests::layout_profile_cleanup_every_prefix_recovers` (16 total).
Any other ignored test is a plan defect. No `--ignored` command lacking `--list`
runs before the separately authorized terminal diagnostic.

## Tasks

### C04-T01 — Complete atomic layout generation

- Files/area: `Cargo.toml`, `Cargo.lock`, new `deny.toml`; public layout front,
  CLI, manifest/case/check/report modules; new private browser, supervisor,
  profile, measurement/rendering, selection, generation/publication modules;
  genuine shared-core integration; focused unit/API/CLI/process tests. Do not
  inspect or modify the preservation source.
- Outcome: expose and execute the reviewed end-to-end generation path against
  explicit roots and injected owned adapters while preserving C03 behavior.
- RED evidence first: failing compile/API/CLI/metadata tests for the exact public
  edge; browser/config/capsule/profile/process lifecycle matrices; the 14 named
  ordinary SR-03.3 profile tests and one ignored prefix test; measurement/four-
  variant/XML/report goldens; retry/ledger/filter/historical publication tests;
  and panic/error/terminalization tests. Record focused failing predicates before
  production edits; dependency resolution is offline only.
- Acceptance:
  - exact feature/dependency/license policy and API/CLI/error boundaries pass;
    constructors are I/O-free; no public heavy/domain type, extra target/mode,
    hidden environment option, or acquisition path exists;
  - SR-03 browser/cache/corpus disjointness, identity/digest/version/switch,
    capsule, environment, journal, group, timeout, panic, recovery, drift, and
    evidence-preservation matrices pass without launching Chromium;
  - SR-06 measurement/HTML/four-variant/XML/report/retry/accounting behavior and
    SR-04 clean/diagnostic/filtered historical state sequences pass; all browser-
    free commands and CSS/default isolation remain clean;
  - every retained core private item used by generation/profile has a genuine
    production caller; obsolete items are deferred to T02 removal.
- Additional commands before the reusable gate:
  - `cargo generate-lockfile --offline`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser --lib layout_profile_`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser --lib layout_browser_`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser --lib layout_generate_`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser --lib layout_filter_`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser --test public_api`
  - `cargo test --locked --offline -p surgeist-generator --features layout-browser --test layout_cli`
  - `command -v cargo-deny && cargo deny --all-features --locked --offline check licenses`
  - `command -v cargo-audit && cargo audit --no-fetch --stale`
- Dependencies: published C03 base and present offline archives/tools.
- Intended commit: `feat(layout): generate atomically with chromium`.

### C04-T02 — Map and retire preservation and artificial linkage

- Files/area: exact preservation blob for one bounded map/read/delete; compiled
  layout/core files and focused tests for any unmapped responsibility;
  `src/core/mod.rs` artificial link and obsolete private helpers.
- Outcome: map each retained responsibility to compiled code/named tests or a
  corpus-owned input, map rejected legacy mechanisms to reviewed replacements,
  delete the source, and remove every artificial reference without changing T01's
  public/domain contract.
- RED evidence: verify the 4,626-line digest; `test ! -e` initially fails; build
  the task-result responsibility table covering schema, browser version/launch,
  batching/retry, HTML/assets, DOM/measurement/variants, XML/provenance, reports/
  dispositions/scopes, filtering, cleanup, and publication. Add a focused failing
  test before correcting any unmapped retained behavior.
- Acceptance:
  - worker supplies the pre-deletion digest and complete mapping; the reviewer
    independently compares it to the exact parent blob under the narrow exception
    above;
  - source is deleted, never copied/renamed/generated/included; no second
    implementation remains;
  - artificial caller and identifier hook are gone; handed-off items have real
    production callers or are removed, with no no-op/test/lint substitute;
  - `test ! -e src/layout/legacy_generator.rs` and the reusable gate pass with
    the exact list-only 16 inventory.
- Additional commands, in order before the reusable gate:
  - before opening/deletion,
    `test "$(wc -l < src/layout/legacy_generator.rs | tr -d ' ')" = 4626`
  - before opening/deletion,
    `test "$(shasum -a 256 src/layout/legacy_generator.rs | cut -d ' ' -f 1)" = d2f5ca87cea6b36826e9172e2d7ba7a99196c375e2ca53f8a84a075200e70a9f`
  - after the responsibility map and deletion,
    `test ! -e src/layout/legacy_generator.rs`
  - then execute the reusable task-clean gate.
- Dependencies: T01 and its fresh task review are clean.
- Intended commit: `refactor(layout): retire preserved generator`.

### C04-T03 — Finalize guidance, policy evidence, and handoff readiness

- Files/area: `README.md`, `AGENTS.md`, rustdoc/examples, focused public/process
  tests, policy/metadata evidence, and deletion of the obsolete C01-C03 cycle-plan
  paths. Retain `.gitkeep`, baseline review, reviewed spec/sequence, and C04 plan.
  No root/sibling artifact is generated.
- Outcome: accurately document the completed two-driver crate and close stale-
  guidance, policy, target, tautological-test, and terminal planning-tree findings.
- RED evidence: source/doc assertions first identify stale scaffold/migration and
  incomplete-command text; add or strengthen executable rustdoc/process evidence
  for trust, acquisition-free operation, offline attestation, profile recovery,
  Taffy adoption, and exact API/CLI boundaries before replacing prose. The final
  one-cycle-plan/count predicates initially fail because C01-C03 remain tracked.
- Acceptance:
  - docs state exact features/binaries/roots/ownership, mutable pins/counts,
    normal-build independence, platform support, trusted-browser containment
    limits, fixed environment, orphan recovery/operator action, offline checking,
    and Taffy adoption; no scaffold/migration/acquisition overclaim remains;
  - exact dependency isolation, license/advisory/MSRV/wasm/metadata evidence,
    preservation deletion, unsafe absence, and 16-name list-only inventory are
    recorded in task evidence; no evidence document is committed;
  - obsolete C01-C03 cycle plans are deleted; tracked `plans/` contains exactly
    `.gitkeep`, the baseline review, reviewed specification, reviewed sequence,
    and this C04 plan, with no transcript/verdict/evidence/handoff document;
  - reusable gate plus `cargo deny --all-features --locked --offline check licenses`,
    `cargo audit --no-fetch --stale`, and `test ! -e src/layout/legacy_generator.rs`
    pass; lock regeneration leaves the committed graph/worktree unchanged;
  - `test "$(git ls-files plans/cycles | wc -l | tr -d ' ')" = 1`
  - `test "$(git ls-files plans | wc -l | tr -d ' ')" = 5`
  - `git ls-files --error-unmatch plans/.gitkeep plans/2026-07-17-crate-baseline-review.md plans/specs/2026-07-17-surgeist-generator-review-remediation.md plans/sequences/2026-07-17-surgeist-generator-review-remediation.md plans/cycles/2026-07-17-surgeist-generator-review-remediation-c04-atomic-layout-generation-leaf-candidate.md`
  - `test ! -e plans/cycles/2026-07-17-surgeist-generator-review-remediation-c01-rooted-core-production-recovery.md`
  - `test ! -e plans/cycles/2026-07-17-surgeist-generator-review-remediation-c02-protected-source-css-vertical-slice.md`
  - `test ! -e plans/cycles/2026-07-17-surgeist-generator-review-remediation-c03-browser-free-layout-corpus-interface.md`
- Dependencies: T02 and its fresh task review are clean.
- Intended commit: `docs: finalize generator guidance and policy`.

## Completion

- Follow `$surgeist-agent` canonical-gate.md for delegated task/fix review,
  administrative status transitions, final checks, and holistic review. After all
  task ranges are clean and status is `complete`, run the reusable gate plus lock
  generation, license, advisory, preservation/planning-tree absence, and the unsafe
  scan below. C04's eventual holistic packet additionally includes every ordered
  T01-T03 task/fix range, preservation map/verdict, policy evidence, exact ignored
  inventory, and the authorized exhaustive result.
- Failure-propagating owned-Rust unsafe scan (post-deletion scope):
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
      if rg -n --pcre2 "$pattern" -- "$rust_path"; then exit 1
      else scan_exit=$?; test "$scan_exit" -eq 1 || exit "$scan_exit"; fi
    done <"$manifest"
  )
  ```
- When every task review and ordinary final command is clean, verify unchanged
  worktree/`main`, rerun only the all-target list command, compare exactly 16
  names, and block the active goal. The holistic review has not started. Do not
  start the expensive command while waiting.
- Only after the user prepares the machine and explicitly authorizes it, execute
  exactly once and sequentially:
  ```sh
  cargo test --locked --offline -p surgeist-generator --all-features --lib -- --ignored --test-threads=1
  ```
  Failure stops publication with no automatic rerun. A pass freezes the candidate;
  any later source/test/plan/lock/doc commit invalidates it and requires fresh
  affected review plus fresh user authority.
- Only after that pass, obtain the canonical fresh holistic review over the exact
  cycle range and complete final evidence. Any finding requiring a commit follows
  the canonical reopen/fix/task-review/status/final-check loop, then blocks again
  for fresh user authority and one replacement exhaustive run before a replacement
  holistic review. After a clean holistic verdict, execute canonical-gate.md
  “Automated landing and publication” without variation and record its authority-
  upstream, immutable candidate, reconciliation/re-review, leased push, ancestry,
  readback, equality, cleanliness, and cleanup evidence in the C04 handoff.
- Handoff records immutable candidate/published tips; reviewed planning SHAs;
  ordered task/fix ranges and verdicts; API/feature/dependency/policy/platform
  evidence; trust/profile/generation/publication behavior; preservation map and
  deletion; artificial-link closure; exact 16-name list and one authorized pass;
  unsafe absence; canonical remote readback; and that root/sibling adoption,
  gitlink, and API artifacts remain future owning-repository work.
- Genuine blockers: offline/tool/policy/MSRV failure, Chromium launch/acquisition
  need, unsafe need, profile terminalization failure, unclassifiable preservation
  responsibility, unsupported declared host behavior, invariant conflict, or the
  deliberate user-authorization gate before the one exhaustive body.
