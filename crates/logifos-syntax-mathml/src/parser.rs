use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use xmlparser::{ElementEnd, StrSpan, Token, Tokenizer};

use crate::text::{decode, normalize_lines, xml_char};
use crate::{
    Attribute, Element, MathmlDocument, NamespaceDeclaration, Node, ParseError, ParseErrorKind,
    QualifiedName, MATHML_NAMESPACE, XML_NAMESPACE,
};

const XMLNS_NAMESPACE: &str = "http://www.w3.org/2000/xmlns/";
const XINCLUDE_NAMESPACE: &str = "http://www.w3.org/2001/XInclude";
const XLINK_NAMESPACE: &str = "http://www.w3.org/1999/xlink";

/// Per-call bounds. Each value must be in `1..=ParseLimits::default()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseLimits {
    /// Source length in UTF-8 bytes; hard ceiling 1,048,576.
    pub source_bytes: usize,
    /// Elements, text runs and comments; hard ceiling 100,000.
    pub tree_nodes: usize,
    /// Root depth is 1, including text/comment leaves; hard ceiling 128.
    pub tree_depth: usize,
}

impl Default for ParseLimits {
    fn default() -> Self {
        Self {
            source_bytes: 1_048_576,
            tree_nodes: 100_000,
            tree_depth: 128,
        }
    }
}

/// Parses a document using the hard resource ceilings.
pub fn parse(source: impl AsRef<[u8]>) -> Result<MathmlDocument, ParseError> {
    parse_with_limits(source, ParseLimits::default())
}

/// Parses a complete document with lowered per-call resource bounds.
///
/// Bytes are checked before UTF-8 decoding; depth and node count are checked
/// before node allocation. No declaration can enable resource resolution.
pub fn parse_with_limits(
    source: impl AsRef<[u8]>,
    limits: ParseLimits,
) -> Result<MathmlDocument, ParseError> {
    let hard = ParseLimits::default();
    if limits.source_bytes == 0
        || limits.source_bytes > hard.source_bytes
        || limits.tree_nodes == 0
        || limits.tree_nodes > hard.tree_nodes
        || limits.tree_depth == 0
        || limits.tree_depth > hard.tree_depth
    {
        return Err(ParseError::new(
            ParseErrorKind::InvalidLimits,
            0,
            "invalid parser limits",
        ));
    }
    let bytes = source.as_ref();
    if bytes.len() > limits.source_bytes {
        return Err(ParseError::new(
            ParseErrorKind::ResourceLimit,
            limits.source_bytes,
            "source byte limit exceeded",
        ));
    }
    let source = std::str::from_utf8(bytes).map_err(|err| {
        ParseError::new(
            ParseErrorKind::InvalidUtf8,
            err.valid_up_to(),
            "source is not UTF-8",
        )
    })?;
    if let Some((offset, _)) = source.char_indices().find(|(_, ch)| !xml_char(*ch)) {
        return Err(ParseError::new(
            ParseErrorKind::MalformedXml,
            offset,
            "invalid XML character",
        ));
    }
    Builder::new(limits).parse(source)
}

struct RawAttribute<'a> {
    prefix: StrSpan<'a>,
    local: StrSpan<'a>,
    value: StrSpan<'a>,
}

struct Header<'a> {
    prefix: StrSpan<'a>,
    local: StrSpan<'a>,
    attributes: Vec<RawAttribute<'a>>,
}

struct OpenElement {
    element: Element,
    bindings: BTreeMap<String, Arc<str>>,
}

struct Builder {
    limits: ParseLimits,
    nodes: usize,
    stack: Vec<OpenElement>,
    root: Option<Element>,
    before_root: Vec<String>,
    after_root: Vec<String>,
}

impl Builder {
    fn new(limits: ParseLimits) -> Self {
        Self {
            limits,
            nodes: 0,
            stack: Vec::new(),
            root: None,
            before_root: Vec::new(),
            after_root: Vec::new(),
        }
    }

    fn reserve_node(&mut self, offset: usize) -> Result<(), ParseError> {
        if self.nodes >= self.limits.tree_nodes || self.stack.len() >= self.limits.tree_depth {
            return Err(ParseError::new(
                ParseErrorKind::ResourceLimit,
                offset,
                "tree node or depth limit exceeded",
            ));
        }
        self.nodes += 1;
        Ok(())
    }

    fn parse(mut self, source: &str) -> Result<MathmlDocument, ParseError> {
        let mut tokenizer = Tokenizer::from(source);
        let mut pending: Option<Header<'_>> = None;
        while let Some(token) = tokenizer.next() {
            let token = token.map_err(|_| malformed(tokenizer.stream().pos()))?;
            let offset = token.span().start();
            match token {
                Token::Declaration {
                    version, encoding, ..
                } => {
                    if version.as_str() != "1.0"
                        || encoding.is_some_and(|e| !e.eq_ignore_ascii_case("UTF-8"))
                    {
                        return Err(ParseError::new(
                            ParseErrorKind::UnsupportedSyntax,
                            offset,
                            "only UTF-8 XML 1.0 is supported",
                        ));
                    }
                }
                Token::DtdStart { .. }
                | Token::EmptyDtd { .. }
                | Token::EntityDeclaration { .. }
                | Token::DtdEnd { .. } => {
                    return Err(ParseError::new(
                        ParseErrorKind::UnsafeXml,
                        offset,
                        "DTDs and entity declarations are prohibited",
                    ));
                }
                Token::ProcessingInstruction { .. } => {
                    return Err(ParseError::new(
                        ParseErrorKind::UnsafeXml,
                        offset,
                        "processing instructions are prohibited",
                    ));
                }
                Token::ElementStart { prefix, local, .. } => {
                    self.reserve_node(offset)?;
                    if pending.is_some() || (self.stack.is_empty() && self.root.is_some()) {
                        return Err(malformed(offset));
                    }
                    pending = Some(Header {
                        prefix,
                        local,
                        attributes: Vec::new(),
                    });
                }
                Token::Attribute {
                    prefix,
                    local,
                    value,
                    ..
                } => {
                    pending
                        .as_mut()
                        .ok_or_else(|| malformed(offset))?
                        .attributes
                        .push(RawAttribute {
                            prefix,
                            local,
                            value,
                        });
                }
                Token::ElementEnd { end, .. } => match end {
                    ElementEnd::Open | ElementEnd::Empty => {
                        let element =
                            self.element(pending.take().ok_or_else(|| malformed(offset))?)?;
                        if end == ElementEnd::Empty {
                            self.append_element(element.element, offset)?;
                        } else {
                            self.stack.push(element);
                        }
                    }
                    ElementEnd::Close(prefix, local) => {
                        let element = self.stack.pop().ok_or_else(|| malformed(offset))?;
                        if element.element.name.prefix.as_deref().unwrap_or("") != prefix.as_str()
                            || element.element.name.local != local.as_str()
                        {
                            return Err(malformed(offset));
                        }
                        self.append_element(element.element, offset)?;
                    }
                },
                Token::Text { text } => {
                    if self.stack.is_empty() {
                        if !text
                            .chars()
                            .all(|ch| matches!(ch, ' ' | '\t' | '\n' | '\r'))
                        {
                            return Err(malformed(offset));
                        }
                    } else {
                        self.reserve_node(offset)?;
                        let text = decode(text.as_str(), text.start(), false)?;
                        self.stack
                            .last_mut()
                            .ok_or_else(|| malformed(offset))?
                            .element
                            .children
                            .push(Node::Text(text));
                    }
                }
                Token::Cdata { text, .. } => {
                    self.reserve_node(offset)?;
                    self.stack
                        .last_mut()
                        .ok_or_else(|| malformed(offset))?
                        .element
                        .children
                        .push(Node::Text(normalize_lines(text.as_str())));
                }
                Token::Comment { text, .. } => {
                    self.reserve_node(offset)?;
                    let text = normalize_lines(text.as_str());
                    if let Some(parent) = self.stack.last_mut() {
                        parent.element.children.push(Node::Comment(text));
                    } else if self.root.is_some() {
                        self.after_root.push(text);
                    } else {
                        self.before_root.push(text);
                    }
                }
            }
        }
        if !self.stack.is_empty() || pending.is_some() {
            return Err(malformed(source.len()));
        }
        Ok(MathmlDocument {
            root: self.root.ok_or_else(|| malformed(source.len()))?,
            before_root: self.before_root,
            after_root: self.after_root,
        })
    }

    fn append_element(&mut self, element: Element, offset: usize) -> Result<(), ParseError> {
        if let Some(parent) = self.stack.last_mut() {
            parent.element.children.push(Node::Element(element));
        } else if self.root.is_none() {
            self.root = Some(element);
        } else {
            return Err(malformed(offset));
        }
        Ok(())
    }

    fn element(&self, header: Header<'_>) -> Result<OpenElement, ParseError> {
        let mut namespaces = Vec::new();
        let mut bindings = BTreeMap::new();
        let mut raw_names = BTreeSet::new();
        for attr in &header.attributes {
            if !raw_names.insert((attr.prefix.as_str(), attr.local.as_str())) {
                return Err(malformed(attr.local.start()));
            }
            let declaration = if attr.prefix.as_str() == "xmlns" {
                Some(Some(attr.local.as_str()))
            } else if attr.prefix.is_empty() && attr.local.as_str() == "xmlns" {
                Some(None)
            } else {
                None
            };
            if let Some(prefix) = declaration {
                let uri = decode(attr.value.as_str(), attr.value.start(), true)?;
                if prefix == Some("xmlns")
                    || uri == XMLNS_NAMESPACE
                    || (prefix == Some("xml")) != (uri == XML_NAMESPACE)
                    || (prefix.is_some() && uri.is_empty())
                {
                    return Err(malformed(attr.local.start()));
                }
                let uri: Arc<str> = uri.into();
                bindings.insert(prefix.unwrap_or("").to_owned(), Arc::clone(&uri));
                namespaces.push(NamespaceDeclaration {
                    prefix: prefix.map(str::to_owned),
                    uri,
                });
            }
        }
        let name = self.name(header.prefix, header.local, &bindings, false)?;
        if self.stack.is_empty()
            && (name.local != "math"
                || name
                    .namespace
                    .as_deref()
                    .is_some_and(|uri| uri != MATHML_NAMESPACE))
        {
            return Err(ParseError::new(
                ParseErrorKind::UnsupportedSyntax,
                header.local.start(),
                "expected a MathML math root",
            ));
        }
        if name.namespace.as_deref() == Some(XINCLUDE_NAMESPACE) {
            return Err(ParseError::new(
                ParseErrorKind::UnsafeXml,
                header.local.start(),
                "XInclude is prohibited",
            ));
        }
        let mut attributes = Vec::new();
        let mut expanded_names = BTreeSet::new();
        for attr in header.attributes {
            if attr.prefix.as_str() == "xmlns"
                || (attr.prefix.is_empty() && attr.local.as_str() == "xmlns")
            {
                continue;
            }
            let name = self.name(attr.prefix, attr.local, &bindings, true)?;
            if !expanded_names.insert((name.local.clone(), name.namespace.clone())) {
                return Err(malformed(attr.local.start()));
            }
            // This syntax boundary does not accept resource locators. Namespace
            // URIs are identities; they never cause resolution.
            if (name.namespace.is_none()
                && matches!(name.local.as_str(), "href" | "src" | "altimg"))
                || (name.namespace.as_deref() == Some(XLINK_NAMESPACE) && name.local == "href")
                || (name.namespace.as_deref() == Some(XML_NAMESPACE) && name.local == "base")
            {
                return Err(ParseError::new(
                    ParseErrorKind::UnsafeXml,
                    attr.local.start(),
                    "external resource attributes are prohibited",
                ));
            }
            attributes.push(Attribute {
                name,
                value: decode(attr.value.as_str(), attr.value.start(), true)?,
            });
        }
        Ok(OpenElement {
            element: Element {
                name,
                namespaces,
                attributes,
                children: Vec::new(),
            },
            bindings,
        })
    }

    fn name(
        &self,
        prefix: StrSpan<'_>,
        local: StrSpan<'_>,
        declarations: &BTreeMap<String, Arc<str>>,
        attribute: bool,
    ) -> Result<QualifiedName, ParseError> {
        if prefix.as_str() == "xmlns" || local.contains(':') {
            return Err(malformed(local.start()));
        }
        let prefix = (!prefix.is_empty()).then_some(prefix.as_str());
        let namespace = if prefix == Some("xml") {
            Some(Arc::from(XML_NAMESPACE))
        } else if attribute && prefix.is_none() {
            None
        } else {
            let key = prefix.unwrap_or("");
            let uri = declarations.get(key).or_else(|| {
                self.stack
                    .iter()
                    .rev()
                    .find_map(|open| open.bindings.get(key))
            });
            if prefix.is_some() && uri.is_none() {
                return Err(malformed(local.start()));
            }
            uri.filter(|uri| !uri.is_empty()).cloned()
        };
        Ok(QualifiedName {
            prefix: prefix.map(str::to_owned),
            local: local.as_str().to_owned(),
            namespace,
        })
    }
}

fn malformed(offset: usize) -> ParseError {
    ParseError::new(
        ParseErrorKind::MalformedXml,
        offset,
        "malformed XML structure or namespace",
    )
}
