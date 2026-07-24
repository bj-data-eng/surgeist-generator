VERDICT: NOT CLEAN

SCOPE: Whole-repository interruption baseline at `1e545530fb992e2e749f26a1ba7ac0d77ed85dd6` on `main`; this is a current-state review, not an approval of any historical implementation range or an implicit reactivation of deleted planning artifacts.

EVIDENCE CHECKED: `AGENTS.md`, `Cargo.toml`, `Cargo.lock`, `README.md`, every tracked file under `src/` and `tests/`, current Git status/history, the deleted C01 packet as historical interruption evidence, Cargo target metadata, the configured native/default/all-feature and WASM checks, Clippy, rustfmt, focused test reruns, owned-Rust unsafe scan, and the preserved layout-prefix digest.

FINDINGS:

[Important] Rooted coordination rejects its own newly created lease tree
Location: `src/core/coordination.rs:280-292`; `src/core/fs.rs:1297-1305`; affected tests in `src/core/lease.rs:234-376` and `src/core/artifact.rs:390-537`
Evidence: `cargo test --offline -p surgeist-generator` and the all-feature run each failed 10 of 62 unit tests. Every failure stops during lease acquisition with `InvalidPath: validate exact rooted entry name: entry is absent or aliased: leases`. The exact focused lease test reproduces the same failure before reaching its contention assertion. The five artifact-policy tests and five lease/check tests therefore do not exercise their intended behavior.
Impact: The Apple-Silicon macOS mutation path cannot acquire the coordination authority needed for artifact installation or shared/exclusive checks. The early common failure also masks any later defects in transaction installation and recovery.
Required remediation: Correct the rooted exact-name/traversal behavior for a just-created exact `leases` entry, add a focused regression at that boundary, then rerun every lease and artifact test before relying on later transaction results.

[Important] Crash-safety and bootstrap tests model their claims instead of exercising recovery code
Location: `src/core/transaction.rs:35-187` and `src/core/transaction.rs:1489-1547`; `src/core/coordination.rs:50-113` and `src/core/coordination.rs:2130-2152`
Evidence: `TransactionProtocol::crash_prefixes` produces prefixes of a test-only hard-coded step list, while `CrashPrefix::recover` decides old/new visibility solely by whether that list contains `Commit`; it never invokes `TransactionEngine`, persists a journal, injects a failure, or calls `recover_all`. `RecoveredPrefix::one_complete_generation` is tautological because its enum has only `Old` and `New`. The bootstrap test likewise checks ordering in a test-only array rather than the bootstrap/recovery implementation. Live artifact tests currently fail before entering the engine.
Impact: The durable transaction, cleanup, and restart-recovery implementation can diverge from the model while tests named for “every crash prefix” remain green. Core data-loss and residue-handling guarantees therefore lack executable proof.
Required remediation: Add deterministic failure injection through the real rooted coordination and `TransactionEngine` paths at each durable boundary, assert actual filesystem visibility and resumable evidence after recovery, and retain model-only tests only as supplementary ordering checks.

[Important] The crate has no usable domain generator or feature-gated command surface
Location: `Cargo.toml:15-18`; `src/lib.rs:5-13`; `src/core/mod.rs:21-49`; `src/layout/legacy_generator.rs:1-32`
Evidence: Cargo metadata reports one library and two integration-test targets, with no binaries. `layout-browser` and `css-corpus` activate nothing. `lib.rs` does not declare a layout or CSS module, and the lease/artifact/domain front doors are crate-private. The preserved legacy layout source is not in the module graph, imports undeclared `chromiumoxide` and `futures` crates, and references assets absent from this repository. `private_front_doors_are_linked` takes artificial function references from unrelated identifier validation to suppress dead-code evidence without creating a product caller.
Impact: Consumers can use the shared value/source contracts, but cannot run a layout or CSS generator or invoke the new publication layer. The crate is an interrupted shared-core extraction, not a complete generator product.
Required remediation: Establish a newly reviewed desired-state contract from this baseline, then wire real feature-owned modules and command targets to the shared core (or explicitly remove/defer a declared feature). Remove the artificial linkage once real callers own the private front doors, and retire the preserved prefix only after its required behavior is represented and tested.

[Minor] The configured quality matrix is not clean
Location: `src/core/coordination.rs:1306-1308`; `src/core/fs.rs:1`, `src/core/fs.rs:1209`, `src/core/fs.rs:1391`, and `src/core/fs.rs:1405`; rustfmt deltas across `src/core/{artifact,coordination,fs,inventory,lease,transaction}.rs`
Evidence: `cargo clippy --offline -p surgeist-generator --all-targets -- -F unsafe-code -D warnings` fails on an identity `map_err`. `cargo fmt --check` reports broad formatting deltas in all six interrupted core files. The installed `wasm32-unknown-unknown` target check exits successfully but emits one unused-import and three dead-code warnings from `fs.rs`.
Impact: The repository does not satisfy its documented Clippy/format gates, and the portable compile is not warning-clean.
Required remediation: After the behavioral correction is task-clean, remove the identity mapping and target-specific unused/dead code, apply rustfmt, and rerun the locked/offline native and WASM matrix with warnings denied where configured.

[Minor] Repository guidance describes a scaffold that no longer matches the tree
Location: `README.md:5-7`; `AGENTS.md:50-55`
Evidence: Both files say no layout generator code has moved and the README calls the commit “only the scaffold.” The tracked tree now contains the exact 4,626-line preserved layout prefix plus roughly 13,000 lines of shared/core implementation. Neither document explains the interrupted state, inert features, absent binaries, or the Apple-Silicon-only trusted-Git/mutation behavior visible in source.
Impact: Maintainers and consumers cannot determine the actual completeness, supported behavior, or valid verification boundary from the committed front-door documentation.
Required remediation: Once the new planning packet fixes the intended current boundary, update README and repository guidance to describe the implemented shared contracts, explicitly unfinished driver work, target support, feature behavior, and authoritative checks without claiming later-cycle completion.

[Minor] The public exit-code test contains a tautology instead of checking the CLI contract
Location: `tests/public_api.rs:325-327`; `src/error.rs:71-87`
Evidence: The test verifies an `InvalidManifest` error exits with `1`, then compares `GeneratorErrorKind::Cli as u8` with itself. It never constructs or observes a CLI-kind error and therefore does not test the documented `64` mapping.
Impact: A regression in CLI error classification or exit-code mapping can pass the public-contract suite, especially while no binary target exercises the boundary.
Required remediation: Exercise a real CLI-kind error through the eventual command front door, or add a focused crate-owned error test that proves the `Cli -> 64` mapping until the binary exists.

OBSERVATIONS:

- The shared public contract layer remains useful evidence: native checking succeeds, and the focused `public_api` and `shared_contracts` suites pass all 11 tests.
- `src/layout/legacy_generator.rs` still matches the preserved migration digest `d2f5ca87cea6b36826e9172e2d7ba7a99196c375e2ca53f8a84a075200e70a9f`; later work should preserve that provenance until behavior is represented.
- The complete owned-Rust scan over `src/` and `tests/` found no executable `unsafe` construct.
