use std::collections::BTreeMap;
use std::fs;

#[test]
fn package_metadata_and_driver_feature_boundaries_are_exact() {
    let manifest_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let manifest = fs::read_to_string(manifest_path).expect("read Cargo.toml");
    let value: toml::Value = toml::from_str(&manifest).expect("parse Cargo.toml");

    let package = value["package"].as_table().expect("package table");
    assert_eq!(package["name"].as_str(), Some("surgeist-generator"));
    assert_eq!(package["version"].as_str(), Some("0.1.0"));
    assert_eq!(package["edition"].as_str(), Some("2024"));
    assert_eq!(package["rust-version"].as_str(), Some("1.97"));
    assert_eq!(package["license"].as_str(), Some("MIT"));

    let features = value["features"].as_table().expect("features table");
    assert_eq!(features.len(), 3);
    assert!(features["default"].as_array().is_some_and(Vec::is_empty));
    assert!(features["css-corpus"].as_array().is_some_and(Vec::is_empty));
    assert_eq!(
        features["layout-browser"]
            .as_array()
            .expect("layout feature")
            .iter()
            .map(|entry| entry.as_str().expect("feature entry"))
            .collect::<Vec<_>>(),
        ["dep:chromiumoxide", "dep:futures", "dep:tokio", "dep:url"]
    );

    let bins = value["bin"].as_array().expect("binary targets");
    let actual_bins = bins
        .iter()
        .map(|bin| {
            let bin = bin.as_table().expect("binary table");
            let required = bin["required-features"]
                .as_array()
                .expect("required features");
            assert_eq!(required.len(), 1);
            (
                bin["name"].as_str().expect("binary name"),
                (
                    bin["path"].as_str().expect("binary path"),
                    required[0].as_str().expect("required feature"),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(actual_bins.len(), 2);
    assert_eq!(
        actual_bins["surgeist-css-generate"],
        ("src/bin/surgeist-css-generate.rs", "css-corpus")
    );
    assert_eq!(
        actual_bins["surgeist-layout-generate"],
        ("src/bin/surgeist-layout-generate.rs", "layout-browser")
    );

    let dependencies = value["dependencies"]
        .as_table()
        .expect("dependencies table");
    let optional = dependencies
        .iter()
        .filter_map(|(name, dependency)| {
            dependency
                .as_table()
                .and_then(|table| table.get("optional"))
                .and_then(toml::Value::as_bool)
                .is_some_and(|value| value)
                .then_some(name.as_str())
        })
        .collect::<Vec<_>>();
    assert_eq!(optional, ["chromiumoxide", "futures", "tokio", "url"]);

    let targets = value["target"].as_table().expect("target table");
    assert_eq!(targets.len(), 1);
    let apple_silicon =
        targets["cfg(all(target_os = \"macos\", target_arch = \"aarch64\"))"]["dependencies"]
            .as_table()
            .expect("Apple-Silicon dependencies");
    assert_eq!(apple_silicon.len(), 1);
    let rustix = apple_silicon["rustix"]
        .as_table()
        .expect("target rustix dependency");
    assert_eq!(rustix["version"].as_str(), Some("=1.1.4"));
    assert_eq!(
        rustix["features"]
            .as_array()
            .expect("rustix features")
            .iter()
            .map(|entry| entry.as_str().expect("rustix feature"))
            .collect::<Vec<_>>(),
        ["fs", "process"]
    );
}
