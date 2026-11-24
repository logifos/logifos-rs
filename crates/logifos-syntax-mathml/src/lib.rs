//! Namespace-aware MathML syntax and explicit conversion into Formula.
//!
//! [`parse`] accepts a complete UTF-8 XML 1.0 document with a `math` root in
//! the MathML namespace (or no namespace for legacy input). Unknown elements
//! and attributes remain syntax data; parsing does not imply rendering support.
//! [`transform`] converts only the current Formula subset and returns a located,
//! typed refusal for information the shared model cannot represent.
//! Prefixes, local namespace declarations, attribute order, comments and mixed
//! content are retained. XML line endings and attribute whitespace are normalized
//! per XML 1.0 sections 2.11 and 3.3.3; character references are decoded. CDATA
//! becomes text without trimming or Unicode normalization. The XML declaration
//! and whitespace outside the document element are not part of the syntax tree.
//!
//! [`write()`] converts Formula back through the syntax model into deterministic
//! MathML. Round trips compare Formula with exact `PartialEq`: child order,
//! sequence grouping, operator spelling and all decoded token text (including
//! whitespace and Unicode) are preserved. Serialization chooses a default
//! MathML namespace, no prefixes, explicit closing tags and no indentation;
//! source namespace declarations, CDATA/reference spelling and whitespace
//! between operands are not reproduced. `mi`, `mn` and `mtext` share Formula's
//! symbol variant and serialize as `mi`. These are syntax choices already
//! erased by the transform, not relaxations of Formula equality. Explicit
//! display and script hints on row-like elements survive exact round trips.
//! Other attributes and comments refuse conversion. A non-sequence Formula root, unsupported
//! operator, invalid XML character or excessive output returns [`WriteError`].
//!
//! The `writer` integration test replays the existing parser case manifest and
//! the MathML inputs in `corpus/conformance/manifest.json`, asserting either an
//! exact Formula round trip or a specific typed refusal. Run
//! `cargo test -p logifos-syntax-mathml --test writer -- --nocapture` for counts.
//! These local fixtures do not establish upstream mathml-stage parity: upstream
//! testsuite inputs are metadata-only and reference corpus pins are unresolved.
//!
//! DTDs, custom entities, processing instructions, XInclude and external resource
//! attributes are refused. The parser has no filesystem or network resolver.
//! Only XML's five predefined references and numeric character references are
//! accepted. Limits apply before allocating each node and cannot be raised above
//! the hard ceilings. Node depth includes text and comments; the root is depth 1.
//!
//! Rules follow [XML 1.0, fifth edition](https://www.w3.org/TR/2008/REC-xml-20081126/)
//! and [Namespaces in XML 1.0, third edition](https://www.w3.org/TR/2009/REC-xml-names-20091208/).

mod error;
mod model;
mod parser;
mod text;
mod transform;
mod writer;

pub use error::{ParseError, ParseErrorKind};
pub use model::{Attribute, Element, MathmlDocument, NamespaceDeclaration, Node, QualifiedName};
pub use parser::{parse, parse_with_limits, ParseLimits};
pub use transform::{transform, TransformError, TransformErrorKind};
pub use writer::{write, WriteError, WriteErrorKind};

/// The MathML namespace URI.
pub const MATHML_NAMESPACE: &str = "http://www.w3.org/1998/Math/MathML";
/// The namespace implicitly bound to the `xml` prefix.
pub const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";
