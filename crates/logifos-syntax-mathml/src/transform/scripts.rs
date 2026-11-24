//! Structural markers are meaningful only inside a multiscript construct.

use logifos_model::ScriptPair;

use super::{
    refusal, xml_whitespace, Element, Formula, Node, TransformError, TransformErrorKind,
    Transformer, MATHML_NAMESPACE,
};

impl Transformer {
    pub(super) fn empty_element(
        &mut self,
        element: &Element,
        location: &str,
        depth: usize,
    ) -> Result<(), TransformError> {
        for (index, node) in element.children.iter().enumerate() {
            let path = format!("{location}/children/{index}");
            match node {
                Node::Text(text) if text.chars().all(xml_whitespace) => {
                    self.visit(element, &path, depth + 1)?;
                }
                Node::Element(child) => {
                    return Err(refusal(TransformErrorKind::InvalidStructure, child, &path));
                }
                _ => {
                    return Err(refusal(
                        TransformErrorKind::UnsupportedContent,
                        element,
                        &path,
                    ))
                }
            }
        }
        Ok(())
    }

    pub(super) fn multiscripts(
        &mut self,
        element: &Element,
        location: &str,
        depth: usize,
    ) -> Result<Formula, TransformError> {
        let mut base = None;
        let mut postscripts = Vec::new();
        let mut prescripts = None;
        // An outer Some means that the subscript slot has been seen;
        // the inner None is an explicit `none` placeholder.
        let mut pending = None;
        for (index, node) in element.children.iter().enumerate() {
            let path = format!("{location}/children/{index}");
            let child = match node {
                Node::Element(child) => child,
                Node::Text(text) if text.chars().all(xml_whitespace) => {
                    self.visit(element, &path, depth + 1)?;
                    continue;
                }
                _ => {
                    return Err(refusal(
                        TransformErrorKind::UnsupportedContent,
                        element,
                        &path,
                    ))
                }
            };
            let marker = matches!(
                child.name.namespace.as_deref(),
                None | Some(MATHML_NAMESPACE)
            ) && matches!(child.name.local.as_str(), "mprescripts" | "none");
            if marker {
                self.check_element(child, &path, depth + 1)?;
                self.empty_element(child, &path, depth + 1)?;
            }
            if marker && child.name.local == "mprescripts" {
                if base.is_none() || pending.is_some() || prescripts.is_some() {
                    return Err(refusal(TransformErrorKind::InvalidStructure, child, &path));
                }
                prescripts = Some(Vec::new());
                continue;
            }
            let value = if marker {
                if base.is_none() {
                    return Err(refusal(TransformErrorKind::InvalidStructure, child, &path));
                }
                None
            } else {
                Some(Box::new(self.element(child, &path, depth + 1)?))
            };
            if base.is_none() {
                base = value;
            } else if let Some(subscript) = pending.take() {
                let pair = ScriptPair {
                    subscript,
                    superscript: value,
                };
                if let Some(prescripts) = &mut prescripts {
                    prescripts.push(pair);
                } else {
                    postscripts.push(pair);
                }
            } else {
                pending = Some(value);
            }
        }
        if pending.is_some() {
            return Err(refusal(
                TransformErrorKind::InvalidStructure,
                element,
                location,
            ));
        }
        let base =
            base.ok_or_else(|| refusal(TransformErrorKind::InvalidStructure, element, location))?;
        Ok(Formula::MultiScripts {
            base,
            postscripts,
            prescripts,
        })
    }
}
