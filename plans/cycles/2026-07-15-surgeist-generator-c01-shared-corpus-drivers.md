# C01 Shared Corpus Drivers

## Header

- Cycle ID: `C01`
- Owning repository: `/Users/codex/Development/surgeist-generator`
- Status: `in_progress`; final specification is independently clean and this
  revised cycle plan is the remaining planning gate.
- Cycle base: `a8b8c6d1cbfe0480ca11a5d5f530ae5b06572412`
- Reviewed semantic specification pair:
  - `plans/specs/2026-07-15-surgeist-generator-shared-corpus-drivers.md`,
    SHA-256 `a672526df3d703419cfb0adf971405c6780b67dd78ceaece5ab3fc813cb22adc`;
  - `plans/specs/2026-07-15-surgeist-generator-focused-verification.md`,
    SHA-256 `a8db3583cca978eaa6154977bb12e4e7601076b1a87b144df21bf8c101c2ed1f`;
  - exact commit `d3b3f8a783d7adfa2e0584af4a1f2f999c0bd0d4`, complete-pair review `CLEAN`.
- Bounded outcome: retain the audited copy evidence, reconcile the current
  baseline to the final shared contract, deliver thin feature-gated layout and
  CSS binaries, pass the complete synthetic locked/offline matrix, publish the
  reviewed leaf candidate, and hand it to the three owning repositories.

## Boundary

- Mutate, commit, land, and publish only this repository. Treat root,
  `surgeist-layout`, and `surgeist-css` as read-only evidence.
- Do not edit, format, test, fetch, commit, or push `surgeist-layout`; do not edit
  or test `surgeist-css`. Do not run either sibling's scripts or corpus.
- Do not run a real browser, browser download, Taffy remote acquisition, or real
  corpus generation path. Focused tests use synthetic temporary corpora,
  injected browser/download boundaries, and local temporary Git repositories.
- Use only installed tooling and locally cached sources. Cargo resolution,
  checks, tests, lint, license inventory, and advisory audit are locked/offline
  or explicitly no-fetch. No target, package, tool, browser, or corpus is
  installed or acquired.
- Keep Rust 1.97, edition 2024, `default = []`, and owned Rust free of executable
  `unsafe`. Heavy dependencies remain isolated behind `layout-browser`;
  `css-corpus` adds no dependency.
- Consumer manifests, fixtures, generated expectations/XML, reports, and tests
  remain owned by layout/CSS. Root workspace/gitlink/API artifacts, consumer
  rewiring, real-corpus validation, and removal of layout's original generator
  are handoffs after this leaf publishes.
- The reviewed semantic pair owns every API, schema, lifecycle, recovery, error,
  target, CLI, and verification decision. A task may reorganize private modules
  but may not weaken or vary those contracts.

## Existing Evidence And Reconciliation

- `C01-T01` is complete at `d72fd9c`: the unreferenced
  `src/layout/legacy_generator.rs` is the reviewed byte-for-byte copy of layout
  commit `92054de23b7c4d431556ef7e42e2226dd1788f1f`, production prefix lines
  1–4626, SHA-256
  `d2f5ca87cea6b36826e9172e2d7ba7a99196c375e2ca53f8a84a075200e70a9f`.
  Its one-path copy proof and independent review are retained.
- `C01-T02` commits `5521d54` and `d2980f7` are a previously reviewed shared-core
  baseline. Later specification revisions intentionally supersede parts of that
  behavior; they are implementation input, not final acceptance evidence.
- `C01-T03` commits `b70dea3` and `d32e9f6` are an unaccepted lifecycle baseline.
  Its exact-span review was not clean. Tasks T06 and T07 replace or reconcile all
  affected behavior; no old review is reused.
- No implementation resumes until this revised plan has a fresh independent
  `CLEAN` review. Each implementation task uses a fresh worker, a task-scoped RED
  observation before production edits, an exact commit span, and a fresh
  independent review. Final acceptance additionally requires an independent
  holistic review of the complete descendant and the full SG-13.3 evidence.

## Impacts

- Public API: additive exact SG-03.3 shared front door plus feature-gated concrete
  layout/CSS modules; `CRATE_NAME` remains unchanged.
- Dependencies: exact locally cached versions and feature edges in SG-03.2;
  `Cargo.lock` remains tracked and is mechanically refreshed offline.
- Generated artifacts: none are committed. Tests write only temporary synthetic
  artifacts and prove deterministic cleanup.
- Platform: mutation support is exactly `aarch64-apple-darwin`; the installed
  `wasm32-unknown-unknown` target verifies the nonmutation core branch.
- Documentation: `README.md` and this repository's `AGENTS.md` describe the
  resulting modules, binaries, ownership split, capability boundary, and offline
  commands without importing another workflow.

## Tasks

### C01-T04 — Reconcile the small shared contract and offline dependency graph

- Files/area: `Cargo.toml`, mechanically generated `Cargo.lock`, `src/lib.rs`,
  `src/error.rs`, `src/core/{mod,case,corpus,hash,manifest,report}.rs`, and focused
  public/value tests.
- Outcome: implement the exact public reexports/signatures/traits/Serde wires,
  strict scalar/path/location/manifest validation, disposition/reason accounting,
  deterministic hashes, provenance, reports, error kinds, target gates, and the
  SG-03.2 feature/dependency matrix. Preserve the legacy copy untouched.
- RED evidence: first add the exact public API/Serde golden and checked-overflow,
  path/location, disposition, report, feature-gating, and nonmutation-target tests
  and record their targeted failure against the baseline.
- Acceptance: root API is exact; core parses no ambiguous/unknown input; all
  count/hash/provenance normalization is deterministic; optional dependencies
  cannot compile without their feature; `cargo generate-lockfile --offline`
  resolves only the pinned cached graph; license inventory covers all features;
  no owned Rust contains executable `unsafe`.
- Focused commands:
  - `cargo generate-lockfile --offline`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features public_api`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features serde_contract`
  - `cargo check --locked --offline -p surgeist-generator --target wasm32-unknown-unknown --no-default-features --lib`
  - `cargo check --locked --offline -p surgeist-generator --no-default-features`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features`
  - `cargo clippy --locked --offline -p surgeist-generator --no-default-features --all-targets -- -F unsafe-code -D warnings`
  - `cargo fmt --check`
  - `cargo deny --all-features --locked --offline list --format tsv --layout license`
- Dependencies: clean revised cycle plan.
- Intended commit: `feat(core): reconcile shared generation contracts`.

### C01-T05 — Close pinned Git source verification and immutable snapshots

- Files/area: `src/core/source.rs`, private source-support modules, and synthetic
  Git/source focused tests.
- Outcome: implement the exact installed-Apple-Git trust boundary, inert cleared
  environment, common plus enabled-worktree configuration union, raw clean-tree
  proof, recursive alternate protection, exact commit/blob snapshotting, closing
  config/identity revalidation, and deterministic source errors in SG-06.
- RED evidence: add table-driven ordinary/linked-worktree, `config.worktree`,
  filter sentinel, missing-promisor-object, raw-byte, alternate, replacement-ref,
  executable-unit, and snapshot-stability tests and record targeted failures.
- Acceptance: verification is read-only and network-free; no configured helper or
  sentinel executes; a full pin never accepts a prefix/tag/tree/blob; locally
  missing data fails without lazy fetch; snapshot bytes/object IDs remain bound
  to the verified commit; source and protected Git trees are byte/identity clean
  after every test.
- Focused commands:
  - `cargo test --locked --offline -p surgeist-generator --no-default-features source::tests`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features git_config_worktree`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features promisor_missing_object`
  - `cargo check --locked --offline -p surgeist-generator --no-default-features`
  - `cargo clippy --locked --offline -p surgeist-generator --no-default-features --all-targets -- -F unsafe-code -D warnings`
  - `cargo fmt --check`
- Dependencies: `C01-T04`.
- Intended commit: `feat(core): verify immutable pinned sources`.

### C01-T06 — Implement rooted coordination, capability probes, and leases

- Files/area: `src/core/{corpus,lease}.rs`, new private rooted-filesystem and
  coordination modules, and crash/race focused tests.
- Outcome: bind every mutation authority to held descriptors and canonical
  identities; implement exact-name/alias/mount probes, journaled lock bootstrap,
  cache-target and corpus gate ordering, domain/key locks, owner audit, shared
  read-only checks, contention, cleanup precedence, and supported-target behavior
  from SG-04, SG-09, SG-10, and SG-12.
- RED evidence: add first-acquisition races, dead/lost claimant transitions,
  case/normalization aliases, mount/identity swaps, two-owner/same-corpus and
  two-corpus/same-cache contention, opposite-domain residue, and read-only lock
  tests before replacing the baseline.
- Acceptance: no public mutation plan/capability exists; no pathname-only write
  authority exists; probes are cleaned or durably reported; unsupported results
  are distinguished from unresolved `ArtifactTransaction`; lock publication is
  never partial; check paths create/recover nothing; every injected race has one
  deterministic owner or fails closed.
- Focused commands:
  - `cargo test --locked --offline -p surgeist-generator --no-default-features coordination`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features generation_lease`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features capability_probe`
  - `cargo check --locked --offline -p surgeist-generator --no-default-features`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features`
  - `cargo clippy --locked --offline -p surgeist-generator --no-default-features --all-targets -- -F unsafe-code -D warnings`
  - `cargo fmt --check`
- Dependencies: `C01-T05`.
- Intended commit: `feat(core): add rooted generation coordination`.

### C01-T07 — Implement durable root transactions and recovery

- Files/area: `src/core/artifact.rs`, private transaction/inventory/cleanup and
  process-resource support, related report/scope integration, and exhaustive
  death-injection tests.
- Outcome: implement the intent-before-stage protocol, canonical old/new
  sidecars, internal pre-intent cleanup receipt, one-root `RENAME_EXCL`/swap
  commit, abort/commit/cleanup receipts, descriptor-bound stale removal, full
  versus filtered/diagnostic policy, and cache/corpus recovery state machines.
- RED evidence: add every SG-09 prefix/marker/swap/cleanup crash point, partial
  old-sidecar-without-intent corruption, directory-link-count evolution,
  replacement/hard-link/mount disputes, stale/full/filtered outcomes, and process
  slot nonterminal barriers; record failures before production edits.
- Acceptance: pre-commit leaves the complete old root; post-commit leaves the
  complete new root plus resumable evidence; no mixed tree or rollback-after-swap
  path exists; cleanup removes only receipt/sidecar-listed identities; filtered
  and diagnostic work cannot authorize forbidden pruning/report changes; all
  error collisions obey SG-12.
- Focused commands:
  - `cargo test --locked --offline -p surgeist-generator --no-default-features transaction`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features recovery`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features filtered_scope`
  - `cargo check --locked --offline -p surgeist-generator --no-default-features`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features`
  - `cargo clippy --locked --offline -p surgeist-generator --no-default-features --all-targets -- -F unsafe-code -D warnings`
  - `cargo fmt --check`
- Dependencies: `C01-T06`.
- Intended commit: `feat(core): add recoverable root transactions`.

### C01-T08 — Extract the layout browser and Taffy driver

- Files/area: `src/layout/{mod,manifest,source,browser,xml}.rs`, private cache/run
  support as needed, `src/bin/surgeist-layout-generate.rs`, layout synthetic
  tests, feature target wiring, and final removal of
  `src/layout/legacy_generator.rs` only after represented behavior is reviewed.
- Outcome: adapt the copied production logic to explicit owner/corpus roots,
  schema-2 manifest/helper contracts, pinned bare Taffy acquisition/import,
  pinned Chromium download/extraction/cache reuse, fully owned process groups,
  Tokio/browser/page/profile lifecycle, deterministic measurement/XML/report
  behavior, full/filtered diagnostics, and offline checks in SG-05 and SG-11.1.
- RED evidence: add exact CLI/manifest/helper/Taffy/cache/archive/Chrome golden,
  browser-supervisor transition-panic, profile/process cleanup, representative XML
  and report, under-lease recollection race, and offline drift tests before
  adapting/removing the legacy copy.
- Acceptance: `layout-browser` alone exposes the exact concrete API and binary;
  binary plumbing is at most 15 nonblank logical lines; no hard-coded corpus,
  helper, or Taffy revision remains; no Chromium/Tokio handle, process/group,
  pipe, task, page, or profile can detach; every test is synthetic and invokes no
  network/real browser/real corpus; copied behavior is represented before the
  transient generator copy is removed.
- Focused commands:
  - `cargo test --locked --offline -p surgeist-generator --no-default-features --features layout-browser layout::tests`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features --features layout-browser browser_supervisor`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features --features layout-browser layout_xml`
  - `cargo check --locked --offline -p surgeist-generator --no-default-features --features layout-browser --all-targets`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features --features layout-browser --all-targets`
  - `cargo clippy --locked --offline -p surgeist-generator --no-default-features --features layout-browser --all-targets -- -F unsafe-code -D warnings`
  - `cargo fmt --check`
  - `/usr/bin/git -C /Users/codex/Development/surgeist-layout status --short`
- Dependencies: `C01-T07` and retained `C01-T01` copy evidence.
- Intended commit: `feat(layout): extract browser corpus driver`.

### C01-T09 — Deliver CSS import provenance and neutral expectations

- Files/area: `src/css/{mod,manifest,neutral}.rs`,
  `src/bin/surgeist-css-generate.rs`, CSS synthetic tests, and feature target
  wiring.
- Outcome: implement schema-1 source verification/import, atomic canonical
  `.surgeist-source.json` pin/blob/digest inventory, strict CSSTree fixture
  interpretation, dispositions, neutral expectations, full/filtered generation,
  reports, and fully offline verification from SG-05.3, SG-07, and SG-11.2.
- RED evidence: add exact sidecar/neutral JSON goldens, linked-checkout import,
  stale-pin and edited-import regeneration rejection, duplicate-member and
  zero-case cases, full/filtered transaction policy, and every offline drift case
  before production implementation.
- Acceptance: `css-corpus` adds no dependency and exposes the exact concrete API
  and at-most-15-line binary; import never acquires or writes its checkout;
  generation cannot relabel/rewrite imported provenance; ASTs/error details are
  omitted; full/filtered ordering and counts are deterministic; checking requires
  no original checkout; no test reads or executes the real CSS repository.
- Focused commands:
  - `cargo test --locked --offline -p surgeist-generator --no-default-features --features css-corpus css::tests`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features --features css-corpus css_import_provenance`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features --features css-corpus neutral_expectation`
  - `cargo check --locked --offline -p surgeist-generator --no-default-features --features css-corpus --all-targets`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features --features css-corpus --all-targets`
  - `cargo clippy --locked --offline -p surgeist-generator --no-default-features --features css-corpus --all-targets -- -F unsafe-code -D warnings`
  - `cargo fmt --check`
- Dependencies: `C01-T07`; execution follows T08 so each exact commit span is
  linear and independently reviewable.
- Intended commit: `feat(css): add neutral corpus driver`.

### C01-T10 — Close documentation, integration, and final evidence

- Files/area: `README.md`, `AGENTS.md`, cross-feature/public integration tests,
  and only minimal reviewed glue needed to expose already implemented contracts.
- Outcome: document the exact feature/CLI/ownership/capability/offline model,
  prove default/layout/CSS/combined coexistence, retain no transient copied file
  or generated corpus artifact, and assemble complete reproducible evidence.
- RED evidence: add documentation/target/binary-thinness/feature-isolation and
  combined-build assertions before final glue; any substantive behavior defect
  opens a separately planned repair task and exact review rather than being
  hidden in this closeout.
- Acceptance: README/AGENTS match SG-14 and repository reality; every SG-13.3
  command passes exactly; the owned-Rust manifest and canonical unsafe scan are
  clean; license output is independently adjudicated; stale offline advisory DB
  revision is disclosed with no finding; sibling statuses match the original
  read-only observation; a holistic reviewer returns `CLEAN` over the complete
  candidate and evidence.
- Final commands: run SG-13.3 verbatim, then
  `git ls-files -co --exclude-standard -- '*.rs' | LC_ALL=C sort -u` and the
  Surgeist canonical executable-unsafe scan over that exact manifest. Record
  `rustc -vV`, exact tool versions, lockfile hash, license output, audit database
  revision, and local commit SHA. Do not run either binary's acquisition path.
- Dependencies: `C01-T08` and `C01-T09` with clean task reviews.
- Intended commit: `docs(generator): close shared driver handoff`.

## Completion And Publication

- Required reviews: clean exact-span reviews for T04 through T10, preserved clean
  T01 copy evidence, and one final independent holistic review of the full
  descendant and locked/offline evidence. Historical T02/T03 reviews are not
  substituted for these gates.
- Publication: after all gates, publish the reviewed descendant directly to this
  repository's authorized `origin/main`; verify local `HEAD`, tracking ref, and
  independently queried remote `main` are the same full SHA before reporting
  success. Do not publish or mutate a sibling/root repository.
- Handoff: report the published SHA and require separate root, layout, and CSS
  owning cycles for gitlink/topology integration, consumer scripts/manifests,
  real corpus/browser compatibility, root-owned API audit artifacts, and eventual
  removal of layout's original generator. Linux/other mutation support remains a
  separately evidenced future cycle.
- Stop conditions: changed authority remote, unavailable cached dependency,
  missing installed target/tool, non-MIT-compatible resolved license, advisory
  finding, sibling mutation, or an unresolved safety/review finding returns to
  coordinator adjudication without network access, installation, scope growth,
  or publication.
