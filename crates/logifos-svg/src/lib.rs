//! Deterministic SVG emission from the box tree

/// The logifos-svg boundary type.
#[derive(Debug, Clone, PartialEq)]
pub struct SvgDocument {
    pub label: String,
}

impl SvgDocument {
    pub fn new(label: impl Into<String>) -> Self {
        Self { label: label.into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructs() {
        let node = SvgDocument::new("corpus");
        assert_eq!(node.label, "corpus");
    }
}
