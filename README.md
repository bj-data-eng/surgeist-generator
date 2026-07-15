# surgeist-generator

Surgeist generator tooling and generation contracts belong to this leaf; root
`surgeist` retains cross-crate integration, gitlinks, and root API artifacts.

This commit is only the scaffold. No layout generator code, fixtures, scripts,
corpus data, or generated artifacts have moved yet.

## Checks

```sh
cargo test --offline
cargo clippy --offline --all-targets -- -F unsafe-code -D warnings
cargo fmt --check
```
