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
    /// The operator is outside the transform's five supported spellings.
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
/// No attributes or inherited styles can be supplied by the current Formula
/// model; the transform refuses those inputs before writing is possible.
/// Under/over constructs retain their distinct elements and ordered operands.
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
    if !matches!(formula, Formula::Sequence(_)) {
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
        self.visit(location, depth)?;
        let mut children = Vec::new();
        let name = match formula {
            Formula::Sequence(items) => {
                for (index, child) in items.iter().enumerate() {
                    children.push(Node::Element(self.element(
                        child,
                        &format!("{location}/children/{index}"),
                        depth + 1,
                    )?));
                }
                if depth == 1 {
                    "math"
                } else {
                    "mrow"
                }
            }
            Formula::Symbol(text) | Formula::Operator(text) => {
                let operator = matches!(formula, Formula::Operator(_));
                if operator && !matches!(text.as_str(), "+" | "-" | "=" | "*" | "×") {
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
            Formula::UnderOver(base, under, over) => {
                for (child, field) in [(base, "base"), (under, "under"), (over, "over")] {
                    children.push(Node::Element(self.element(
                        child,
                        &format!("{location}/{field}"),
                        depth + 1,
                    )?));
                }
                "munderover"
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
        };
        Ok(Element {
            name: QualifiedName {
                prefix: None,
                local: name.into(),
                namespace: Some(Arc::clone(&self.namespace)),
            },
            namespaces: Vec::new(),
            attributes: Vec::new(),
            children,
        })
    }
}

// Only serializes syntax constructed above: fixed names, one root namespace,
// no attributes or comments. This is not a general-purpose XML serializer.
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
