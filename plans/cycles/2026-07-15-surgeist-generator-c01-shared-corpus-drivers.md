# C01 Shared Core Reconciliation

## Header

- Repository: `/Users/codex/Development/surgeist-generator`
- Status: `in_progress`; implementation is paused until this revision is
  independently `CLEAN`.
- Cycle base: `a8b8c6d1cbfe0480ca11a5d5f530ae5b06572412`.
- Reviewed semantic pair at exact commit
  `d3b3f8a783d7adfa2e0584af4a1f2f999c0bd0d4`, review `CLEAN`:
  - main SG-01–SG-14, SHA-256
    `a672526df3d703419cfb0adf971405c6780b67dd78ceaece5ab3fc813cb22adc`;
  - normative SG-13.2 companion, SHA-256
    `a8db3583cca978eaa6154977bb12e4e7601076b1a87b144df21bf8c101c2ed1f`.
- Outcome: preserve the audited layout copy and make the default-feature shared
  API, pinned-source verifier, rooted coordination, and durable transaction core
  satisfy the final pair. No domain driver is implemented in C01.

## Reviewed Multi-Cycle Sequence

1. **C01, this plan:** SG-02; shared/core clauses of SG-03, SG-04, SG-06,
   SG-07.1, SG-08–SG-10, SG-12, SG-13.1/13.2. Focused shared items 1–11.
2. **C02, separately planned/reviewed after C01:** layout edges of SG-03.2/3.4,
   SG-05.1–5.2, layout clauses of SG-06/8–10/12/13.1, SG-11.1, and focused
   layout items 1–9. It owns optional layout dependencies, the exact at-most-15-
   physical-line binary, Taffy/Chromium lifecycle, XML, and legacy-copy removal.
3. **C03, separately planned/reviewed after C02:** CSS edges of SG-03.4,
   SG-05.3, SG-07.2–7.3, CSS clauses of SG-08–10/12/13.1, SG-11.2, and focused
   CSS items 1–9. It owns CSS import provenance, neutral output, the exact
   at-most-15-physical-line binary, and the combined-feature task gate.
4. **C04, separately planned/reviewed after C03:** remaining SG-01, SG-13.3, and
   SG-14 integration/docs/evidence; full matrix, holistic review, canonical
   automated landing/publication with immutable-SHA push/readback/cleanup, and
   cross-repository handoff. No earlier cycle publishes.

## Boundary And Read-Only Baselines

- Mutate/commit only this repository. Do not edit, format, test, fetch, commit,
  or push root, layout, or CSS; do not run a real corpus, browser, download, or
  acquisition path. Use only installed tooling and cached sources with
  locked/offline or no-fetch commands. Install/acquire nothing.
- Keep Rust 1.97, edition 2024, `default = []`, tracked `Cargo.lock`, and no
  executable `unsafe`. C01 may add only SG-03.2's target-specific `rustix`; all
  optional layout dependencies and binary targets wait for C02/C03.
- Recorded read-only baseline, compared again after every C01 task and at close:
  - layout HEAD `fe1178e99ec567c3f887b595700c2ca6b2e75133`, empty porcelain status;
  - CSS HEAD `ae44d858308e4f73c17e91c4c8768c43ce6ceb82`, sole status
    `?? plans/specs/`, whose only file is
    `plans/specs/css-snapshot-2026-remediation.md`, mode `0644`, size `156722`,
    SHA-256 `08ba1dc26fd92faf7d95f562cb9639a9f9d40520b98992e6e5eef24083b3c1b6`;
  - root HEAD `19590f6d9fa01c0df197c5ef07fb626c5cf18ced`, empty porcelain status.

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
  `d2980f7`, plus the ordered corrective commit span recorded here before review.
  The prior clean review is historical; a fresh reviewer assesses the complete
  packet against the final pair.
- Trace: shared/default clauses of SG-03.1–3.4 and SG-06; SG-07.1; SG-08;
  static/path/source/report rows of SG-12; focused shared items 1–6 and 11.
- Files: `Cargo.toml`, mechanically refreshed `Cargo.lock`, `src/lib.rs`,
  `src/error.rs`, `src/core/{mod,case,corpus,hash,manifest,report,source}.rs`, and
  focused public/source tests. The legacy copy remains untouched.
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
  - compare all three read-only baselines above exactly.
- Intended correction commit: `feat(core): reconcile shared source contracts`.

### C01-T03 — Reconcile rooted leases and durable transactions (reopened)

- Complete review packet: initial implementation commits `b70dea3` and
  `d32e9f6`, plus the ordered corrective commit span recorded here before review.
  Its prior review was not clean; no finding or span is waived.
- Trace: shared/core SG-04 and SG-09–10; transaction/lease/capability precedence
  in SG-12; shared coordination clauses of SG-13.1; focused shared items 7–10.
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
  every focused shared item has deterministic synthetic evidence.
- Gate:
  - `cargo check --locked --offline -p surgeist-generator --no-default-features`
  - `cargo test --locked --offline -p surgeist-generator --no-default-features`
  - `cargo clippy --locked --offline -p surgeist-generator --no-default-features --all-targets -- -F unsafe-code -D warnings`
  - `cargo check --locked --offline -p surgeist-generator --target wasm32-unknown-unknown --no-default-features --lib`
  - `cargo fmt --check`
  - compare all three read-only baselines above exactly.
- Intended correction commit: `feat(core): reconcile rooted transactions`.

## C01 Closeout

1. Record each correction span in this plan and obtain fresh `CLEAN` reviews of
   the complete ordered T02 and T03 packets; retain T01's exact clean proof.
2. In a planning-only commit mark C01 tasks/status complete, then rerun both task
   gates, the owned-Rust manifest plus canonical executable-unsafe scan, and exact
   sibling/root baseline comparisons.
3. Obtain a fresh holistic `CLEAN` review of exact `cycle_base..cycle_head`, the
   complete semantic pair, all task reviews, and final evidence. A finding reopens
   its owning T02/T03 packet, appends a correction, and repeats that packet review
   and closeout; it does not create an unplanned task.
4. Record the reviewed C01 SHA as C02's immutable input. Do not land/push/publish
   in C01; C04 alone applies the canonical Surgeist landing/publication workflow.

Stop for coordinator adjudication on an authority-remote change, unavailable
cached dependency/installed target, sibling baseline change, safety finding, or
review failure. Do not gain network/install authority or widen scope.
