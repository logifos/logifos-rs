//! MathML syntax model

/// The logifos-syntax-mathml boundary type.
#[derive(Debug, Clone, PartialEq)]
pub struct MathmlDocument {
    pub label: String,
}

impl MathmlDocument {
    pub fn new(label: impl Into<String>) -> Self {
        Self { label: label.into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructs() {
        let node = MathmlDocument::new("corpus");
        assert_eq!(node.label, "corpus");
    }
}
