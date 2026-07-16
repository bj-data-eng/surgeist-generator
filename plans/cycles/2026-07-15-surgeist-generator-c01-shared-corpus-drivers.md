# C01 Shared Core Reconciliation

## Header

- Repository: `/Users/codex/Development/surgeist-generator`
- Status: `in_progress`; implementation is paused until this revision is
  independently `CLEAN`.
- Cycle base: `a8b8c6d1cbfe0480ca11a5d5f530ae5b06572412`.
- Reviewed semantic pair at exact commit
  `d3b3f8a783d7adfa2e0584af4a1f2f999c0bd0d4`, review `CLEAN`:
  - `plans/specs/2026-07-15-surgeist-generator-shared-corpus-drivers.md`,
    SG-01–SG-14, SHA-256
    `a672526df3d703419cfb0adf971405c6780b67dd78ceaece5ab3fc813cb22adc`;
  - `plans/specs/2026-07-15-surgeist-generator-focused-verification.md`,
    normative SG-13.2 companion, SHA-256
    `a8db3583cca978eaa6154977bb12e4e7601076b1a87b144df21bf8c101c2ed1f`.
- Reviewed sequence:
  `plans/sequences/2026-07-16-surgeist-generator-shared-corpus-drivers.md`
  C01 entry, SHA-256
  `760dfe56ac1fd57eef6b1cb5ac99bfc51e4a3d236b1e7280d6c164ac7e8b69c1`,
  exact revision `5b2ee3cc01c237f64713925aaad83b14855690ab`, independent
  review `CLEAN`.
- Outcome: preserve the audited layout copy and make the default-feature shared
  API, pinned-source verifier, rooted coordination, and durable transaction core
  satisfy the final pair. No domain driver is implemented in C01.

## Reviewed Multi-Cycle Sequence

1. **C01, this plan:** SG-02 closure and focused shared items 1–3 and 5–6;
   other shared-core work is an explicitly named prerequisite for later closure.
2. **C02, separately planned/reviewed after C01:** SG-03.2, SG-05.2, SG-11.1,
   focused shared item 4, and focused layout items 1–9. It owns optional layout
   dependencies, the exact at-most-15-
   physical-line binary, Taffy/Chromium lifecycle, XML, and legacy-copy removal.
3. **C03, separately planned/reviewed after C02:** every remaining cross-driver
   section, focused shared items 7–11, and focused CSS items 1–9. It owns CSS
   import provenance, neutral output, the exact at-most-15-physical-line binary,
   and the combined-feature task gate.
4. **C04, separately planned/reviewed after C03:** remaining SG-01, SG-13.3, and
   SG-14 integration/docs/evidence; full matrix, holistic review, final canonical
   publication/readback, and cross-repository handoff. Each preceding cycle is
   also canonically landed, published, and remotely verified before the next is
   planned from that immutable SHA.

## Boundary And Read-Only Baselines

- Mutate/commit only this repository. Do not edit, format, test, fetch, commit,
  or push root, layout, or CSS; do not run a real corpus, browser, download, or
  acquisition path. Use only installed tooling and cached sources with
  locked/offline or no-fetch commands. Install/acquire nothing.
- Keep Rust 1.97, edition 2024, `default = []`, tracked `Cargo.lock`, and no
  executable `unsafe`. The retained T02 range already adds exact shared
  `serde`/`serde_json`/`sha2`/`toml` plus empty layout/CSS features; its correction
  adds only target-specific `rustix`. Optional layout dependencies and binaries
  wait for C02/C03.
- Read-only sibling evidence:
  - layout was observed at HEAD `c8abd4a056b9c9ab74d109d5494736e8196e514b`
    with empty porcelain status during planning. A
    read-only audit of former baseline `fe1178e9..c8abd4a0` found four commits
    changing only its cycle plan and `src/{grid_tests,lib,lib_tests,node_input,scroll}.rs`;
    that range did not change its generator path. The invariant is the immutable
    `92054de…` source-prefix proof and generator-repository copy hash
    `d2f5ca87cea6b36826e9172e2d7ba7a99196c375e2ca53f8a84a075200e70a9f`,
    not layout's live HEAD/status. Each task records live layout state read-only;
    unrelated concurrent layout drift is observational and never authorizes a
    layout write/test or blocks C01 while the immutable proof still matches;
  - CSS HEAD `ae44d858308e4f73c17e91c4c8768c43ce6ceb82`, sole status
    `?? plans/specs/`, whose only file is
    `plans/specs/css-snapshot-2026-remediation.md`, mode `0644`, size `156722`,
    SHA-256 `08ba1dc26fd92faf7d95f562cb9639a9f9d40520b98992e6e5eef24083b3c1b6`;
  - root HEAD `19590f6d9fa01c0df197c5ef07fb626c5cf18ced`, empty porcelain status.

## Impacts

- API: additive exact default shared surface; feature APIs remain deferred.
- Dependencies/features: retained range adds exact `serde` with derive,
  `serde_json`, `sha2`, `toml`, the `default`/`layout-browser`/`css-corpus` feature
  table, `.gitignore` lockfile tracking, and `Cargo.lock`; correction adds exact
  target-specific `rustix` and refreshes that lockfile offline.
- Artifacts/docs: no generated corpus artifact, binary, README, or AGENTS change.
- Compatibility: Rust 1.97/edition 2024 and `CRATE_NAME` remain unchanged.
- Root/siblings: no integration or mutation; published C01 SHA is C02 input.
- Safety: no executable `unsafe`; descriptor authority remains crate-private.

## Task Packets

### C01-T01 — Preserve the layout production prefix (complete)

- Trace: SG-02. No focused runtime item.
- Evidence: commit `d72fd9c`; only `src/layout/legacy_generator.rs`; exact layout
  commit `92054de23b7c4d431556ef7e42e2226dd1788f1f`, lines 1–4626 including final
  LF; SHA-256
  `d2f5ca87cea6b36826e9172e2d7ba7a99196c375e2ca53f8a84a075200e70a9f`;
  prior exact one-path review `CLEAN`. C01 never modifies or tests this file.

### C01-T02 — Reconcile shared values, reports, and pinned sources (reopened)

- Complete review packet: initial implementation commits `5521d54` and
  `d2980f7`, plus the ordered corrective span recorded outside this semantic plan
  in `plans/evidence/2026-07-16-c01-t02.md`. The prior review is historical; a
  fresh reviewer assesses that complete packet against the final pair.
- Trace: C01 evidence owner for focused shared items 1–3 and 5. It implements
  prerequisites from SG-03.4, SG-06–8, and SG-12 plus source/public-API cases
  toward focused items 4 and 11, whose executable closure remains C02/C03.
- Files: retained-range `.gitignore`; `Cargo.toml`; mechanically refreshed
  `Cargo.lock`; `src/lib.rs`; `src/error.rs`;
  `src/core/{mod,case,corpus,hash,manifest,report,source}.rs`; focused
  public/source tests; task evidence. The legacy copy remains untouched.
- RED: first record targeted failures for exact public API/Serde goldens,
  overflow/path/disposition/report contracts, installed-Git trust, ordinary and
  linked `config.worktree`, raw cleanliness/filter sentinels, promisor absence,
  alternates, and immutable snapshots.
- Acceptance: exact default API and errors; deterministic validation/hashes;
  source verification is read-only, no-network, helper-free, full-pin exact, and
  closing-identity stable; no optional domain dependency is present.
- Gate:
  - `cargo generate-lockfile --offline`
  - `cargo check --locked --offline -p surgeist-generator --no-default-features`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features`
  - `cargo clippy --locked --offline -p surgeist-generator --no-default-features --all-targets -- -F unsafe-code -D warnings`
  - `cargo check --locked --offline -p surgeist-generator --target wasm32-unknown-unknown --no-default-features --lib`
  - `cargo fmt --check`
  - `cargo deny --all-features --locked --offline list --format tsv --layout license`
  - `cargo audit --no-fetch --stale`
  - reprove the immutable layout source/copy hash, record live layout HEAD/status,
    and compare CSS/root baselines exactly.
- Dependencies: reviewed sequence/C01 plan and preserved clean T01 evidence.
- Intended correction commit: `feat(core): reconcile shared source contracts`.

### C01-T03 — Reconcile rooted leases and durable transactions (reopened)

- Complete review packet: initial implementation commits `b70dea3` and
  `d32e9f6`, plus the ordered corrective span recorded outside this semantic plan
  in `plans/evidence/2026-07-16-c01-t03.md`. Its prior review was not clean; no
  finding or span is waived.
- Trace: C01 evidence owner for focused shared item 6. It implements prerequisite
  shared mechanisms from SG-04, SG-09–10, SG-12, and SG-13.1 plus precursor
  cases toward focused items 7–10, whose executable closure remains C03.
- Files: `src/core/{artifact,lease}.rs`, private rooted-fs/coordination/
  transaction/inventory modules, related reexports, and crash/race tests.
- RED: first record failures for bootstrap claim races, alias/mount/identity
  probes, shared/exclusive contention, pre-intent receipt and every publication/
  cleanup prefix, old-sidecar corruption, root swaps, stale/full/filtered policy,
  descriptor replacement, and exact error collisions.
- Acceptance: no public or pathname-only mutation authority; unsupported probes
  are clean while residue is `ArtifactTransaction`; lock files are never partial;
  checks create/recover nothing; pre-commit preserves old, post-commit preserves
  new plus resumable evidence; cleanup removes only receipt-bound identities;
  focused item 6 and every named later-cycle prerequisite have deterministic
  synthetic evidence without claiming focused items 7–10 complete.
- Gate:
  - `cargo check --locked --offline -p surgeist-generator --no-default-features`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features`
  - `cargo clippy --locked --offline -p surgeist-generator --no-default-features --all-targets -- -F unsafe-code -D warnings`
  - `cargo check --locked --offline -p surgeist-generator --target wasm32-unknown-unknown --no-default-features --lib`
  - `cargo fmt --check`
  - reprove the immutable layout source/copy hash, record live layout HEAD/status,
    and compare CSS/root baselines exactly.
- Dependencies: fresh `CLEAN` complete-packet review for T02.
- Intended correction commit: `feat(core): reconcile rooted transactions`.

## C01 Closeout

1. Complete the external task-evidence files and obtain fresh `CLEAN` complete-
   packet reviews for T02/T03; retain T01's exact clean proof.
2. Make only the header status transition to `complete`. Run both task gates,
   license/audit commands, owned-Rust manifest plus canonical unsafe scan, the
   immutable layout proof/live observation, exact CSS/root baselines, and
   clean-HEAD/status verification.
3. Obtain a fresh holistic `CLEAN` review of exact `cycle_base..cycle_head`, the
   semantic pair, task packets/reviews, and evidence. An owned finding reopens
   T02/T03. A cross-task finding first requires a fresh reviewed plan revision
   adding one bounded integration-fix packet; no edit precedes that review.
4. After holistic `CLEAN`, rerun the complete step-2 gate and clean-HEAD/evidence
   verification at the unchanged reviewed SHA.
5. Apply the canonical Surgeist exact-range reconciliation, re-review, automated
   landing/publication, immutable-SHA remote readback, and cleanup workflow; hand
   the published C01 SHA to C02 and only then plan C02 from it.

Stop for coordinator adjudication on an authority-remote change, unavailable
cache/target, CSS/root baseline change, layout pinned-source/copy mismatch,
evidence of a sibling write/test by this cycle, safety finding, or review failure.
Do not gain network/install authority or widen scope.
