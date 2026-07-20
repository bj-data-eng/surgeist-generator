use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    GeneratorError, GeneratorErrorKind, RelativePath, Result, Sha256Digest, SourceRevision,
};

const GENERATOR: &str = "surgeist-layout-generate";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Variant {
    BorderBoxLtr,
    BorderBoxRtl,
    ContentBoxLtr,
    ContentBoxRtl,
}

impl Variant {
    pub(super) const ALL: [Self; 4] = [
        Self::BorderBoxLtr,
        Self::BorderBoxRtl,
        Self::ContentBoxLtr,
        Self::ContentBoxRtl,
    ];

    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::BorderBoxLtr => "border_box_ltr",
            Self::BorderBoxRtl => "border_box_rtl",
            Self::ContentBoxLtr => "content_box_ltr",
            Self::ContentBoxRtl => "content_box_rtl",
        }
    }

    pub(super) const fn browser_key(self) -> &'static str {
        match self {
            Self::BorderBoxLtr => "borderBoxLtrData",
            Self::BorderBoxRtl => "borderBoxRtlData",
            Self::ContentBoxLtr => "contentBoxLtrData",
            Self::ContentBoxRtl => "contentBoxRtlData",
        }
    }

    pub(super) fn output_path(self, source: &RelativePath) -> Result<RelativePath> {
        let relative = source
            .as_str()
            .strip_prefix("html/")
            .and_then(|value| value.strip_suffix(".html"))
            .ok_or_else(|| generation_error("layout source is not html/<path>.html"))?;
        RelativePath::new(format!("xml/{relative}__{}.xml", self.name()))
    }

    pub(super) fn test_name(self, source: &RelativePath) -> Result<String> {
        let stem = source
            .as_str()
            .rsplit('/')
            .next()
            .and_then(|value| value.strip_suffix(".html"))
            .ok_or_else(|| generation_error("layout source has no .html fixture stem"))?;
        Ok(format!("{stem}__{}", self.name()))
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MeasuredLayout {
    #[serde(default)]
    pub(super) viewport_width: f64,
    #[serde(default)]
    pub(super) viewport_height: f64,
    #[serde(default = "default_input")]
    pub(super) input: MeasuredNode,
    #[serde(default)]
    pub(super) expectations: Vec<MeasuredBox>,
}

impl MeasuredLayout {
    #[cfg(test)]
    fn zero() -> Self {
        Self {
            viewport_width: 0.0,
            viewport_height: 0.0,
            input: default_input(),
            expectations: vec![MeasuredBox::default()],
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MeasuredNode {
    #[serde(default = "default_tag")]
    pub(super) tag: String,
    #[serde(default)]
    pub(super) attributes: BTreeMap<String, String>,
    #[serde(default)]
    pub(super) style: BTreeMap<String, String>,
    #[serde(default)]
    pub(super) text: Option<String>,
    #[serde(default)]
    pub(super) children: Vec<MeasuredNode>,
}

fn default_tag() -> String {
    "div".to_owned()
}

fn default_input() -> MeasuredNode {
    MeasuredNode {
        tag: default_tag(),
        attributes: BTreeMap::new(),
        style: BTreeMap::new(),
        text: None,
        children: Vec::new(),
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MeasuredBox {
    #[serde(default)]
    pub(super) x: f64,
    #[serde(default)]
    pub(super) y: f64,
    #[serde(default)]
    pub(super) width: f64,
    #[serde(default)]
    pub(super) height: f64,
    #[serde(default)]
    pub(super) children: Vec<MeasuredBox>,
}

#[derive(Clone, Debug)]
pub(super) struct Provenance<'a> {
    pub(super) source: &'a RelativePath,
    pub(super) source_sha256: &'a Sha256Digest,
    pub(super) linked_resources: &'a BTreeMap<RelativePath, Sha256Digest>,
    pub(super) helper_sha256: &'a Sha256Digest,
    pub(super) base_style_sha256: Option<&'a Sha256Digest>,
    pub(super) browser: &'a str,
    pub(super) browser_executable_sha256: &'a Sha256Digest,
    pub(super) launch_profile_sha256: &'a Sha256Digest,
    pub(super) corpus_manifest_sha256: &'a Sha256Digest,
    pub(super) taffy_revision: &'a SourceRevision,
    pub(super) taffy_sidecar_sha256: &'a Sha256Digest,
}

pub(super) fn render(
    variant: Variant,
    measurement: &MeasuredLayout,
    provenance: &Provenance<'_>,
) -> Result<Vec<u8>> {
    let mut output = String::new();
    output.push_str("<!-- generated-by: ");
    output.push_str(GENERATOR);
    output.push_str(" schema=2 source=\"");
    output.push_str(&attribute(provenance.source.as_str()));
    output.push_str("\" source-sha256=\"");
    output.push_str(provenance.source_sha256.as_str());
    output.push('"');
    if !provenance.linked_resources.is_empty() {
        output.push_str(" linked-resource-sha256=\"");
        let joined = provenance
            .linked_resources
            .iter()
            .map(|(path, digest)| format!("{}={}", path.as_str(), digest.as_str()))
            .collect::<Vec<_>>()
            .join(",");
        output.push_str(&attribute(&joined));
        output.push('"');
    }
    output.push_str(" helper-sha256=\"");
    output.push_str(provenance.helper_sha256.as_str());
    output.push('"');
    if let Some(digest) = provenance.base_style_sha256 {
        output.push_str(" base-style-sha256=\"");
        output.push_str(digest.as_str());
        output.push('"');
    }
    for (name, value) in [
        ("browser", provenance.browser),
        (
            "browser-executable-sha256",
            provenance.browser_executable_sha256.as_str(),
        ),
        (
            "launch-profile-sha256",
            provenance.launch_profile_sha256.as_str(),
        ),
        (
            "corpus-manifest-sha256",
            provenance.corpus_manifest_sha256.as_str(),
        ),
        ("taffy-revision", provenance.taffy_revision.as_str()),
        (
            "taffy-sidecar-sha256",
            provenance.taffy_sidecar_sha256.as_str(),
        ),
    ] {
        output.push(' ');
        output.push_str(name);
        output.push_str("=\"");
        output.push_str(&attribute(value));
        output.push('"');
    }
    output.push_str(" -->\n<test name=\"");
    output.push_str(&attribute(&variant.test_name(provenance.source)?));
    output.push_str("\" use-rounding=\"false\">\n  <viewport width=\"");
    output.push_str(&pixels(measurement.viewport_width));
    output.push_str("\" height=\"");
    output.push_str(&pixels(measurement.viewport_height));
    output.push_str("\"/>\n  <input>\n");
    render_input(&mut output, &measurement.input, 4);
    output.push_str("  </input>\n  <expectations>\n");
    for expected in &measurement.expectations {
        render_box(&mut output, expected, 4);
    }
    output.push_str("  </expectations>\n</test>\n");
    Ok(output.into_bytes())
}

fn render_input(output: &mut String, node: &MeasuredNode, indent: usize) {
    output.push_str(&" ".repeat(indent));
    output.push('<');
    output.push_str(&node.tag);
    for (name, value) in &node.attributes {
        output.push(' ');
        output.push_str(name);
        output.push_str("=\"");
        output.push_str(&attribute(value));
        output.push('"');
    }
    if !node.style.is_empty() {
        output.push_str(" style=\"");
        for (index, (name, value)) in node.style.iter().enumerate() {
            if index != 0 {
                output.push(' ');
            }
            output.push_str(name);
            output.push_str(": ");
            output.push_str(&attribute(value));
            output.push(';');
        }
        output.push('"');
    }
    if node.children.is_empty() && node.text.is_none() {
        output.push_str("/>\n");
        return;
    }
    output.push('>');
    if let Some(value) = &node.text {
        output.push_str(&text(value));
    }
    if !node.children.is_empty() {
        output.push('\n');
        for child in &node.children {
            render_input(output, child, indent + 2);
        }
        output.push_str(&" ".repeat(indent));
    }
    output.push_str("</");
    output.push_str(&node.tag);
    output.push_str(">\n");
}

fn render_box(output: &mut String, measured: &MeasuredBox, indent: usize) {
    output.push_str(&" ".repeat(indent));
    output.push_str("<node x=\"");
    output.push_str(&number(measured.x));
    output.push_str("\" y=\"");
    output.push_str(&number(measured.y));
    output.push_str("\" width=\"");
    output.push_str(&number(measured.width));
    output.push_str("\" height=\"");
    output.push_str(&number(measured.height));
    if measured.children.is_empty() {
        output.push_str("\"/>\n");
    } else {
        output.push_str("\">\n");
        for child in &measured.children {
            render_box(output, child, indent + 2);
        }
        output.push_str(&" ".repeat(indent));
        output.push_str("</node>\n");
    }
}

fn pixels(value: f64) -> String {
    format!("{}px", number(value))
}

fn number(value: f64) -> String {
    if value == 0.0 {
        "0".to_owned()
    } else if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        let mut rendered = format!("{value:.6}");
        while rendered.ends_with('0') {
            rendered.pop();
        }
        rendered
    }
}

fn attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
}

fn text(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;")
}

fn generation_error(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(GeneratorErrorKind::Generation, "render layout XML", detail)
}

#[cfg(test)]
mod tests {
    use super::{MeasuredLayout, Provenance, Variant, attribute, render, text};
    use crate::{RelativePath, Sha256Digest, SourceRevision};
    use std::collections::BTreeMap;

    #[test]
    fn layout_xml_provenance_complete_golden() {
        let zero = Sha256Digest::from_text("0".repeat(64)).expect("digest");
        let revision =
            SourceRevision::new("1111111111111111111111111111111111111111").expect("revision");
        let source = RelativePath::new("html/group/case.html").expect("source");
        let bytes = render(
            Variant::BorderBoxLtr,
            &MeasuredLayout::zero(),
            &Provenance {
                source: &source,
                source_sha256: &zero,
                linked_resources: &BTreeMap::new(),
                helper_sha256: &zero,
                base_style_sha256: None,
                browser: "Chrome 1 cache/chrome",
                browser_executable_sha256: &zero,
                launch_profile_sha256: &zero,
                corpus_manifest_sha256: &zero,
                taffy_revision: &revision,
                taffy_sidecar_sha256: &zero,
            },
        )
        .expect("render");
        let expected = concat!(
            "<!-- generated-by: surgeist-layout-generate schema=2 source=\"html/group/case.html\" source-sha256=\"0000000000000000000000000000000000000000000000000000000000000000\" helper-sha256=\"0000000000000000000000000000000000000000000000000000000000000000\" browser=\"Chrome 1 cache/chrome\" browser-executable-sha256=\"0000000000000000000000000000000000000000000000000000000000000000\" launch-profile-sha256=\"0000000000000000000000000000000000000000000000000000000000000000\" corpus-manifest-sha256=\"0000000000000000000000000000000000000000000000000000000000000000\" taffy-revision=\"1111111111111111111111111111111111111111\" taffy-sidecar-sha256=\"0000000000000000000000000000000000000000000000000000000000000000\" -->\n",
            "<test name=\"case__border_box_ltr\" use-rounding=\"false\">\n",
            "  <viewport width=\"0px\" height=\"0px\"/>\n",
            "  <input>\n    <div/>\n  </input>\n",
            "  <expectations>\n    <node x=\"0\" y=\"0\" width=\"0\" height=\"0\"/>\n  </expectations>\n",
            "</test>\n"
        );
        assert_eq!(bytes, expected.as_bytes());
        assert_eq!(
            Sha256Digest::from_bytes(&bytes).as_str(),
            "04dd77a3fca470f65858a35b059a34a146031adbc5dd80931dd8cbe508dacb6a"
        );
    }

    #[test]
    fn layout_xml_preserved_escape_complete_golden() {
        assert_eq!(attribute("&\"< >"), "&amp;&quot;&lt; >");
        assert_eq!(text("&\"< >"), "&amp;\"&lt; >");
    }

    #[test]
    fn layout_xml_optional_provenance_complete_golden() {
        let zero = Sha256Digest::from_text("0".repeat(64)).expect("digest");
        let one = Sha256Digest::from_text("1".repeat(64)).expect("digest");
        let revision =
            SourceRevision::new("1111111111111111111111111111111111111111").expect("revision");
        let source = RelativePath::new("html/group/case.html").expect("source");
        let mut linked = BTreeMap::new();
        linked.insert(
            RelativePath::new("assets/a.css").expect("linked path"),
            one.clone(),
        );
        let bytes = render(
            Variant::BorderBoxLtr,
            &MeasuredLayout::zero(),
            &Provenance {
                source: &source,
                source_sha256: &zero,
                linked_resources: &linked,
                helper_sha256: &zero,
                base_style_sha256: Some(&one),
                browser: "Chrome & \"quoted\" <1>",
                browser_executable_sha256: &zero,
                launch_profile_sha256: &zero,
                corpus_manifest_sha256: &zero,
                taffy_revision: &revision,
                taffy_sidecar_sha256: &zero,
            },
        )
        .expect("render optional provenance");
        let first = std::str::from_utf8(&bytes)
            .expect("UTF-8 XML")
            .lines()
            .next()
            .expect("first line");
        assert!(first.contains(
            "source-sha256=\"0000000000000000000000000000000000000000000000000000000000000000\" linked-resource-sha256=\"assets/a.css=1111111111111111111111111111111111111111111111111111111111111111\" helper-sha256="
        ));
        assert!(first.contains(
            "base-style-sha256=\"1111111111111111111111111111111111111111111111111111111111111111\" browser=\"Chrome &amp; &quot;quoted&quot; &lt;1>\""
        ));
    }
}
