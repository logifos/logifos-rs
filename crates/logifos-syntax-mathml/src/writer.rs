//! Bounded Formula-to-syntax conversion and deterministic XML serialization.

use std::{fmt, sync::Arc};

use logifos_model::{Formula, MAX_DEPTH, MAX_NODES};

use crate::{
    text::xml_char, Element, MathmlDocument, NamespaceDeclaration, Node, ParseLimits,
    QualifiedName, MATHML_NAMESPACE,
};

/// Stable categories for a refused Formula-to-MathML conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteErrorKind {
    /// A document requires a sequence root to preserve Formula grouping.
    InvalidStructure,
    /// The operator is outside the transform's supported spellings.
    UnsupportedOperator,
    /// Symbol text contains a character prohibited by XML 1.0.
    InvalidCharacter,
    /// The emitted document would exceed the parser's byte, node or depth limit.
    ResourceLimit,
}

/// An all-or-nothing writer refusal; diagnostics never echo token contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteError {
    /// Stable failure category.
    pub kind: WriteErrorKind,
    /// Formula path: the root is empty, sequences use `/children/N`, fractions
    /// use `/numerator` and `/denominator`, scripts use `/base` and `/script`,
    /// radicals use `/radicand` and `/index`, and under/over constructs use
    /// `/base`, `/under` and `/over`.
    /// An output byte-limit refusal identifies the whole document (empty path).
    pub location: String,
}

impl WriteError {
    /// Maps refusals into the version 1 error vocabulary.
    pub fn code(&self) -> &'static str {
        match self.kind {
            WriteErrorKind::ResourceLimit => "RESOURCE_LIMIT",
            _ => "UNSUPPORTED_SYNTAX",
        }
    }
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} at Formula path {:?}", self.kind, self.location)
    }
}

impl std::error::Error for WriteError {}

fn refusal(kind: WriteErrorKind, location: &str) -> WriteError {
    WriteError {
        kind,
        location: location.into(),
    }
}

/// Writes a complete MathML document whose parsed Formula equals `formula`.
///
/// The root must be a sequence, as produced by [`crate::transform`]. Nested
/// sequences remain explicit `mrow` elements, including empty and singleton
/// rows. All symbols use `mi`, since Formula does not distinguish identifier,
/// number and text tokens. Operators retain their exact supported spelling.
/// Typed presentation hints retain their owning element, including table cells.
/// Attribute values use canonical case and decimal integer spellings.
/// Script constructs retain their distinct elements and ordered operands,
/// including multiscript placeholders and an explicitly empty prescript boundary.
/// Default spaces and string literals retain their distinct token kinds.
///
/// Output uses the default MathML namespace, explicit closing tags, no pretty
/// whitespace, and escaped text (including CR as a character reference). XML
/// character, source-byte and syntax-tree limits match [`crate::parse`], counting
/// nonempty token text as a syntax node. Failure returns no partial document.
///
/// ```
/// use logifos_syntax_mathml::{parse, transform, write};
/// let formula = transform(&parse("<math><mi>x</mi><mo>+</mo><mn>1</mn></math>")?)?;
/// let mathml = write(&formula)?;
/// assert_eq!(transform(&parse(mathml)?)?, formula);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn write(formula: &Formula) -> Result<String, WriteError> {
    let root = match formula {
        Formula::Styled { content, .. } => content.as_ref(),
        _ => formula,
    };
    if !matches!(root, Formula::Sequence(_)) {
        return Err(refusal(WriteErrorKind::InvalidStructure, ""));
    }
    let mut builder = SyntaxBuilder {
        nodes: 0,
        text_bytes: 0,
        namespace: Arc::from(MATHML_NAMESPACE),
    };
    let mut root = builder.element(formula, "", 1)?;
    root.namespaces.push(NamespaceDeclaration {
        prefix: None,
        uri: builder.namespace,
    });
    let document = MathmlDocument {
        before_root: Vec::new(),
        root,
        after_root: Vec::new(),
    };
    let mut output = Output(String::new());
    output.element(&document.root)?;
    Ok(output.0)
}

struct SyntaxBuilder {
    nodes: usize,
    text_bytes: usize,
    namespace: Arc<str>,
}

impl SyntaxBuilder {
    fn visit(&mut self, location: &str, depth: usize) -> Result<(), WriteError> {
        if depth > MAX_DEPTH || self.nodes >= MAX_NODES {
            return Err(refusal(WriteErrorKind::ResourceLimit, location));
        }
        self.nodes += 1;
        Ok(())
    }

    fn element(
        &mut self,
        formula: &Formula,
        location: &str,
        depth: usize,
    ) -> Result<Element, WriteError> {
        if let Formula::Styled { style, content } = formula {
            return self.styled(*style, content, location, depth);
        }
        self.visit(location, depth)?;
        let mut children = Vec::new();
        let mut attributes_elem: Vec<crate::Attribute> = Vec::new();
        let name = match formula {
            Formula::Styled { .. } => unreachable!("handled before syntax allocation"),
            Formula::Sequence(items)
            | Formula::Style(items)
            | Formula::ErrorGroup(items)
            | Formula::Padded(items)
            | Formula::Phantom(items) => {
                for (index, child) in items.iter().enumerate() {
                    children.push(Node::Element(self.element(
                        child,
                        &format!("{location}/children/{index}"),
                        depth + 1,
                    )?));
                }
                match formula {
                    Formula::Style(_) => "mstyle",
                    Formula::ErrorGroup(_) => "merror",
                    Formula::Padded(_) => "mpadded",
                    Formula::Phantom(_) => "mphantom",
                    _ if depth == 1 => "math",
                    _ => "mrow",
                }
            }
            Formula::Symbol(text) | Formula::Operator(text) | Formula::StringLiteral(text) => {
                let operator = matches!(formula, Formula::Operator(_));
                if operator && !logifos_model::operator::is_operator_text(text) {
                    return Err(refusal(WriteErrorKind::UnsupportedOperator, location));
                }
                if text.len() > ParseLimits::default().source_bytes - self.text_bytes {
                    return Err(refusal(WriteErrorKind::ResourceLimit, location));
                }
                self.text_bytes += text.len();
                if !text.chars().all(xml_char) {
                    return Err(refusal(WriteErrorKind::InvalidCharacter, location));
                }
                if !text.is_empty() {
                    self.visit(location, depth + 1)?;
                    children.push(Node::Text(text.clone()));
                }
                if operator {
                    "mo"
                } else if matches!(formula, Formula::StringLiteral(_)) {
                    "ms"
                } else {
                    "mi"
                }
            }
            Formula::Fraction(first, second)
            | Formula::Subscript(first, second)
            | Formula::Superscript(first, second) => {
                let (name, first_field, second_field) = match formula {
                    Formula::Fraction(..) => ("mfrac", "numerator", "denominator"),
                    Formula::Subscript(..) => ("msub", "base", "script"),
                    _ => ("msup", "base", "script"),
                };
                for (child, field) in [(first, first_field), (second, second_field)] {
                    children.push(Node::Element(self.element(
                        child,
                        &format!("{location}/{field}"),
                        depth + 1,
                    )?));
                }
                name
            }
            Formula::Under(base, script) | Formula::Over(base, script) => {
                let (name, field) = if matches!(formula, Formula::Under(..)) {
                    ("munder", "under")
                } else {
                    ("mover", "over")
                };
                for (child, field) in [(base, "base"), (script, field)] {
                    children.push(Node::Element(self.element(
                        child,
                        &format!("{location}/{field}"),
                        depth + 1,
                    )?));
                }
                name
            }
            Formula::UnderOver(base, under, over) | Formula::SubSup(base, under, over) => {
                let (name, lower, upper) = if matches!(formula, Formula::SubSup(..)) {
                    ("msubsup", "subscript", "superscript")
                } else {
                    ("munderover", "under", "over")
                };
                for (child, field) in [(base, "base"), (under, lower), (over, upper)] {
                    children.push(Node::Element(self.element(
                        child,
                        &format!("{location}/{field}"),
                        depth + 1,
                    )?));
                }
                name
            }
            Formula::Space(width, height, depth) => {
                for (field, length) in [("width", width), ("height", height), ("depth", depth)] {
                    if let Some(length) = length {
                        attributes_elem.push(crate::Attribute {
                            name: QualifiedName {
                                prefix: None,
                                local: field.into(),
                                namespace: None,
                            },
                            value: Self::format_length(*length),
                        });
                    }
                }
                "mspace"
            }
            Formula::MultiScripts {
                base,
                postscripts,
                prescripts,
            } => {
                children.push(Node::Element(self.element(
                    base,
                    &format!("{location}/base"),
                    depth + 1,
                )?));
                self.script_pairs(
                    postscripts,
                    &format!("{location}/postscripts"),
                    depth + 1,
                    &mut children,
                )?;
                if let Some(prescripts) = prescripts {
                    let path = format!("{location}/prescripts");
                    children.push(Node::Element(self.marker(
                        "mprescripts",
                        &path,
                        depth + 1,
                    )?));
                    self.script_pairs(prescripts, &path, depth + 1, &mut children)?;
                }
                "mmultiscripts"
            }
            Formula::Root(radicand, index) => {
                for (child, field) in [(radicand, "radicand"), (index, "index")] {
                    children.push(Node::Element(self.element(
                        child,
                        &format!("{location}/{field}"),
                        depth + 1,
                    )?));
                }
                "mroot"
            }
            Formula::Sqrt(radicand) => {
                // msqrt inlines its children, so only the transform's
                // sequence radicand reparses to an equal Formula.
                let Formula::Sequence(items) = &**radicand else {
                    return Err(refusal(
                        WriteErrorKind::InvalidStructure,
                        &format!("{location}/radicand"),
                    ));
                };
                for (index, child) in items.iter().enumerate() {
                    children.push(Node::Element(self.element(
                        child,
                        &format!("{location}/radicand/children/{index}"),
                        depth + 1,
                    )?));
                }
                "msqrt"
            }
            Formula::Table(rows) => {
                for (row_index, row) in rows.iter().enumerate() {
                    let row_path = format!("{location}/rows/{row_index}");
                    self.visit(&row_path, depth + 1)?;
                    let mut cells = Vec::with_capacity(row.len());
                    for (cell_index, cell) in row.iter().enumerate() {
                        // mtd generates an anonymous row, so only the
                        // transform's sequence cells reparse equal.
                        let cell_path = format!("{row_path}/cells/{cell_index}");
                        self.visit(&cell_path, depth + 2)?;
                        let (cell, style) = match cell {
                            Formula::Styled { style, content } => {
                                if !style.valid_for(content, false, true) {
                                    return Err(refusal(
                                        WriteErrorKind::InvalidStructure,
                                        &cell_path,
                                    ));
                                }
                                (content.as_ref(), Some(*style))
                            }
                            _ => (cell, None),
                        };
                        let Formula::Sequence(items) = cell else {
                            return Err(refusal(WriteErrorKind::InvalidStructure, &cell_path));
                        };
                        let mut content = Vec::with_capacity(items.len());
                        for (index, child) in items.iter().enumerate() {
                            content.push(Node::Element(self.element(
                                child,
                                &format!("{cell_path}/children/{index}"),
                                depth + 3,
                            )?));
                        }
                        let mut element = self.node("mtd", content);
                        if let Some(style) = style {
                            Self::style_attributes(&mut element, style);
                        }
                        cells.push(Node::Element(element));
                    }
                    children.push(Node::Element(self.node("mtr", cells)));
                }
                "mtable"
            }
        };
        let mut element = self.syntax_element(name, children);
        element.attributes = attributes_elem;
        Ok(element)
    }

    fn styled(
        &mut self,
        style: logifos_model::PresentationStyle,
        content: &Formula,
        location: &str,
        depth: usize,
    ) -> Result<Element, WriteError> {
        if !style.valid_for(content, depth == 1, false) {
            return Err(refusal(WriteErrorKind::InvalidStructure, location));
        }
        let mut element = self.element(content, location, depth)?;
        Self::style_attributes(&mut element, style);
        Ok(element)
    }

    fn style_attributes(element: &mut Element, style: logifos_model::PresentationStyle) {
        use logifos_model::{MathColor, MathDisplay, ScriptLevel};
        let mut attribute = |name: &str, value: String| {
            element.attributes.push(crate::Attribute {
                name: QualifiedName {
                    prefix: None,
                    local: name.into(),
                    namespace: None,
                },
                value,
            });
        };
        if let Some(display) = style.display {
            attribute(
                "display",
                match display {
                    MathDisplay::Block => "block",
                    MathDisplay::Inline => "inline",
                }
                .into(),
            );
        }
        if let Some(displaystyle) = style.displaystyle {
            attribute("displaystyle", displaystyle.to_string());
        }
        if let Some(level) = style.script_level {
            attribute(
                "scriptlevel",
                match level {
                    ScriptLevel::Absolute(n) => n.to_string(),
                    ScriptLevel::Add(n) => format!("+{n}"),
                    ScriptLevel::Subtract(n) => format!("-{n}"),
                },
            );
        }
        if let Some(variant) = style.math_variant {
            attribute("mathvariant", variant.as_str().into());
        }
        if let Some(color) = style.math_color {
            attribute(
                "mathcolor",
                match color {
                    MathColor::Red => "red",
                    MathColor::Blue => "blue",
                }
                .into(),
            );
        }
        if let Some(space) = style.right_space {
            attribute("rspace", space.to_css());
        }
        if let Some(accent) = style.accent {
            attribute("accent", accent.to_string());
        }
        if style.column_align.is_some() {
            attribute("columnalign", "center".into());
        }
    }

    fn script_pairs(
        &mut self,
        pairs: &[logifos_model::ScriptPair],
        location: &str,
        depth: usize,
        children: &mut Vec<Node>,
    ) -> Result<(), WriteError> {
        for (index, pair) in pairs.iter().enumerate() {
            for (child, field) in [
                (&pair.subscript, "subscript"),
                (&pair.superscript, "superscript"),
            ] {
                let path = format!("{location}/{index}/{field}");
                children.push(Node::Element(match child {
                    Some(child) => self.element(child, &path, depth)?,
                    None => self.marker("none", &path, depth)?,
                }));
            }
        }
        Ok(())
    }

    fn marker(&mut self, name: &str, location: &str, depth: usize) -> Result<Element, WriteError> {
        self.visit(location, depth)?;
        Ok(self.syntax_element(name, Vec::new()))
    }

    fn syntax_element(&self, name: &str, children: Vec<Node>) -> Element {
        Element {
            name: QualifiedName {
                prefix: None,
                local: name.into(),
                namespace: Some(Arc::clone(&self.namespace)),
            },
            namespaces: Vec::new(),
            attributes: Vec::new(),
            children,
        }
    }
    /// A finished element with the shared namespace and no attributes.
    /// Formats an authored length: shortest decimal with its unit.
    fn format_length(length: logifos_model::Length) -> String {
        let unit = match length.unit {
            logifos_model::LengthUnit::Em => "em",
            logifos_model::LengthUnit::Ex => "ex",
            logifos_model::LengthUnit::Px => "px",
            logifos_model::LengthUnit::Percent => "%",
        };
        format!("{}{unit}", length.value)
    }

    fn node(&self, name: &str, children: Vec<Node>) -> Element {
        Element {
            name: QualifiedName {
                prefix: None,
                local: name.into(),
                namespace: Some(Arc::clone(&self.namespace)),
            },
            namespaces: Vec::new(),
            attributes: Vec::new(),
            children,
        }
    }
}

// Only serializes syntax constructed above: fixed names, one root namespace,
// validated enum/boolean/integer attributes and no comments. Not a general XML serializer.
struct Output(String);

impl Output {
    fn push(&mut self, text: &str) -> Result<(), WriteError> {
        if text.len() > ParseLimits::default().source_bytes - self.0.len() {
            return Err(refusal(WriteErrorKind::ResourceLimit, ""));
        }
        self.0.push_str(text);
        Ok(())
    }

    fn element(&mut self, element: &Element) -> Result<(), WriteError> {
        self.push("<")?;
        self.push(&element.name.local)?;
        for namespace in &element.namespaces {
            self.push(" xmlns=\"")?;
            self.push(&namespace.uri)?;
            self.push("\"")?;
        }
        for attribute in &element.attributes {
            self.push(" ")?;
            self.push(&attribute.name.local)?;
            self.push("=\"")?;
            for ch in attribute.value.chars() {
                let mut buffer = [0; 4];
                self.push(match ch {
                    '"' => "&quot;",
                    '&' => "&amp;",
                    '<' => "&lt;",
                    _ => ch.encode_utf8(&mut buffer),
                })?;
            }
            self.push("\"")?;
        }
        self.push(">")?;
        for node in &element.children {
            match node {
                Node::Element(child) => self.element(child)?,
                Node::Text(text) => {
                    for ch in text.chars() {
                        let mut buffer = [0; 4];
                        self.push(match ch {
                            '&' => "&amp;",
                            '<' => "&lt;",
                            '>' => "&gt;",
                            '\r' => "&#xD;",
                            _ => ch.encode_utf8(&mut buffer),
                        })?;
                    }
                }
                Node::Comment(_) => return Err(refusal(WriteErrorKind::InvalidStructure, "")),
            }
        }
        self.push("</")?;
        self.push(&element.name.local)?;
        self.push(">")
    }
}
