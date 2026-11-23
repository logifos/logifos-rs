//! Formula to box tree

pub mod font;

/// The logifos-layout boundary type.
#[derive(Debug, Clone, PartialEq)]
pub struct BoxTree {
    pub label: String,
}

impl BoxTree {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructs() {
        let node = BoxTree::new("corpus");
        assert_eq!(node.label, "corpus");
    }
}
