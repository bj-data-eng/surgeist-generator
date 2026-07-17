# Surgeist Generator Review Remediation Implementation Sequence

## Authority

- Design-owning repository: `/Users/codex/Development/surgeist-generator`.
- Immutable implementation-series base:
  `1e545530fb992e2e749f26a1ba7ac0d77ed85dd6`.
- Reviewed specification:
  `plans/specs/2026-07-17-surgeist-generator-review-remediation.md` at commit
  `24300d5774bb28cf0f0c40335ed3a35a71d56fd5`, normalized-content SHA-256
  `ccd42c66d3b78f0e9ca57e5bdd4fb7582af059fa55ce0cf5978cd98cffa7c7c7c7`.
- Scope remains the leaf repository only. Root and sibling adoption are future
  work outside this sequence.

## Ordered Cycles

### C01 — Rooted Core And Real Recovery Proof

- Owning repository: `surgeist-generator`.
- Bounded outcome: rooted existence handles missing intermediate components
  without weakening path policy, and production bootstrap/install/recovery paths
  have exhaustive interruption evidence instead of model-only claims.
- Specification sections: SR-02 rows **Rooted lease-tree rejection** and
  **Model-only crash/bootstrap tests**; SR-04.1 through SR-04.4 and the shared
  transaction/error-boundary clauses of SR-04.5.
- Prerequisites: clean specification and sequence reviews; immutable base and
  preservation copy revalidated; no domain-driver implementation begun.
- Entry state: the implementation-series base plus planning-only commits; current
  lease tests fail before their intended assertions and crash/bootstrap proof is
  tautological.
- Exit evidence: all lease/artifact assertions are reachable; exhaustive
  production-prefix install, recovery, cleanup, and both contention branches
  satisfy the specified visibility, residue, error, and idempotence oracles; the
  complete shared-core affected matrix is clean.
- Handoff: publish and remotely verify the leaf cycle candidate, then hand its
  immutable SHA and core evidence to C02. No root or sibling handoff.

### C02 — Protected Source Foundation And CSS Vertical Slice

- Owning repository: `surgeist-generator`.
- Bounded outcome: shared protected-source/disjointness machinery supports a
  complete `css-corpus` API and thin binary whose import, generation, checking,
  sidecars, expectations, reports, and publication behavior are real and
  deterministic.
- Specification sections: SR-03.1 and SR-03.2 shared source-protection rules and
  CSS command rows; SR-04.5 CSS state transitions; SR-05.1 CSS feature edge;
  SR-05.3; SR-05.4 CSS boundary; SR-07.1 through SR-07.4.
- Prerequisites: C01 published and remotely verified; its production transaction
  and recovery contracts are the only publication foundation used here.
- Entry state: the shared core is task-clean and cycle-clean; `css-corpus` remains
  inert and no CSS binary or CSSTree artifact contract is implemented.
- Exit evidence: the CSS feature and binary execute all three commands against
  synthetic explicit roots; source protection, SHA-1/SHA-256 sidecars, neutral
  case ordering, exact expectations/reports, full/filtered state transitions,
  stale removal, read-only verification, and real CLI failure behavior match the
  reviewed contract.
- Handoff: publish and remotely verify the leaf cycle candidate, then hand its
  immutable SHA plus shared-source and CSS evidence to C03. No root or sibling
  mutation or adoption.

### C03 — Layout Driver Migration And Preservation Retirement

- Owning repository: `surgeist-generator`.
- Bounded outcome: the complete `layout-browser` API and thin binary replace the
  preservation copy with compiled, acquisition-free layout corpus management,
  supervised Chromium measurement, deterministic XML/reports, and recoverable
  browser-profile lifecycle behavior.
- Specification sections: SR-01.1 and SR-01.2 product decisions; SR-02 rows
  **Missing domain surface** and **Tautological CLI test**; SR-03.1 and SR-03.2
  layout command/browser clauses; SR-03.3; SR-04.5 layout transitions; SR-05.1
  layout dependencies and feature edge; SR-05.2; SR-05.4 layout/supervisor
  boundary; SR-06.1 through SR-06.4; SR-08.1.
- Prerequisites: C02 published and remotely verified; shared source protection,
  transaction recovery, and package feature boundaries remain valid; the exact
  preservation digest still matches.
- Entry state: the CSS vertical slice is complete; `layout-browser` remains inert
  and all retained layout behavior exists only in the immutable preservation
  source.
- Exit evidence: schema-2 compatibility and Taffy migration contracts, layout
  import/check/generate modes, exact Chromiumoxide switch semantics, trusted
  executable boundary, supervisor/profile recovery, panic/error precedence,
  XML/report publication, and both binary error paths are covered by the named
  synthetic evidence; every retained responsibility is mapped before the
  preservation source is absent.
- Handoff: publish and remotely verify the leaf cycle candidate, then hand its
  immutable SHA, retirement map, feature/API surface, and domain evidence to C04.
  Layout-corpus adoption remains future sibling-owned work.

### C04 — Terminal Quality, Guidance, And Leaf Candidate

- Owning repository: `surgeist-generator`.
- Bounded outcome: the composed two-driver crate has clean configured quality,
  dependency-policy, portable-default, documentation, and candidate-handoff
  evidence with no artificial linkage or stale scaffold guidance.
- Specification sections: SR-02 rows **Quality matrix** and **Stale guidance**;
  SR-04.6; SR-05.1 license/advisory and complete feature-matrix clauses;
  SR-08.2; SR-08.3; SR-09.
- Prerequisites: C03 published and remotely verified; both real features and
  binaries are present and the preservation source has been retired.
- Entry state: functional domain work is cycle-clean, while terminal formatting,
  warning, policy, documentation, and composed-matrix closure remains.
- Exit evidence: no private-front-door linkage remains; the complete supported
  feature, test, Clippy, format, portable-default, license, advisory, unsafe, and
  provenance matrix is clean offline; README/AGENTS match the resulting source;
  the tree is clean and the immutable remote `main` readback matches the reviewed
  leaf candidate.
- Handoff: produce the canonical leaf candidate record for the user's independent
  review. Any later root gitlink or sibling corpus adoption requires a separate
  owning-repository workflow.

## Closure Allocation

The cycles above are the only closure owners:

| Contract or finding | Closure cycle |
| --- | --- |
| Rooted lease-tree rejection | C01 |
| Model-only crash/bootstrap tests | C01 |
| SR-04.1 through SR-04.4 and shared SR-04.5 clauses | C01 |
| SR-03.1/SR-03.2 shared and CSS clauses plus CSS SR-04.5 clauses | C02 |
| SR-05.3, CSS portions of SR-05.1/SR-05.4, and SR-07 | C02 |
| SR-01, layout portions of SR-03/SR-04.5/SR-05, and SR-06 | C03 |
| Missing domain surface and tautological CLI test | C03 |
| SR-04.6, terminal SR-05.1 clauses, SR-08.2/SR-08.3, and SR-09 | C04 |
| Quality matrix and stale guidance | C04 |

SR-02 is exhausted by its six individually allocated rows. SR-08.1 closes with
C03; no specification section or baseline finding is deferred beyond C04.
