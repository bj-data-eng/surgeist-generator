#![cfg(feature = "layout-browser")]

use std::fs;

#[test]
fn layout_browser_feature_has_the_exact_optional_dependency_edge() {
    let manifest_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = fs::read_to_string(manifest_path).expect("read Cargo.toml");
    let value: toml::Value = toml::from_str(&manifest).expect("parse Cargo.toml");
    let feature = value["features"]["layout-browser"]
        .as_array()
        .expect("layout-browser feature array")
        .iter()
        .map(|entry| entry.as_str().expect("feature string"))
        .collect::<Vec<_>>();
    assert_eq!(
        feature,
        ["dep:chromiumoxide", "dep:futures", "dep:tokio", "dep:url"]
    );

    let dependencies = value["dependencies"].as_table().expect("dependency table");
    for (name, version, features, default_features) in [
        ("chromiumoxide", "=0.9.1", &["bytes"][..], Some(false)),
        ("futures", "=0.3.31", &[][..], None),
        (
            "tokio",
            "=1.48.0",
            &[
                "fs",
                "io-util",
                "macros",
                "process",
                "rt-multi-thread",
                "sync",
                "time",
            ][..],
            None,
        ),
        ("url", "=2.5.7", &[][..], None),
    ] {
        let dependency = dependencies[name]
            .as_table()
            .unwrap_or_else(|| panic!("{name} dependency table"));
        assert_eq!(dependency["version"].as_str(), Some(version), "{name}");
        assert_eq!(dependency["optional"].as_bool(), Some(true), "{name}");
        assert_eq!(
            dependency
                .get("default-features")
                .and_then(toml::Value::as_bool),
            default_features,
            "{name}"
        );
        let actual_features = dependency
            .get("features")
            .and_then(toml::Value::as_array)
            .map(|entries| {
                entries
                    .iter()
                    .map(|entry| entry.as_str().expect("dependency feature"))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        assert_eq!(actual_features, features, "{name}");
    }
}

#[test]
fn layout_license_policy_is_exact_and_has_no_bypasses() {
    let policy_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("deny.toml");
    let policy = fs::read_to_string(policy_path).expect("read deny.toml");
    let value: toml::Value = toml::from_str(&policy).expect("parse deny.toml");
    assert_eq!(value.as_table().expect("policy table").len(), 1);
    let licenses = value["licenses"].as_table().expect("licenses table");
    assert_eq!(licenses.len(), 2);
    assert_eq!(licenses["confidence-threshold"].as_float(), Some(0.8));
    let actual = licenses["allow"]
        .as_array()
        .expect("license allowlist")
        .iter()
        .map(|entry| entry.as_str().expect("license string"))
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        [
            "0BSD",
            "Apache-2.0",
            "Apache-2.0 WITH LLVM-exception",
            "BSD-2-Clause",
            "BSD-3-Clause",
            "BSL-1.0",
            "CC0-1.0",
            "ISC",
            "MIT",
            "MPL-2.0",
            "OpenSSL",
            "Unicode-3.0",
            "Unicode-DFS-2016",
            "Unlicense",
            "Zlib",
        ]
    );
}
