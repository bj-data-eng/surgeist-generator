//! Generator tooling and generation contracts for Surgeist.
//!
//! This crate owns the generator boundary intended to receive the current
//! layout-owned generator in a later, separately planned migration. No generator
//! code, fixtures, or generated artifacts have moved into this scaffold.

#![forbid(unsafe_code)]

/// Crate identity string used by smoke tests.
pub const CRATE_NAME: &str = "surgeist-generator";

#[cfg(test)]
mod tests {
    use super::CRATE_NAME;

    #[test]
    fn exposes_crate_identity() {
        assert_eq!(CRATE_NAME, "surgeist-generator");
    }
}
