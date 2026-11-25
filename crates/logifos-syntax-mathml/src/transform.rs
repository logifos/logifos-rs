//! Conservative conversion from syntax into the current Formula vocabulary.

mod presentation;
mod scripts;

use std::fmt;

use logifos_model::{Formula, Length, LengthUnit, MAX_DEPTH, MAX_NODES};

use crate::{Element, MathmlDocument, Node, QualifiedName, MATHML_NAMESPACE};

/// Stable categories for a refused syntax-to-Formula conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformErrorKind {
    /// An element has no representation in the current Formula model.
    UnsupportedElement,
    /// A non-namespace attribute cannot be preserved by Formula.
    UnsupportedAttribute,
    /// An `mo` token is outside the five typed operators.
    UnsupportedOperator,
    /// Comments or character data cannot be represented at this position.
    UnsupportedContent,
    /// A root, token or binary construct has an invalid structure.
    InvalidStructure,
    /// The shared model's depth or node budget was exceeded.
    ResourceLimit,
}

/// A refusal naming the element and its exact location in the syntax tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransformError {
    /// Stable failure category.
    pub kind: TransformErrorKind,
    /// Qualified name of the rejected element, or the containing element for text.
    pub element: QualifiedName,
    /// Syntax-tree path, rooted at `/root`. Child indices include all syntax nodes.
    /// Attribute and document-comment paths identify their respective array entries.
    /// This is a tree location, not an original source byte offset.
    pub location: String,
    /// The rejected attribute's qualified name, when applicable.
    pub attribute: Option<Box<QualifiedName>>,
}

impl TransformError {
    /// Maps refusals into the facade's version 1 error vocabulary.
    pub fn code(&self) -> &'static str {
        match self.kind {
            TransformErrorKind::ResourceLimit => "RESOURCE_LIMIT",
            _ => "UNSUPPORTED_SYNTAX",
        }
    }
}

impl fmt::Display for TransformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?} for element {} at {}",
            self.kind, self.element.local, self.location
        )
    }
}

impl std::error::Error for TransformError {}

fn refusal(kind: TransformErrorKind, element: &Element, location: &str) -> TransformError {
    TransformError {
        kind,
        element: element.name.clone(),
        location: location.into(),
        attribute: None,
    }
}

/// Transforms a parsed MathML document into the shared Formula model.
///
/// `math` and `mrow` become ordered sequences (including empty or singleton rows).
/// `mi`, `mn` and `mtext` become symbols with their decoded text preserved exactly;
/// Formula has one shared text/identifier class. `mo` trims its boundary
/// whitespace per MathML Core, then accepts the operator dictionary's text —
/// the typed operators, the fences and every non-whitespace operator
/// character — with no substitution or symbol fallback. `mfrac`, `msub`
/// and `msup` require exactly two element operands in source order. `msqrt`
/// wraps every child in one radicand; `mroot` takes base then index and falls
/// back to a row outside its two-child arity. `munder` and `mover` preserve
/// base then script; `munderover` preserves base, underscript then overscript.
/// Under/over elements outside their two- or three-child arity become rows.
/// `mtable` holds `mtr` rows of `mtd` cells (each an anonymous row); loose
/// rows or cells refuse.
///
/// Only XML whitespace between operands is insignificant. Namespace declarations
/// identify names and carry no Formula presentation data. Row-like elements
/// preserve validated `displaystyle` and bounded `scriptlevel` hints; the root
/// also preserves `display`. Grouping elements retain supported font variants
/// and red/blue colors; operators retain named positive `rspace`; over-script
/// constructs retain boolean `accent`; table cells retain centered `columnalign`.
/// These are syntax capabilities; layout may explicitly refuse a retained hint.
/// Other attributes, comments, nested token markup
/// and non-whitespace text
/// outside tokens refuses the entire conversion. No partial tree is returned.
/// `msubsup` preserves base, subscript then superscript, with row semantics at
/// other arities. Multiscripts preserve paired slots and an optional prescript
/// boundary; malformed pairs and nonempty markers refuse typed. `none` is
/// accepted only in multiscript slots. Attribute-free `mspace` and `ms` retain
/// their distinct kinds; dimensions and custom quotes remain unsupported.
/// `mstyle`, `merror`, `mpadded` and `mphantom` preserve their
/// row-like container kinds. `semantics` transforms only its first element;
/// subsequent element subtrees, including annotations and their attributes,
/// are ignored. A missing first element or an annotation first element refuses.
/// Fences, enclosure and standalone annotations remain unsupported.
/// Names in either the MathML namespace or no namespace are accepted.
///
/// Traversal is bounded by the shared model's root-inclusive depth and node
/// limits, also counting syntax text nodes and insignificant whitespace in
/// presentation content. Ignored semantics subtrees are not traversed. These
/// checks apply to manually constructed syntax trees as well as parsed input.
pub fn transform(document: &MathmlDocument) -> Result<Formula, TransformError> {
    // Comments around the root are authoring annotations, not formula
    // content; they carry no Formula representation and drop silently.
    let _ = (&document.before_root, &document.after_root);
    if document.root.name.local != "math" {
        return Err(refusal(
            TransformErrorKind::InvalidStructure,
            &document.root,
            "/root",
        ));
    }
    Transformer { nodes: 0 }.element(&document.root, "/root", 1)
}

struct Transformer {
    nodes: usize,
}

impl Transformer {
    fn visit(
        &mut self,
        element: &Element,
        location: &str,
        depth: usize,
    ) -> Result<(), TransformError> {
        if depth > MAX_DEPTH || self.nodes >= MAX_NODES {
            return Err(refusal(
                TransformErrorKind::ResourceLimit,
                element,
                location,
            ));
        }
        self.nodes += 1;
        Ok(())
    }

    /// Collects element operands with insignificant whitespace between them,
    /// the shared rule for row-like elements.
    fn row_like(
        &mut self,
        element: &Element,
        location: &str,
        depth: usize,
    ) -> Result<Vec<Formula>, TransformError> {
        let mut children = Vec::new();
        for (index, node) in element.children.iter().enumerate() {
            let path = format!("{location}/children/{index}");
            match node {
                Node::Element(child) => {
                    children.push(self.element(child, &path, depth + 1)?);
                }
                Node::Text(text) if text.chars().all(xml_whitespace) => {
                    self.visit(element, &path, depth + 1)?;
                }
                Node::Text(_) | Node::Comment(_) => {
                    return Err(refusal(
                        TransformErrorKind::UnsupportedContent,
                        element,
                        &path,
                    ));
                }
            }
        }
        Ok(children)
    }

    fn element(
        &mut self,
        element: &Element,
        location: &str,
        depth: usize,
    ) -> Result<Formula, TransformError> {
        self.check_element(element, location, depth)?;
        let style = presentation::attributes(element, location)?;
        let formula = self.content(element, location, depth)?;
        if style == logifos_model::PresentationStyle::default() {
            Ok(formula)
        } else {
            Ok(Formula::Styled {
                style,
                content: Box::new(formula),
            })
        }
    }

    fn content(
        &mut self,
        element: &Element,
        location: &str,
        depth: usize,
    ) -> Result<Formula, TransformError> {
        match element.name.local.as_str() {
            "semantics" => self.semantics(element, location, depth),
            "mstyle" | "merror" | "mpadded" | "mphantom" => {
                let children = self.row_like(element, location, depth)?;
                Ok(match element.name.local.as_str() {
                    "mstyle" => Formula::Style(children),
                    "merror" => Formula::ErrorGroup(children),
                    "mpadded" => Formula::Padded(children),
                    _ => Formula::Phantom(children),
                })
            }
            "mmultiscripts" => self.multiscripts(element, location, depth),
            "mspace" => {
                self.empty_element(element, location, depth)?;
                let mut extents = [None; 3];
                for attribute in &element.attributes {
                    let slot = match attribute.name.local.as_str() {
                        "width" => 0,
                        "height" => 1,
                        "depth" => 2,
                        _ => continue,
                    };
                    extents[slot] = parse_length(&attribute.value);
                }
                Ok(Formula::Space(extents[0], extents[1], extents[2]))
            }
            "mi" | "mn" | "mtext" | "mo" | "ms" => self.token(element, location, depth),
            "math" if depth != 1 => Err(refusal(
                TransformErrorKind::UnsupportedElement,
                element,
                location,
            )),
            "math" | "mrow" | "mfrac" | "msub" | "msup" | "msqrt" | "mroot" | "munder"
            | "mover" | "munderover" | "msubsup" => self.construct(element, location, depth),
            "mtable" => {
                let mut rows = Vec::new();
                for (row_index, node) in element.children.iter().enumerate() {
                    let row_path = format!("{location}/children/{row_index}");
                    match node {
                        Node::Element(row) if row.name.local.as_str() == "mtr" => {
                            self.check_element(row, &row_path, depth + 1)?;
                            let mut cells = Vec::new();
                            for (cell_index, cell_node) in row.children.iter().enumerate() {
                                let cell_path = format!("{row_path}/children/{cell_index}");
                                match cell_node {
                                    Node::Element(cell) if cell.name.local.as_str() == "mtd" => {
                                        self.check_element(cell, &cell_path, depth + 2)?;
                                        let children =
                                            self.row_like(cell, &cell_path, depth + 2)?;
                                        let content = Formula::Sequence(children);
                                        let style = presentation::attributes(cell, &cell_path)?;
                                        cells.push(
                                            if style == logifos_model::PresentationStyle::default()
                                            {
                                                content
                                            } else {
                                                Formula::Styled {
                                                    style,
                                                    content: Box::new(content),
                                                }
                                            },
                                        );
                                    }
                                    Node::Text(text) if text.chars().all(xml_whitespace) => {
                                        self.visit(row, &cell_path, depth + 2)?;
                                    }
                                    Node::Text(_) | Node::Comment(_) => {
                                        return Err(refusal(
                                            TransformErrorKind::UnsupportedContent,
                                            row,
                                            &cell_path,
                                        ));
                                    }
                                    _ => {
                                        let offending = match cell_node {
                                            Node::Element(cell) => cell,
                                            _ => row,
                                        };
                                        return Err(refusal(
                                            TransformErrorKind::UnsupportedElement,
                                            offending,
                                            &cell_path,
                                        ));
                                    }
                                }
                            }
                            rows.push(cells);
                        }
                        Node::Text(text) if text.chars().all(xml_whitespace) => {
                            self.visit(element, &row_path, depth + 1)?;
                        }
                        Node::Text(_) | Node::Comment(_) => {
                            return Err(refusal(
                                TransformErrorKind::UnsupportedContent,
                                element,
                                &row_path,
                            ));
                        }
                        _ => {
                            let offending = match node {
                                Node::Element(row) => row,
                                _ => element,
                            };
                            return Err(refusal(
                                TransformErrorKind::UnsupportedElement,
                                offending,
                                &row_path,
                            ));
                        }
                    }
                }
                Ok(Formula::Table(rows))
            }
            _ => Err(refusal(
                TransformErrorKind::UnsupportedElement,
                element,
                location,
            )),
        }
    }

    fn semantics(
        &mut self,
        element: &Element,
        location: &str,
        depth: usize,
    ) -> Result<Formula, TransformError> {
        let mut first = None;
        for (index, node) in element.children.iter().enumerate() {
            let path = format!("{location}/children/{index}");
            match node {
                Node::Element(child) if first.is_none() => {
                    if matches!(child.name.local.as_str(), "annotation" | "annotation-xml") {
                        return Err(refusal(TransformErrorKind::InvalidStructure, child, &path));
                    }
                    first = Some(self.element(child, &path, depth + 1)?);
                }
                Node::Element(_) => {}
                Node::Text(text) if text.chars().all(xml_whitespace) => {
                    self.visit(element, &path, depth + 1)?;
                }
                Node::Text(_) | Node::Comment(_) => {
                    return Err(refusal(
                        TransformErrorKind::UnsupportedContent,
                        element,
                        &path,
                    ));
                }
            }
        }
        first.ok_or_else(|| refusal(TransformErrorKind::InvalidStructure, element, location))
    }

    fn check_element(
        &mut self,
        element: &Element,
        location: &str,
        depth: usize,
    ) -> Result<(), TransformError> {
        self.visit(element, location, depth)?;
        if !matches!(
            element.name.namespace.as_deref(),
            None | Some(MATHML_NAMESPACE)
        ) {
            return Err(refusal(
                TransformErrorKind::UnsupportedElement,
                element,
                location,
            ));
        }
        presentation::attributes(element, location)?;
        Ok(())
    }

    fn construct(
        &mut self,
        element: &Element,
        location: &str,
        depth: usize,
    ) -> Result<Formula, TransformError> {
        let children = self.row_like(element, location, depth)?;
        match element.name.local.as_str() {
            "math" | "mrow" => Ok(Formula::Sequence(children)),
            "munder" | "mover" | "munderover" | "msubsup" => {
                let arity = if matches!(element.name.local.as_str(), "munderover" | "msubsup") {
                    3
                } else {
                    2
                };
                if children.len() != arity {
                    if presentation::attributes(element, location)?
                        .accent
                        .is_some()
                    {
                        return Err(refusal(
                            TransformErrorKind::InvalidStructure,
                            element,
                            location,
                        ));
                    }
                    // MathML Core 3.4.1 and 3.4.2 use rows at other arities.
                    return Ok(Formula::Sequence(children));
                }
                let mut children = children.into_iter();
                let base = Box::new(children.next().unwrap());
                let script = Box::new(children.next().unwrap());
                Ok(match element.name.local.as_str() {
                    "munder" => Formula::Under(base, script),
                    "mover" => Formula::Over(base, script),
                    "msubsup" => Formula::SubSup(base, script, Box::new(children.next().unwrap())),
                    _ => Formula::UnderOver(base, script, Box::new(children.next().unwrap())),
                })
            }
            "msqrt" => Ok(Formula::Sqrt(Box::new(Formula::Sequence(children)))),
            "mroot" if children.len() == 2 => {
                let mut children = children.into_iter();
                let base = children.next().unwrap();
                let index = children.next().unwrap();
                Ok(Formula::Root(Box::new(base), Box::new(index)))
            }
            _ => {
                // Core 3.3.3: mroot outside its two-child arity lays
                // out as a row.
                if element.name.local.as_str() == "mroot" {
                    return Ok(Formula::Sequence(children));
                }
                let [first, second]: [Formula; 2] = children.try_into().map_err(|_| {
                    refusal(TransformErrorKind::InvalidStructure, element, location)
                })?;
                let pair = (Box::new(first), Box::new(second));
                Ok(match element.name.local.as_str() {
                    "mfrac" => Formula::Fraction(pair.0, pair.1),
                    "msub" => Formula::Subscript(pair.0, pair.1),
                    _ => Formula::Superscript(pair.0, pair.1),
                })
            }
        }
    }

    fn token(
        &mut self,
        element: &Element,
        location: &str,
        depth: usize,
    ) -> Result<Formula, TransformError> {
        let mut text = String::new();
        for (index, node) in element.children.iter().enumerate() {
            let path = format!("{location}/children/{index}");
            match node {
                Node::Text(value) => {
                    self.visit(element, &path, depth + 1)?;
                    text.push_str(value);
                }
                Node::Element(child) => {
                    return Err(refusal(TransformErrorKind::InvalidStructure, child, &path));
                }
                // Comments inside a text token annotate its characters and
                // carry no Formula meaning; ms is a literal (its content must
                // stay exact) and an element with no text content model
                // refuses.
                Node::Comment(_)
                    if matches!(element.name.local.as_str(), "mi" | "mn" | "mo" | "mtext") =>
                {
                    self.visit(element, &path, depth + 1)?;
                }
                Node::Comment(_) => {
                    return Err(refusal(
                        TransformErrorKind::UnsupportedContent,
                        element,
                        &path,
                    ));
                }
            }
        }
        if element.name.local == "ms" {
            return Ok(Formula::StringLiteral(text));
        }
        if element.name.local != "mo" {
            return Ok(Formula::Symbol(text));
        }
        // MathML Core strips the token's boundary whitespace before the
        // operator lookup; inner whitespace stays significant.
        let trimmed = text.trim_matches(xml_whitespace);
        if logifos_model::operator::is_operator_text(trimmed) {
            Ok(Formula::Operator(trimmed.to_owned()))
        } else {
            Err(refusal(
                TransformErrorKind::UnsupportedOperator,
                element,
                location,
            ))
        }
    }
}

fn xml_whitespace(character: char) -> bool {
    matches!(character, ' ' | '\t' | '\r' | '\n')
}

/// Parses a CSS <length-percentage>; invalid values are treated as absent
/// per Core's error handling, and percentages resolve to zero extents.
fn parse_length(value: &str) -> Option<Length> {
    let text = value.trim();
    if text.is_empty() {
        return None;
    }
    let split = text
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+'))
        .unwrap_or(text.len());
    let (number, unit) = text.split_at(split);
    let value: f64 = number.parse().ok()?;
    if !value.is_finite() {
        return None;
    }
    let unit = match unit {
        "em" => LengthUnit::Em,
        "ex" => LengthUnit::Ex,
        "px" => LengthUnit::Px,
        "%" => LengthUnit::Percent,
        "0" => LengthUnit::Px,
        _ => return None,
    };
    Some(Length { value, unit })
}
