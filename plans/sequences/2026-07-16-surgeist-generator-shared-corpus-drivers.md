# Shared Corpus Drivers Implementation Sequence

## Header

- Repository: `/Users/codex/Development/surgeist-generator`
- Status: `proposed`
- Immutable cycle-series base:
  `a8b8c6d1cbfe0480ca11a5d5f530ae5b06572412`
- Reviewed semantic inputs at
  `d3b3f8a783d7adfa2e0584af4a1f2f999c0bd0d4`:
  - `plans/specs/2026-07-15-surgeist-generator-shared-corpus-drivers.md`,
    SHA-256 `a672526df3d703419cfb0adf971405c6780b67dd78ceaece5ab3fc813cb22adc`;
  - `plans/specs/2026-07-15-surgeist-generator-focused-verification.md`,
    SHA-256 `a8db3583cca978eaa6154977bb12e4e7601076b1a87b144df21bf8c101c2ed1f`.
- Terminal outcome: one reviewed/published shared-core extraction, followed by
  reviewed/published layout, CSS, and integration candidates in strict order.

## Invariants

- Every cycle is independently planned, reviewed, implemented, task-reviewed,
  verified, holistically reviewed, canonically landed/published to
  `surgeist-generator` `origin/main`, remotely read back by immutable SHA, and
  handed off before the next cycle is planned from that published SHA.
- Only this repository mutates. Root, layout, and CSS remain read-only; their
  corpora/tests are never run. No real browser/acquisition path runs. All Cargo
  work is locked/offline or no-fetch with already installed tooling/caches.
- Rust remains 1.97/edition 2024, `default = []`, and executable-`unsafe` free.
  Consumer corpora and root integration/API artifacts never move into this leaf.

## Exact Closure Allocation

Each main-spec section/subsection has one closure cycle; focused items have one
evidence cycle. Earlier cycles may implement a named prerequisite used by a
later closure owner, but cannot claim that later section complete.

| Cycle | Exact main-spec closure | Exact focused evidence |
| --- | --- | --- |
| C01 Shared Core | SG-02.1–2.4; SG-03.1; SG-04.1–4.3; SG-07.1 | shared items 1–11 |
| C02 Layout Driver | SG-03.2; SG-05.2; SG-11.1 | layout items 1–9 |
| C03 CSS And Driver Integration | SG-03.3–3.4; SG-05.1 and 5.3; SG-06.1–6.2; SG-07.2–7.3; SG-08; SG-09.1–9.3; SG-10; SG-11.2–11.3; SG-12; SG-13.1–13.2 | CSS items 1–9 |
| C04 Final Integration | SG-01; SG-13.3; SG-14 | final matrix/evidence, not a new SG-13.2 item |

## Ordered Cycles

### C01 — Shared Core Reconciliation

- Preserve the reviewed layout prefix copy, then reconcile the retained T02/T03
  code with the final default-feature shared API, source verification, rooted
  coordination, transactions, recovery, and all shared focused evidence.
- Prerequisite clauses implemented for later closure: shared API portions of
  SG-03.4, shared source/collection of SG-06, shared hashes/provenance of SG-08,
  shared mechanisms of SG-09–10, shared errors of SG-12, and read-only shared
  checking of SG-13.1.
- Impacts: additive default API; target-specific exact `rustix` dependency and
  offline lock refresh; no binary/artifact/docs/MSRV change; no executable
  `unsafe`. Publish the clean core SHA before C02.

### C02 — Layout Driver Extraction

- Add the complete SG-03.2 optional dependency graph, layout feature/API/binary,
  schema-2 manifest, Taffy and Chromium cache/acquisition/resource lifecycle,
  deterministic XML/report behavior, layout offline checking, and synthetic
  layout evidence; remove the transient copied file only after representation.
- Prerequisite clauses implemented for C03 closure: layout portions of
  SG-03.3–3.4, SG-05.1, SG-06, SG-08–10, SG-12, SG-13.1, and SG-11.3.
- Impacts: additive feature/API and exact optional dependencies; one exact
  at-most-15-physical-line binary; no committed corpus artifact/MSRV/unsafe
  change. Run layout-only and default gates, then publish/read back before C03.

### C03 — CSS Driver And Cross-Driver Closure

- Add the dependency-free CSS feature/API/binary, schema-1 import sidecar,
  neutral expectations/reports, CSS checking, CSS focused evidence, and combined
  feature coexistence; close every cross-domain/shared section allocated above.
- Impacts: additive feature/API, no dependency; one exact at-most-15-physical-
  line binary; no committed corpus artifact/MSRV/unsafe change. Run default,
  layout-only, CSS-only, and combined gates, then publish/read back before C04.

### C04 — Final Integration, Evidence, And Handoff

- Update README/AGENTS, run SG-13.3 verbatim plus owned-Rust unsafe/license/audit
  evidence, reconcile all cycle SHAs/reviews and sibling baselines, and obtain a
  final holistic review. Apply the canonical automated landing/publication
  workflow, immutable-SHA remote readback, cleanup, and root/layout/CSS handoff.
- Impacts: documentation/integration evidence only unless a separately reviewed
  correction cycle is required; no schema/MSRV/dependency/artifact/unsafe change.

## Stop And Replan

A changed authority remote, sibling baseline, unavailable cache/installed tool,
license/advisory/safety finding, or non-clean review stops the current cycle.
Resolve it within that cycle's canonical correction/re-review flow or create a
new reviewed sequence revision; never skip ahead, acquire software, widen scope,
or publish an unreviewed descendant.
