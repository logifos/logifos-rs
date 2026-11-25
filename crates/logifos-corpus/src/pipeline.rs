use logifos_layout::{
    font::FontProvider,
    layout::{layout, LayoutOptions},
};
use logifos_svg::{emit, SvgOptions};
use logifos_syntax_mathml::{
    parse, transform, write, ParseErrorKind, TransformErrorKind, WriteErrorKind,
};
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Outcome {
    Pass,
    Refusal,
    Failure,
}
impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Refusal => "refusal",
            Self::Failure => "failure",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Observation {
    pub stage: &'static str,
    pub outcome: Outcome,
    pub category: String,
}

impl Observation {
    pub fn new(stage: &'static str, outcome: Outcome, category: impl Into<String>) -> Self {
        Self {
            stage,
            outcome,
            category: category.into(),
        }
    }
    pub fn value(&self) -> Value {
        json!({"stage":self.stage,"outcome":self.outcome.as_str(),"category":self.category})
    }
}

fn passed(stage: &'static str, observations: &mut Vec<Observation>) {
    observations.push(Observation::new(stage, Outcome::Pass, "complete"));
}

use crate::Capture;

/// Runs the pipeline and captures the emitted artifacts of a fully passing
/// case, for the canon comparison and the desired-render emission.
pub(crate) fn run_capturing(
    kind: &str,
    source: &[u8],
    fonts: Option<&mut FontProvider>,
    captured: &mut Option<Capture>,
) -> Vec<Observation> {
    let mut observations = Vec::new();
    let result = execute(kind, source, fonts, &mut observations, captured);
    if let Err(observation) = result {
        observations.push(observation);
    }
    observations
}

fn execute(
    kind: &str,
    source: &[u8],
    fonts: Option<&mut FontProvider>,
    observations: &mut Vec<Observation>,
    captured: &mut Option<Capture>,
) -> Result<(), Observation> {
    let refusal = |stage, category| Observation::new(stage, Outcome::Refusal, category);
    if kind == "bootstrap-request" {
        let reply = logifos_facade::request_json(source);
        if reply["ok"] == false {
            return match reply["error"]["code"].as_str() {
                Some(code) => Err(refusal("facade", code.to_owned())),
                None => Err(Observation::new(
                    "facade",
                    Outcome::Failure,
                    "missing_error_code",
                )),
            };
        }
        if reply["ok"] != true {
            return Err(Observation::new(
                "facade",
                Outcome::Failure,
                "invalid_response",
            ));
        }
        passed("facade", observations);
        return Ok(());
    }
    if kind == "latex" {
        // The KaTeX corpus replays once the LaTeX syntax crate lands (gate 4);
        // until then the gap is an observation, not a harness failure.
        return Err(refusal("input", "latex_not_yet_supported".to_owned()));
    }
    if kind != "mathml" {
        return Err(Observation::new(
            "input",
            Outcome::Failure,
            "unsupported_input_kind",
        ));
    }
    let document = parse(source).map_err(|e| {
        refusal(
            "parse",
            match e.kind {
                ParseErrorKind::InvalidUtf8 => "invalid_utf8",
                ParseErrorKind::MalformedXml => "malformed_xml",
                ParseErrorKind::UnsafeXml => "unsafe_xml",
                ParseErrorKind::UnsupportedSyntax => "unsupported_syntax",
                ParseErrorKind::ResourceLimit => "resource_limit",
                ParseErrorKind::InvalidLimits => "invalid_limits",
            }
            .into(),
        )
    })?;
    passed("parse", observations);
    let formula = transform(&document).map_err(|e| {
        refusal(
            "transform",
            match e.kind {
                TransformErrorKind::UnsupportedElement => "unsupported_element",
                TransformErrorKind::UnsupportedAttribute => "unsupported_attribute",
                TransformErrorKind::UnsupportedOperator => "unsupported_operator",
                TransformErrorKind::UnsupportedContent => "unsupported_content",
                TransformErrorKind::InvalidStructure => "invalid_structure",
                TransformErrorKind::ResourceLimit => "resource_limit",
            }
            .into(),
        )
    })?;
    passed("transform", observations);
    let output = write(&formula).map_err(|e| {
        refusal(
            "writer",
            match e.kind {
                WriteErrorKind::InvalidStructure => "invalid_structure",
                WriteErrorKind::UnsupportedOperator => "unsupported_operator",
                WriteErrorKind::InvalidCharacter => "invalid_character",
                WriteErrorKind::ResourceLimit => "resource_limit",
            }
            .into(),
        )
    })?;
    passed("writer", observations);
    let reparsed = parse(&output)
        .map_err(|_| Observation::new("roundtrip", Outcome::Failure, "invalid_writer_output"))?;
    let restored = transform(&reparsed).map_err(|_| {
        Observation::new(
            "roundtrip",
            Outcome::Failure,
            "untransformable_writer_output",
        )
    })?;
    if restored != formula {
        return Err(Observation::new(
            "roundtrip",
            Outcome::Failure,
            "formula_mismatch",
        ));
    }
    passed("roundtrip", observations);
    let fonts = fonts.ok_or_else(|| refusal("layout", "font_not_configured".into()))?;
    let boxes = layout(
        &formula,
        fonts,
        LayoutOptions {
            font_size_px: 18.0,
            script_level: 0,
            display: false,
        },
    )
    .map_err(|e| refusal("layout", e.code().to_owned()))?;
    passed("layout", observations);
    let svg = emit(&boxes.tree, &boxes.glyphs, &SvgOptions::default()).map_err(|e| {
        Observation::new(
            "emit",
            Outcome::Failure,
            match e {
                logifos_svg::SvgError::InvalidColor => "invalid_color",
                logifos_svg::SvgError::InvalidXmlCharacter => "invalid_xml_character",
                logifos_svg::SvgError::Geometry(_) => "invalid_geometry",
                logifos_svg::SvgError::Outline(_) => "invalid_outline",
            },
        )
    })?;
    let repeated = emit(&boxes.tree, &boxes.glyphs, &SvgOptions::default())
        .map_err(|_| Observation::new("emit", Outcome::Failure, "repeat_refused"))?;
    if svg != repeated {
        return Err(Observation::new(
            "emit",
            Outcome::Failure,
            "nondeterministic_output",
        ));
    }
    let metrics = svg.metrics();
    *captured = Some(Capture {
        svg: svg.as_str().to_owned(),
        metrics: (metrics.width(), metrics.height(), metrics.depth()),
    });
    passed("emit", observations);
    Ok(())
}
