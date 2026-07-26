# P01-I02 CSSTree Grouped-Case Import

## 1 Outcome

Extend the feature-gated CSSTree fixture decoder so the existing CSS corpus
workflow imports one case object or an array of case objects beneath every
top-level label. Classify newly accepted members with the pinned loader's
per-member outcome rule, derive stable unique IDs without changing existing
IDs, and preserve the schema-1 neutral expectation and report contracts.

The completed initiative imports the pinned CSSTree `fixtures/ast` tree at
`88e3d965c0b1628642a30a841745b410d6835052` as exactly 74 files and 935
neutral cases: 721 parsed and 214 rejected. It executes no upstream code and
leaves ordinary `check-corpus` independent of the source checkout, Node.js, a
browser, and the network.

## 2 Non-Goals

- No `surgeist-css`, root `surgeist`, layout-generator, gitlink, root API
  artifact, or sibling implementation change.
- No public CSS command, request, feature, error-kind, neutral schema version,
  report schema, or manifest schema change.
- No AST, upstream diagnostic prose, JavaScript error class, source locator, or
  parser implementation in neutral output.
- No CSSTree, Node.js, browser, network, source-acquisition, or downloader
  execution.
- No committed source checkout, production corpus, generated expectation tree,
  or report.
- No new dependency, feature, target, binary, script, generator, or CI rule.
- No change to source verification, import publication, filtering,
  transactions, leases, or layout behavior except where focused regression
  evidence proves that shared behavior must remain unchanged.
- No executable `unsafe`.

## 3 Authority, Ownership, And Evidence

### 3.1 Authority And Coordinator Instruction

`surgeist-generator` owns fixture decoding, neutral identity derivation,
canonical serialization, reports, focused tests, documentation, commits, and
its published candidate. The project handoff is
`plans/P01-resolve-baseline-review/P01-csstree-grouped-case-import-handoff.md`
at commit `54800c8267654cc43aaa8c0ffc749caf9a9001ae` with digest
`sha256:6db5e87a7f1a95d2b16c9aef033128fbd66319018b7cf45178ad419f1cf32805`.

The sender specification is retained in the sibling `surgeist-css` repository
at immutable local commit
`4b288d6467d91f2fc33eac78ef0b0b725154195d`, revision `P01/I02/V01`,
and SHA-256
`684825e26a7aa6896df2532d463e70d82c49d8e7b284d4c7bc1b863c7cd50b12`.
Its clauses `3.1` through `3.3` own the consumer evidence and prerequisite. The
committed recipient handoff above is this packet's one-hop planning reference
and records the sender's exact absolute artifact path.

At intake the sender's remote `main` remained
`5ab2461adae33b5cba2c29154929a89f61c8e303`. The user directed the
coordinator to check the sibling repository directly. This specification
therefore uses the exact local immutable sender commit as its evidence source
and records the remote lag rather than treating it as authority to edit or
publish the sibling.

### 3.2 Recipient Baseline

The recipient cycle base is
`54800c8267654cc43aaa8c0ffc749caf9a9001ae`, whose parent is the handoff's
expected provider base
`c9d2f61f1f6d39c233907db88073e6c9737ca7ec`.

At that source revision:

1. `src/css/expectation.rs` deserializes literal top-level `error` as
   `Vec<RawErrorCase>`.
2. Every other top-level value must deserialize as one `RawOrdinaryCase`.
3. Ordinary cases require string `source` and any `ast`; optional `options`
   must be an object and optional `generate` a string.
4. Error-array members require string `source`; other fields are ignored.
5. Ordinary IDs are `<fixture>#/<escaped-label>`.
6. Literal error-array IDs are `<fixture>#/error/<index>`.
7. Cases are sorted by final ID before canonical serialization.
8. Duplicate decoded JSON members and trailing values are rejected before
   typed decoding.
9. The current decoder cannot deserialize the pinned corpus's 24 non-`error`
   array groups.

### 3.3 Independently Verified Corpus

The existing ignored evidence checkout at
`/Users/codex/Development/surgeist-css/tmp/csstree` resolves to revision
`88e3d965c0b1628642a30a841745b410d6835052`; its `fixtures/ast` tree is
`bfadc7a7a8d93dce59a27fa7df3bb0f6f6a623d8`.

A read-only traversal of that tree independently confirmed:

| Evidence | Count |
| --- | ---: |
| JSON fixture files | 74 |
| Parsed cases | 721 |
| Rejected cases | 214 |
| Total cases | 935 |
| Top-level array groups | 60 |
| Array members | 297 |
| Non-`error` array groups | 24 |
| Non-`error` array members | 108 |
| Cases carrying options | 34 |

The observed option keys are `atrule`, `parseAtrulePrelude`,
`parseCustomProperty`, `parseRulePrelude`, `parseValue`, and `property`.
`stylesheet/tolerant.json` group `bad selector` contains two parsed members and
one rejected member, proving that enclosing-label classification is incorrect.
The pinned loader's `if (test.error)` branch uses JavaScript truthiness rather
than structural member presence. All 214 rejected members in the pinned tree
carry truthy string `error` values, so the exact census does not distinguish
those predicates; synthetic evidence must.

## 4 Neutral Case Model

### 4.1 Group And Member Phases

Typed decoding separates the enclosing group from the member:

```text
FixtureGroup {
    label,
    members: Singleton(CaseMember) | Array(Vec<CaseMember>)
}

CaseMember {
    source,
    ast_seen,
    error_value,
    options,
    generate
}
```

The exact Rust representation remains private. It must preserve the semantic
phases above so member outcome and identity do not depend on a special
top-level label.

An object value is a singleton group. An array value is a grouped sequence,
including an empty sequence that derives no cases and therefore fails the
existing non-empty-fixture rule when the complete fixture has no other cases.
Member order is the JSON array order.

### 4.2 Stable ID Grammar

Let `path` be the strict import-relative fixture path and `token(label)` be the
existing JSON Pointer token escape that replaces `~` with `~0` and `/` with
`~1`.

| Source shape | Exact case ID |
| --- | --- |
| Object under label `L` | `<path>#/<token(L)>` |
| Array member at zero-based index `N` under label `L` | `<path>#/<token(L)>/<N>` |

`N` is canonical base-ten ASCII without a sign or leading zero except the
single digit `0`. This grammar preserves every existing ordinary-object ID and
every existing literal-`error` array ID.

The label field in neutral output is:

- omitted for array members under literal label `error`, preserving existing
  neutral bytes; and
- the decoded enclosing label for every singleton object, including one under
  literal label `error`, and every array member under any other label.

The pointer escape is injective. Distinct fixture paths, decoded labels, source
shapes, or member indexes must never collapse to one ID. The existing global
duplicate-ID guard remains authoritative and returns `InvalidInventory` with
the duplicate ID if a collision is observed.

### 4.3 Per-Member Outcome And Legacy Compatibility

An array under the literal top-level label `error` is an explicit compatibility
carve-out. Every member in that array remains rejected, requires only string
`source`, and ignores `ast`, `error`, `options`, `generate`, and all other
members exactly as the baseline `RawErrorCase` decoder does. Its neutral label,
options, and canonical CSS remain omitted. This preserves previously accepted
source-only error arrays and their exact bytes.

Every member outside that array carve-out, including a singleton object under
literal label `error`, is `rejected` exactly when its `error` value is
JavaScript-truthy, matching the pinned loader's `if (test.error)` branch. For
JSON values, `null`, `false`, numeric zero including negative zero, and the
empty string are falsy; nonzero numbers, nonempty strings, arrays, and objects
including empty arrays and objects are truthy. An absent `error` member is also
falsy. A falsy or absent `error` member makes the case `parsed` and requires an
`ast` member. AST and error payload values do not enter neutral output.

Every member requires string `source`. Outside the literal-`error` array
compatibility carve-out, optional `options` must be an object and is
canonicalized recursively by the existing key-ordering contract, while optional
`generate` must be a string and remains `canonical_css`. Unknown upstream
members remain ignored after the duplicate-member prepass, preserving
compatibility with fields such as `offset`, `comment`, `_error`, and
`skipRoundtrip`.

A truthy `error` value takes outcome precedence when `ast` is also present.
This matches the pinned upstream loader and the observed rejected fixture that
contains both fields. Outside the literal-`error` array compatibility
carve-out, options and generated CSS, when shape-valid, remain preserved
independently of outcome.

### 4.4 Deterministic Ordering

Import retains the existing Git-verified fixture ordering. Typed decoding
retains each array's member order in its zero-based IDs. Final expectation cases
remain sorted by complete case ID using Rust string lexicographic order, so
top-level JSON object-member order remains semantically irrelevant and repeated
generation is byte-identical.

## 5 Decoder And Failure Contract

### 5.1 Accepted Shapes

Every top-level value must be either:

1. one case object; or
2. one array whose every element is a case object.

Arrays outside the literal-`error` compatibility carve-out may mix parsed and
rejected members according to clause `4.3`. A singleton object under literal
label `error` follows the same truthiness, label, metadata, and validation rules
as every other singleton. Existing ordinary object fixtures and literal-`error`
arrays produce byte-identical cases, IDs, labels, outcomes, options, canonical
CSS, dispositions, and reasons.

### 5.2 Rejected Shapes

The existing `InvalidInventory` kind and `derive CSS expectations` operation
remain stable. Derivation fails before publication for:

- a top-level scalar or nested array where a case object is required;
- a member without string `source`;
- a non-legacy member with absent or falsy `error` and without `ast`;
- non-object `options`;
- non-string `generate`;
- duplicate decoded members at any depth;
- trailing JSON values;
- an empty complete derived case set;
- a duplicate derived case ID;
- an unmatched manifest override; or
- a full manifest case-count mismatch.

No failure may publish a partial expectation or report, alter the imported
source snapshot, or weaken existing transaction and stale-state semantics.

## 6 Artifacts, Reports, And Checking

Neutral expectation schema version `1` is unchanged. Each non-legacy case
retains exactly:

- ID;
- context;
- optional label;
- source text;
- optional canonical options;
- `parsed` or `rejected` upstream outcome;
- optional canonical CSS;
- disposition; and
- optional reason.

Literal-`error` array members retain the baseline subset: ID, context, source
text, rejected outcome, disposition, and optional reason. Their ignored
metadata must not begin affecting neutral bytes. A singleton under label
`error` is not part of that legacy subset.

The generation report remains the existing one-artifact-per-fixture report.
Its artifact `case_count` and aggregate disposition counts cover every derived
member exactly once. A full generation over the pinned tree therefore covers
74 expectation artifacts and 935 cases.

`check-corpus` continues to validate the imported sidecar, imported fixture
bytes, expectations, reports, counts, provenance, and inventory from persisted
corpus state only. It neither re-derives from an external checkout nor opens
Git, Node.js, a browser, or the network.

## 7 API, Dependency, And Compatibility Classification

| Concern | Classification |
| --- | --- |
| Public Rust API | Unchanged |
| `css-corpus` feature | Unchanged |
| CSS binary and commands | Unchanged |
| Manifest schema | Unchanged schema `1` |
| Neutral expectation schema | Unchanged schema `1` |
| Report schema | Unchanged |
| Existing neutral bytes | Byte-compatible for previously accepted shapes |
| Accepted upstream input | Additive object-or-array support under every label |
| Dependencies and lockfile | Unchanged |
| MSRV | Preserve Rust `1.97` |
| Platform | Preserve current support |
| Generated artifacts | None committed |
| Root integration | Future `surgeist-css` consumer work only |
| Safety | No executable `unsafe` |
| Anticipated SemVer | Compatible behavior correction; final versioning owns classification |

## 8 Verification Contract

### 8.1 Focused Synthetic Evidence

Generator-owned tests must prove:

1. arbitrary labels accept singleton objects and arrays;
2. the pinned mixed-group shape classifies each member independently;
3. singleton and grouped IDs follow clause `4.2`, including `~` and `/`
   escaping;
4. grouped IDs are deterministic, unique, and member-order preserving;
5. literal `error` arrays retain their existing IDs, omitted labels, and
   rejected outcomes, continue to accept source-only members, and continue to
   ignore `ast`, `error`, `options`, and `generate` without changing bytes;
6. outside the literal-`error` carve-out, absent, `null`, `false`, positive or
   negative zero, and empty-string `error` values are parsed when `ast` exists,
   while nonzero numbers, nonempty strings, arrays, and objects are rejected;
7. singleton objects under label `error` exercise both truthy and falsy
   outcomes and retain their normal label and shape-valid metadata;
8. object-only fixture bytes remain unchanged;
9. options are recursively canonical and preserved for grouped members outside
   the literal-`error` carve-out;
10. malformed member shapes and duplicate decoded identities fail as
   `InvalidInventory` without publication;
11. repeated generation produces byte-identical expectations and reports; and
12. full expected-case mismatch remains pre-publication failure.

The focused tests remain synthetic and self-contained. They do not read a
sibling checkout or require upstream software.

### 8.2 Exact Pinned Acceptance

One acceptance run uses the existing explicit source-root boundary against the
read-only sibling evidence checkout. It must:

1. verify the exact revision and tree from clause `3.3`;
2. import with manifest counts 74 and 935;
3. generate and check the complete corpus;
4. count 74 expectation files, 721 parsed cases, 214 rejected cases, and 935
   total cases from neutral output;
5. run generation a second time and prove expectation and report bytes are
   identical; and
6. remove its disposable owner/corpus root without changing either repository.

This acceptance is not an ordinary-test dependency and commits no corpus. It
uses only already-present Git, Cargo artifacts, and the existing checkout.

### 8.3 Verification Selection

The current cycle plan must select the applicable offline commands from the
repository guide's command inventory and the installed workflow's canonical
gate. Evidence must cover the affected `css-corpus` feature, the composed
all-features crate, formatting, diff hygiene, and repository-wide owned-Rust
unsafe absence. Ignored exhaustive filesystem diagnostics are not executed
under the active user instruction; only the configured list-only inventory
command may inspect them.

## 9 Implementation Boundary

This is one single-repository cycle under telemetry sequence `P01/I02/S01`; no
sequence document is required. The current cycle may change only the private
CSS expectation decoder, focused CSS tests, and directly affected CSS-facing
documentation when the implementation makes clarification necessary.

A worker must not redesign source verification, publication, reports, public
API, or cross-repository behavior.

The successful handoff to `surgeist-css` is a `CRATE_CANDIDATE` report containing
the published full SHA, reviewed planning revisions, exact ID grammar,
compatibility result, command evidence, reviews, census, checkout-free checking
confirmation, dependency and safety classifications, and any genuine
nonblocking observation.

## 10 Acceptance And Blockers

The initiative is accepted only when:

1. every clause `4` through `7` invariant is represented in source and focused
   tests;
2. the exact pinned acceptance in clause `8.2` proves 74 files and the
   721/214/935 census;
3. the cycle plan's applicable command matrix passes using already-present
   offline tooling;
4. task and holistic reviews are clean for their exact attributed ranges;
5. the exact reviewed candidate is published to and read back from recipient
   remote `main`;
6. telemetry records the clean review, publication attestation, and closed
   cycle; and
7. the return handoff satisfies clause `9`.

A contradiction in the immutable sender specification, inability to preserve
existing IDs or neutral bytes, missing offline tooling, source revision or tree
drift, acquisition requirement, executable `unsafe`, or failure of the exact
census is a blocker. The coordinator must report the exact path, command,
observed result, expected result, contract impact, and next-action owner rather
than narrowing the corpus or reclassifying lost cases.
