//! Conservative conversion from syntax into the current Formula vocabulary.

mod scripts;

use std::fmt;

use logifos_model::{Formula, MAX_DEPTH, MAX_NODES};

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
/// Formula has one shared text/identifier class. `mo` accepts only `+`, `-`, `=`,
/// `*` and `×`, with no trimming, substitution or symbol fallback. `mfrac`, `msub`
/// and `msup` require exactly two element operands in source order. `msqrt`
/// wraps every child in one radicand; `mroot` takes base then index and falls
/// back to a row outside its two-child arity. `munder` and `mover` preserve
/// base then script; `munderover` preserves base, underscript then overscript.
/// Under/over elements outside their two- or three-child arity become rows.
///
/// Only XML whitespace between operands is insignificant. Namespace declarations
/// identify names and carry no Formula presentation data. Every non-namespace
/// attribute, comment, other element, nested token markup and non-whitespace text
/// outside tokens refuses the entire conversion. No partial tree is returned.
/// `msubsup` preserves base, subscript then superscript, with row semantics at
/// other arities. Multiscripts preserve paired slots and an optional prescript
/// boundary; malformed pairs and nonempty markers refuse typed. `none` is
/// accepted only in multiscript slots. Attribute-free `mspace` and `ms` retain
/// their distinct kinds; dimensions and custom quotes remain unsupported.
/// Fences, tables, style, enclosure, semantics and annotations remain unsupported.
/// Names in either the MathML namespace or no namespace are accepted.
///
/// Traversal is bounded by the shared model's root-inclusive depth and node
/// limits, also counting syntax text nodes and insignificant whitespace. These
/// checks apply to manually constructed syntax trees as well as parsed input.
pub fn transform(document: &MathmlDocument) -> Result<Formula, TransformError> {
    for (comments, path) in [
        (&document.before_root, "/before_root/0"),
        (&document.after_root, "/after_root/0"),
    ] {
        if !comments.is_empty() {
            return Err(refusal(
                TransformErrorKind::UnsupportedContent,
                &document.root,
                path,
            ));
        }
    }
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

    fn element(
        &mut self,
        element: &Element,
        location: &str,
        depth: usize,
    ) -> Result<Formula, TransformError> {
        self.check_element(element, location, depth)?;
        match element.name.local.as_str() {
            "mmultiscripts" => self.multiscripts(element, location, depth),
            "mspace" => {
                self.empty_element(element, location, depth)?;
                Ok(Formula::Space)
            }
            "mi" | "mn" | "mtext" | "mo" | "ms" => self.token(element, location, depth),
            "math" if depth != 1 => Err(refusal(
                TransformErrorKind::UnsupportedElement,
                element,
                location,
            )),
            "math" | "mrow" | "mfrac" | "msub" | "msup" | "msqrt" | "mroot" | "munder"
            | "mover" | "munderover" | "msubsup" => self.construct(element, location, depth),
            _ => Err(refusal(
                TransformErrorKind::UnsupportedElement,
                element,
                location,
            )),
        }
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
        if let Some(attribute) = element.attributes.first() {
            let mut error = refusal(
                TransformErrorKind::UnsupportedAttribute,
                element,
                &format!("{location}/attributes/0"),
            );
            error.attribute = Some(Box::new(attribute.name.clone()));
            return Err(error);
        }
        Ok(())
    }

    fn construct(
        &mut self,
        element: &Element,
        location: &str,
        depth: usize,
    ) -> Result<Formula, TransformError> {
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
        match element.name.local.as_str() {
            "math" | "mrow" => Ok(Formula::Sequence(children)),
            "munder" | "mover" | "munderover" | "msubsup" => {
                let arity = if matches!(element.name.local.as_str(), "munderover" | "msubsup") {
                    3
                } else {
                    2
                };
                if children.len() != arity {
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
        match text.as_str() {
            "+" | "-" | "=" | "*" | "×" => Ok(Formula::Operator(text)),
            _ => Err(refusal(
                TransformErrorKind::UnsupportedOperator,
                element,
                location,
            )),
        }
    }
}

fn xml_whitespace(character: char) -> bool {
    matches!(character, ' ' | '\t' | '\r' | '\n')
}
