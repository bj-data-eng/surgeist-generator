# Architecture and trust

The crate separates shared corpus infrastructure from fixture semantics. Its
CSS adapter owns CSSTree import and neutral expectations. Its browser engine
executes caller-prepared jobs and publishes caller-produced bytes. Callers own
fixture semantics, measurement protocols, and serialization; the engine owns
its runtime, browser, processes, and publication transactions.

The engine captures sources, global inputs, declared resources, and attestations
before measurement, then revalidates them before publication. Resource errors
are deferred until a fixture produces an artifact; all-unsupported fixtures
retain their resource bypass. Concurrent input mutation invalidates the run.
This is snapshot/revalidation protection, not browser request interception.

A full run verifies declared accounting and publishes artifacts/reports together.
Filtered generation writes selected artifacts without rewriting full reports or
pruning unrelated outputs. Shared input, provenance, accounting, or publication
failures do not partially install a new corpus. Per-fixture preparation, lowering,
measurement, and resource failures in a full run publish diagnostic accounting
and retain outputs that were not regenerated;
a later full run can repair that diagnostic state. A private transactional
ownership ledger permits repair after filtered or diagnostic generation without
claiming the unchanged full report is current provenance.
The browser-specific schema-4 report records executable/engine identity, inputs,
source attestations, browser settings, artifact hashes, and outcome accounting.
CSS report schemas and serialization are unchanged.

Legacy reports may supply checked `PriorOwnership` for one full migration. Their
paths and hashes are independently verified and protected through publication.
They cannot authorize filtered migration or pass current-report verification.

## Roots, acquisition, and recovery

`CorpusLocation` retains its contained-owner invariant. `BrowserLocation` binds
an exact browser owner and exact corpus separately, supporting caller-owned
corpora outside the browser owner without widening either root to a common
ancestor. Cache paths must remain under their declared owner and outside generated
outputs and recovery directories. Publication authority remains restricted to the exact corpus.

Acquisition is explicit. Managed source/browser operations verify complete cache
entries, serialize access to a pin, stage missing acquisitions, and promote a
complete entry atomically. Existing-only operations never fetch or install.
Use these owner-relative locations:

- `tmp/surgeist-sources/<source-id>/<full-revision>/`
- `tmp/surgeist-browser/`, with separate browser-version entries.

Repository `tmp/` is ignored and survives normal `cargo clean`. Acquisition
staging and locks live beside the cache entries. Profile and publication recovery
do not remove completed acquisitions; cache deletion is separate maintenance.

Source import declarations bind a repository/revision, source tree, exclusions,
expected source count, and preserved authored files. Import verifies the exact
checkout and atomically publishes imported bytes plus a generic attestation.
Adoption of existing imports verifies their bytes before attestation. Ordinary
corpus checks verify persisted attestations offline without a checkout; explicit
source checks also verify the existing pinned checkout.

Browser execution authenticates the selected cache/executable identity, regular
executable shape, single-link status, SHA-256 digest, version output, and pinned
launch switches before launch and again at close. These checks authenticate a
trusted capability; they do not sandbox the browser. Lifecycle guarantees cover
the recorded inherited process group, and path-based process spawn is not atomic
execution from the held file descriptor.

Version and measurement processes receive a cleared, fixed environment. Private
profile directories supply `HOME`, temporary directories, and XDG homes;
`PATH=/usr/bin:/bin`, `TZ=UTC`, `LANG=C`, and `LC_ALL=C`. Proxy variables are
cleared and `NO_PROXY=*`. Caller environment entries do not reach the browser.

Coordination, transactions, and durable browser-profile journals live beneath
the corpus's `.surgeist-generator` directory. Callers keep this machine-local
directory ignored by Git; it is not part of a portable corpus checkout.
Completion, errors, and panics
terminalize the recorded process group before publication. Recovery never signals
a process: live, reused, permission-inconclusive, corrupt, or identity-drifted
evidence is preserved and reported. Historical layout journals are not silently
ignored by the new browser namespace. CSS coordination behavior is unchanged.

## Repository ownership

This independent leaf owns its manifest, implementation, public API, focused
tests, and documentation. It commits no production corpora, generated
expectations, acquired source trees, or browser installations. Root `surgeist`
owns cross-crate composition, adapters, integration tests, this leaf's gitlink,
and the only API generator and generated API audit artifacts.

See the [reference](reference.md) for interfaces and compatibility, and
[how-to guide](how-to.md) for operations.
