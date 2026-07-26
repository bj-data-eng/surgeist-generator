# P01 CSSTree Grouped-Case Import Handoff

## 1 Handoff Identity

`SURGEIST_HANDOFF: WORK_DIRECTIVE`

| Field | Value |
| --- | --- |
| Handoff ID | `css-p01-i02-to-generator-p01-csstree-grouped-cases-v1` |
| State | Prepared in the receiving repository; clause 10 blocks issuance as `READY_TO_START` until its publication predicates hold |
| Issuer | `surgeist-css` at `/Users/codex/Development/surgeist-css` |
| Issuer remote | `https://github.com/bj-data-eng/surgeist-css.git` |
| Issuer specification | Exact absolute path below, clauses 3.1 through 3.3 |
| Issuer specification revision | `P01/I02/V01` |
| Issuer specification SHA-256 | `684825e26a7aa6896df2532d463e70d82c49d8e7b284d4c7bc1b863c7cd50b12` |
| Issuer specification commit | `4b288d6467d91f2fc33eac78ef0b0b725154195d` |
| Recipient | `surgeist-generator` at `/Users/codex/Development/surgeist-generator` |
| Recipient project | `P01` |
| Expected recipient base | `c9d2f61f1f6d39c233907db88073e6c9737ca7ec` |
| Recipient remote | `https://github.com/bj-data-eng/surgeist-generator.git` |
| Target branch | `main` |
| Publication required | Yes |
| Return contract | `CRATE_CANDIDATE` or `BLOCKER` |

```text
issuer_specification_path: /Users/codex/Development/surgeist-css/plans/P01-implement-full-css-spec/initiatives/P01-I02-csstree-public-api-corpus-harness.md
```

This document transfers a bounded provider requirement from `surgeist-css` to
the receiving `surgeist-generator` coordinator. It is not a generator
specification, sequence, cycle plan, implementation authorization, or
coordination-plane issuance. The receiving coordinator must verify the evidence,
create and review the required generator-local planning packet, and execute that
packet under the installed `surgeist-agent` workflow.

## 2 Objective

Extend the existing feature-gated CSSTree importer in `surgeist-generator` so
that its public CSS corpus workflow can import the exact pinned CSSTree fixture
tree, including grouped arrays under arbitrary labels, without executing
upstream JavaScript or weakening the generator's deterministic neutral corpus
contract.

The resulting published generator candidate unblocks the CSS-owned
`P01/I02` public-API corpus harness. It does not implement that harness.

## 3 Source Evidence

The consumer specification pins the canonical upstream repository
`https://github.com/csstree/csstree.git` at
`88e3d965c0b1628642a30a841745b410d6835052`. The package identifies itself as
`css-tree` version `3.2.1` under the MIT license. The pinned
`fixtures/ast` tree object is
`bfadc7a7a8d93dce59a27fa7df3bb0f6f6a623d8`.

The exact fixture census is:

| Context | Files | Parsed | Rejected | Total |
| --- | ---: | ---: | ---: | ---: |
| `atrule` | 14 | 116 | 14 | 130 |
| `atrulePrelude` | 1 | 2 | 0 | 2 |
| `block` | 1 | 29 | 0 | 29 |
| `declaration` | 4 | 73 | 4 | 77 |
| `declarationList` | 3 | 19 | 0 | 19 |
| `mediaQuery` | 4 | 43 | 6 | 49 |
| `rule` | 5 | 33 | 0 | 33 |
| `selector` | 21 | 196 | 121 | 317 |
| `selectorList` | 1 | 2 | 8 | 10 |
| `stylesheet` | 4 | 48 | 28 | 76 |
| `value` | 16 | 160 | 33 | 193 |
| **Total** | **74** | **721** | **214** | **935** |

There are 60 top-level array groups containing 297 cases. Twenty-four of those
groups are under labels other than `error` and contain 108 cases. Thirty-four
cases carry parser options using only these observed keys: `atrule`,
`parseAtrulePrelude`, `parseCustomProperty`, `parseRulePrelude`, `parseValue`,
and `property`.

The pinned upstream loader at `lib/__tests/fixture/ast.js` accepts either one
case object or an array of case objects under any top-level label. It determines
the outcome of each individual case by the presence of that case's `error`
member. The enclosing label does not determine outcome.

The existing local evidence checkout is
`/Users/codex/Development/surgeist-css/tmp/csstree`. It is an ignored,
disposable evidence source, not a generator-owned artifact or a required
ordinary-test dependency.

## 4 Observed Provider Gap

At recipient base
`c9d2f61f1f6d39c233907db88073e6c9737ca7ec`,
`src/css/expectation.rs` lines 651 through 689 deserialize:

- a literal top-level `error` value as `Vec<RawErrorCase>`; and
- every other top-level value as one `RawOrdinaryCase`.

That shape handles the existing object fixtures and the existing literal
`error` array, but it cannot decode the pinned corpus's 24 non-`error` grouped
arrays. Consequently, the current importer cannot derive all 935 pinned cases.

The existing neutral expectation contract already retains the information the
CSS consumer needs: source identity, context, source text, canonicalized
options, upstream outcome, canonical CSS when present, disposition, and reason.
The missing capability is grouped-case decoding and stable per-member identity,
not a new AST contract.

## 5 Required Generator Contract

The generator-local plan and implementation must preserve the existing
`css-corpus` feature boundary and the public
`surgeist-css-generate` commands `import-csstree`, `generate`, and
`check-corpus`.

The completed provider contract must:

1. import the exact 74-file pinned tree without executing CSSTree, Node.js, a
   browser, or any other upstream program;
2. derive exactly 935 neutral cases with 721 `parsed` and 214 `rejected`
   outcomes;
3. accept one case object or an array of case objects under every top-level
   label;
4. classify each case from its own shape, including mixed parsed and rejected
   members in one array;
5. assign every grouped member a stable, unique, deterministic case ID;
6. preserve deterministic file ordering, group ordering, member ordering,
   context, source text, options, canonical CSS when present, upstream outcome,
   disposition, and reason;
7. reject duplicate derived identities and malformed fixture shapes with typed
   generator diagnostics;
8. preserve compatibility with the currently supported object fixtures and
   literal `error` arrays;
9. keep `check-corpus` independent of a source checkout, network, browser, and
   upstream executable; and
10. keep canonical neutral bytes and reports reproducible across identical
    imports.

`surgeist-generator` owns and documents the exact grouped-case ID grammar.
`surgeist-css` will consume generated IDs as opaque validated identities and
must not reproduce their derivation.

## 6 Acceptance Evidence

Generator-owned focused tests must prove:

1. object and array values are accepted under arbitrary labels;
2. a grouped array may contain both parsed and rejected cases;
3. outcome classification is per member rather than per enclosing label;
4. stable grouped-member IDs are deterministic, unique, and order-preserving;
5. duplicate derived identities fail explicitly;
6. options remain canonically ordered and semantically preserved;
7. existing object-only and literal `error`-array fixtures remain compatible;
8. repeated import and generation produce byte-identical neutral expectations
   and reports; and
9. the exact pinned checkout produces the 74-file, 935-case, 721/214 census.

The generator coordinator must obtain a clean planning review, task-scoped
worker implementation, separate clean task review, final clean-context holistic
review, logical traceability commits, publication to `origin/main`, and remote
readback under the installed workflow.

## 7 Required Commands

At minimum, the final generator candidate must pass these configured
CSS-focused and repository-wide gates without acquiring missing software:

```sh
RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --features css-corpus
cargo test --locked --offline -p surgeist-generator --features css-corpus
cargo clippy --locked --offline -p surgeist-generator --features css-corpus --all-targets -- -F unsafe-code -D warnings
RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --all-features
cargo test --locked --offline -p surgeist-generator --all-features
cargo clippy --locked --offline -p surgeist-generator --all-features --all-targets -- -F unsafe-code -D warnings
cargo fmt --check
git diff --check
```

The generator-local reviewed cycle plan owns any additional matrix required by
the candidate's committed `AGENTS.md`, manifest, README, and changed surface.
The pinned source checkout may be read only through the already supported
explicit source-root import boundary.

## 8 Ownership And Exclusions

`surgeist-generator` owns fixture decoding, neutral-case identity derivation,
canonical neutral serialization, import verification, generation,
`check-corpus`, focused generator tests, and its published candidate.

This handoff excludes:

- implementing the `surgeist-css` corpus consumer, oracle, probes, or test
  harness;
- committing the CSSTree source checkout or production neutral corpus in
  `surgeist-generator`;
- importing upstream AST objects, diagnostic prose, JavaScript error classes,
  or source-locator internals into the neutral schema;
- executing or adding a runtime dependency on CSSTree, Node.js, a browser, or
  network access;
- changing the layout generator;
- changing sibling repositories during generator implementation;
- adding a production dependency unless a reviewed generator-local design
  establishes that it is necessary and the user separately authorizes any
  external acquisition; and
- adding or permitting executable `unsafe` code.

The expected API classification is internal behavior compatible with the
existing feature-gated CSS corpus front door. The generator-local plan must
explicitly classify any discovered public or schema break before
implementation.

## 9 Return Contract

A successful `CRATE_CANDIDATE` report to `surgeist-css` must include:

1. the full generator commit SHA published and verified on remote `main`;
2. the reviewed generator specification, sequence, and cycle paths with exact
   semantic revisions;
3. the exact grouped-case ID grammar and neutral schema compatibility result;
4. the exact focused and final commands with pass results;
5. task-review and clean-context holistic-review results;
6. the exact 74-file, 935-case, 721/214 import census;
7. confirmation that ordinary `check-corpus` needs no source checkout,
   upstream executable, browser, or network;
8. public API, dependency, feature, generated-artifact, license, and unsafe
   classifications; and
9. any nonblocking observations that are genuinely outside the reviewed
   generator scope.

A `BLOCKER` report must name the exact failing file, command or fixture,
observed result, expected result, impact on clause 5, and the owner of the next
action. It must not silently narrow the corpus or reclassify lost cases as
unsupported.

## 10 Readiness And Next Transition

This handoff artifact is prepared but is not yet an issued
`READY_TO_START` directive because its issuer specification commit
`4b288d6467d91f2fc33eac78ef0b0b725154195d` is currently local to
`surgeist-css`; the observed issuer tracking ref remains
`5ab2461adae33b5cba2c29154929a89f61c8e303`.

The handoff becomes eligible for issuance only after:

1. the issuer specification commit is published and read back as fetchable from
   the configured `surgeist-css` authority remote;
2. this receiving artifact is published and read back as fetchable from the
   configured `surgeist-generator` authority remote;
3. the receiving coordinator independently verifies clauses 3 and 4 against
   the pinned source and current generator base; and
4. the receiving coordinator creates a reviewed generator-local initiative,
   sequence when required, and current cycle plan under `P01`.

No generator implementation or CSS harness implementation is authorized by
this prepared artifact alone.
