use std::sync::Arc;

/// An XML name retaining both its spelling and resolved namespace identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualifiedName {
    /// The source prefix, or `None` for an unprefixed name.
    pub prefix: Option<String>,
    /// The local part of the name.
    pub local: String,
    /// The resolved namespace URI, shared with its declaration to bound storage.
    /// `None` means no namespace.
    pub namespace: Option<Arc<str>>,
}

/// A namespace declaration made on this element, in source order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamespaceDeclaration {
    /// `None` denotes a default namespace declaration.
    pub prefix: Option<String>,
    /// An empty URI explicitly clears the default namespace.
    pub uri: Arc<str>,
}

/// A non-namespace XML attribute, in source order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    /// Namespace declarations are stored separately on the element.
    pub name: QualifiedName,
    /// XML-normalized and reference-decoded value.
    pub value: String,
}

/// An element retaining syntax without assigning mathematical meaning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    /// Original qualified name and its namespace identity.
    pub name: QualifiedName,
    /// Declarations local to this element; inherited bindings are not duplicated.
    pub namespaces: Vec<NamespaceDeclaration>,
    /// All non-namespace attributes, including unknown presentation attributes.
    pub attributes: Vec<Attribute>,
    /// Ordered content, including whitespace-only text and comments.
    pub children: Vec<Node>,
}

/// A syntax node. Text and CDATA runs stay at their original positions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    /// A child element.
    Element(Element),
    /// Decoded character data, including significant whitespace.
    Text(String),
    /// Comment content without its delimiters.
    Comment(String),
}

/// A complete document with exactly one MathML root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MathmlDocument {
    /// Comments before the root, in source order.
    pub before_root: Vec<String>,
    /// The `math` document element.
    pub root: Element,
    /// Comments after the root, in source order.
    pub after_root: Vec<String>,
}
