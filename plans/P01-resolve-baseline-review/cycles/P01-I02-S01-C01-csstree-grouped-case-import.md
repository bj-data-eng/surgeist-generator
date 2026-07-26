# P01-I02-S01-C01 CSSTree Grouped-Case Import

## 1 Header

- Cycle path: `P01/I02/S01/C01`
- Owning repository: `/Users/codex/Development/surgeist-generator`
- Status: `reviewed`
- Cycle base: `a91f1cce79a44a6f031f6c3a767c01504f79c7b4`
- Reviewed specification: `P01/I02/V01`,
  `plans/P01-resolve-baseline-review/initiatives/P01-I02-csstree-grouped-case-import.md`,
  SHA-256
  `4b161b384b565f03a0e3fff8c9f57e4a9086279ae85f7b6de22292c87d5d0e38`,
  review `CLEAN`.
- Sequence: telemetry-only `P01/I02/S01`, bound to `P01/I02/V01`; no sequence
  document is required for this one-cycle initiative.
- Applicable specification clauses: `3.2`, `3.3`, `4`, `5`, `6`, `7`, `8`,
  `9`, and `10`.
- Bounded outcome: import arbitrary CSSTree singleton and grouped cases with
  stable IDs and loader-equivalent member outcomes while preserving legacy
  bytes and proving the exact pinned 74/721/214/935 census.

## 2 Outcome And Non-Goals

Generalize the private CSSTree fixture decoder to accept singleton objects or
arrays beneath every top-level label while preserving all previously accepted
neutral bytes. Newly accepted members follow the pinned loader's per-member
JavaScript-truthiness rule, receive stable escaped IDs, and pass the exact
74-file, 721-parsed, 214-rejected, 935-total pinned acceptance.

This cycle does not change public APIs, commands, features, dependencies,
schemas, reports, source verification, import publication, corpus acquisition,
layout behavior, production corpora, sibling repositories, root integration,
or generated audit artifacts. It adds no executable `unsafe` and runs no
ignored test body.

## 3 Boundary And Resolved Decisions

- Change only `src/css/expectation.rs`, focused tests in `src/css/tests.rs`, and
  directly affected CSS-facing documentation if implementation makes a
  clarification necessary.
- Preserve singleton IDs as `<path>#/<escaped-label>`. Derive array-member IDs
  as `<path>#/<escaped-label>/<zero-based-index>`, using the existing JSON
  Pointer token escaping.
- Preserve the baseline literal-`error` array branch exactly: source-only
  members remain accepted as rejected cases; labels, options, and canonical CSS
  remain omitted; all other members remain ignored.
- Treat a singleton under label `error` as an ordinary singleton. Treat every
  case outside the legacy array branch as rejected only when its `error` value
  is JavaScript-truthy. JSON `null`, `false`, positive or negative zero, and
  empty strings are falsy; nonzero numbers, nonempty strings, arrays, and
  objects are truthy.
- Require `ast` for every non-legacy parsed member. Preserve shape-valid options
  and generated CSS for non-legacy parsed or rejected members.
- Preserve duplicate-member and trailing-value rejection, final ID sorting,
  manifest override binding, case-count validation, atomic publication, and
  checkout-free `check-corpus`.
- Use only already-present offline tooling and the existing read-only checkout
  `/Users/codex/Development/surgeist-css/tmp/csstree` at revision
  `88e3d965c0b1628642a30a841745b410d6835052`.

## 4 Impacts

| Concern | Cycle classification |
| --- | --- |
| Public Rust API and CLI | Unchanged |
| Features and dependencies | Unchanged |
| Manifest, neutral, and report schemas | Unchanged schema versions |
| Existing accepted fixture bytes | Byte-compatible |
| Accepted upstream input | Additive grouped-case support |
| Generated repository artifacts | None |
| Documentation | Only if current behavior text becomes inaccurate |
| MSRV and platform | Preserve repository values |
| Root and sibling work | Future consumer adoption only |
| Safety | No executable `unsafe` |
| Anticipated SemVer | Compatible behavior correction; final initiative versioning owns the applied classification |

## 5 Task

### 5.1 P01-I02-S01-C01-T01 Implement Grouped-Case Decoding

**Files and outcome**

Own `src/css/expectation.rs`, focused `src/css/tests.rs` coverage, and only
directly necessary CSS documentation. Replace the special object-only ordinary
decoder with a private group/member model that implements clause `3` without
changing public or persisted schemas.

**RED evidence**

Before production edits, add focused front-door tests:

1. `css_expectation_grouped_cases_follow_member_truthiness_and_stable_ids`
   is RED because the baseline rejects arbitrary array labels. It covers mixed
   outcomes, every JSON truthiness class, options/generate preservation, escaped
   labels, deterministic indexed IDs, and an empty group beside another valid
   case.
2. `css_expectation_literal_error_arrays_preserve_legacy_bytes` proves
   source-only acceptance and byte-identical omission when ignored metadata is
   present. Run it before production edits as a baseline-green characterization,
   not RED evidence.
3. `css_expectation_error_singleton_uses_ordinary_semantics` is RED because the
   baseline routes label `error` through the legacy array decoder. It proves
   truthy and falsy outcomes, the ordinary singleton ID and label, and
   shape-valid metadata.

Run and retain the two expected failures and one characterization pass:

```sh
cargo test --locked --offline -p surgeist-generator --features css-corpus --lib css_expectation_grouped_cases_follow_member_truthiness_and_stable_ids
cargo test --locked --offline -p surgeist-generator --features css-corpus --lib css_expectation_literal_error_arrays_preserve_legacy_bytes
cargo test --locked --offline -p surgeist-generator --features css-corpus --lib css_expectation_error_singleton_uses_ordinary_semantics
```

**Acceptance**

- All three RED predicates pass through the real generate front door after the
  implementation.
- Existing expectation goldens remain byte-identical and existing malformed,
  duplicate, count-mismatch, repeat-generation, report, and `check-corpus`
  tests remain green.
- An empty array derives no cases and remains accepted when another group yields
  a valid case. A complete fixture deriving no cases, top-level scalars, nested
  arrays, malformed source/options/generate values, missing `ast` on parsed
  members, and duplicate IDs fail as `InvalidInventory` before publication.
- The exact pinned checkout imports 74 fixtures and generates 935 unique neutral
  cases: 721 parsed and 214 rejected. Repeated generation leaves expectation and
  report bytes identical, and `check-corpus` passes after the checkout path is no
  longer supplied.
- No production corpus or temporary acceptance root remains.

**Task commands**

Run the three focused commands above, then:

```sh
cargo test --locked --offline -p surgeist-generator --features css-corpus --lib css_expectation_
RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --features css-corpus
cargo test --locked --offline -p surgeist-generator --features css-corpus
RUSTFLAGS="-D warnings" cargo check --locked --offline -p surgeist-generator --all-features
cargo test --locked --offline -p surgeist-generator --all-features
cargo test --locked --offline -p surgeist-generator --all-features -- --ignored --list
cargo fmt --check
git diff --check
git diff --check a91f1cce79a44a6f031f6c3a767c01504f79c7b4..HEAD
```

Run the complete Clippy matrix with `clippy::too_many_lines` explicitly enabled
at threshold 100 through a disposable configuration:

```zsh
set -eu
clippy_conf_dir="$(mktemp -d "${TMPDIR:-/tmp}/surgeist-generator-clippy.XXXXXX")"
trap 'rm -rf "$clippy_conf_dir"' EXIT
printf '%s\n' 'too-many-lines-threshold = 100' >"$clippy_conf_dir/clippy.toml"

CLIPPY_CONF_DIR="$clippy_conf_dir" cargo clippy --locked --offline -p surgeist-generator --no-default-features --all-targets -- -F unsafe-code -W clippy::too_many_lines -D warnings
CLIPPY_CONF_DIR="$clippy_conf_dir" cargo clippy --locked --offline -p surgeist-generator --features layout-browser --all-targets -- -F unsafe-code -W clippy::too_many_lines -D warnings
CLIPPY_CONF_DIR="$clippy_conf_dir" cargo clippy --locked --offline -p surgeist-generator --features css-corpus --all-targets -- -F unsafe-code -W clippy::too_many_lines -D warnings
CLIPPY_CONF_DIR="$clippy_conf_dir" cargo clippy --locked --offline -p surgeist-generator --all-features --all-targets -- -F unsafe-code -W clippy::too_many_lines -D warnings

rm -rf "$clippy_conf_dir"
trap - EXIT
```

Perform the bounded pinned acceptance with this complete disposable setup and
procedure:

```zsh
set -eu
acceptance_root="$(mktemp -d "${TMPDIR:-/tmp}/surgeist-generator-csstree.XXXXXX")"
trap 'rm -rf "$acceptance_root"' EXIT
acceptance_owner="$acceptance_root/owner"
acceptance_corpus="$acceptance_owner/corpus"
source_checkout="/Users/codex/Development/surgeist-css/tmp/csstree"
mkdir -p "$acceptance_corpus"

test "$(git -C "$source_checkout" rev-parse HEAD)" = 88e3d965c0b1628642a30a841745b410d6835052
test "$(git -C "$source_checkout" rev-parse HEAD:fixtures/ast)" = bfadc7a7a8d93dce59a27fa7df3bb0f6f6a623d8

printf '%s\n' \
  'schema_version = 1' \
  '' \
  '[source]' \
  'kind = "csstree"' \
  'repository = "https://github.com/csstree/csstree.git"' \
  'revision = "88e3d965c0b1628642a30a841745b410d6835052"' \
  'fixture_root = "fixtures/ast"' \
  'import_root = "source"' \
  'expected_files = 74' \
  'expected_cases = 935' \
  '' \
  '[artifacts]' \
  'expectation_root = "expectations"' \
  'report_file = "expectations/generation-reports/all.json"' \
  >"$acceptance_corpus/corpus.toml"

cargo run --locked --offline -p surgeist-generator --features css-corpus --bin surgeist-css-generate -- --owner-root "$acceptance_owner" --corpus-root "$acceptance_corpus" import-csstree --source-root "$source_checkout"
cargo run --locked --offline -p surgeist-generator --features css-corpus --bin surgeist-css-generate -- --owner-root "$acceptance_owner" --corpus-root "$acceptance_corpus" generate

test "$(find "$acceptance_corpus/expectations" -type f -name '*.json' ! -path '*/generation-reports/*' | wc -l | tr -d ' ')" = 74
test "$(rg -o '"upstream_outcome": "parsed"' "$acceptance_corpus/expectations" | wc -l | tr -d ' ')" = 721
test "$(rg -o '"upstream_outcome": "rejected"' "$acceptance_corpus/expectations" | wc -l | tr -d ' ')" = 214
test "$(rg -o '"id": "[^"]+"' "$acceptance_corpus/expectations" | LC_ALL=C sort -u | wc -l | tr -d ' ')" = 935

find "$acceptance_corpus/expectations" -type f -print0 | LC_ALL=C sort -z | xargs -0 shasum -a 256 >"$acceptance_root/first.sha256"
cargo run --locked --offline -p surgeist-generator --features css-corpus --bin surgeist-css-generate -- --owner-root "$acceptance_owner" --corpus-root "$acceptance_corpus" generate
find "$acceptance_corpus/expectations" -type f -print0 | LC_ALL=C sort -z | xargs -0 shasum -a 256 >"$acceptance_root/second.sha256"
cmp "$acceptance_root/first.sha256" "$acceptance_root/second.sha256"

unset source_checkout
cargo run --locked --offline -p surgeist-generator --features css-corpus --bin surgeist-css-generate -- --owner-root "$acceptance_owner" --corpus-root "$acceptance_corpus" check-corpus

rm -rf "$acceptance_root"
trap - EXIT
```

Record the exact assertions and successful SHA-256 inventory comparison. Do not
run any ignored test body.

Build an explicit manifest of every tracked or non-ignored untracked
Surgeist-owned Rust file, print it for evidence, and require the canonical
textual unsafe scan to find no match:

```zsh
set -eu
owned_rust_manifest="$(mktemp)"
trap 'rm -f "$owned_rust_manifest"' EXIT
git ls-files -co --exclude-standard -z -- '*.rs' ':(exclude)vendor/**' ':(exclude)target/**' >"$owned_rust_manifest"
LC_ALL=C sort -zu "$owned_rust_manifest" -o "$owned_rust_manifest"
test -s "$owned_rust_manifest"
tr '\0' '\n' <"$owned_rust_manifest"
unsafe_pattern='#\s*\[\s*(?:unsafe\s*\(|no_mangle\b|export_name\b)|\bunsafe\s*(?:\{|fn\b|trait\b|impl\b|extern\b)|\bstatic\s+mut\b|\bextern\s*(?:"[^"]*")?\s*\{'
while IFS= read -r -d '' rust_path; do
  if rg -n --pcre2 "$unsafe_pattern" -- "$rust_path"; then
    exit 1
  else
    scan_exit=$?
    test "$scan_exit" -eq 1
  fi
done <"$owned_rust_manifest"
rm -f "$owned_rust_manifest"
trap - EXIT
```

**Dependency**

The published and clean-reviewed `P01/I02/V01` specification and this reviewed
cycle plan.

**Intended commit**

`feat(css): import grouped CSSTree cases`

## 6 Completion

The exact final rerun set on the completed candidate is:

1. the task-command block in clause `5.1` beginning with the aggregate
   `css_expectation_` test and ending with the exact
   `a91f1cce79a44a6f031f6c3a767c01504f79c7b4..HEAD` diff check;
2. the complete four-command disposable-configuration Clippy block in clause
   `5.1`;
3. the complete pinned-acceptance setup, two generation passes, census and
   uniqueness assertions, SHA-256 comparison, checkout-free check, and cleanup
   block in clause `5.1`; and
4. the complete owned-Rust manifest and textual unsafe-scan block in clause
   `5.1`.

The cycle is accepted when T01 has one clean independent task review, that exact
final rerun set passes on the task head, and a fresh holistic reviewer returns
`CLEAN` for the complete cycle range. Follow the installed canonical review,
landing, remote-readback, telemetry-review, and cycle-closure contracts rather
than copying them here.

The published candidate handoff reports the full remote `main` SHA, reviewed
planning revisions, exact ID and truthiness rules, compatibility result,
commands, task and holistic verdicts, 74/721/214/935 census, checkout-free
checking result, dependency/API/safety classifications, and genuine
nonblocking observations.

Block on source revision/tree drift, inability to preserve legacy bytes or IDs,
missing already-present offline tooling, acquisition pressure, executable
`unsafe`, census mismatch, or any review finding. Report the exact failing path
or command, observed and expected result, contract impact, and next-action
owner.
