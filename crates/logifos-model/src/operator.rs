//! The MathML Core operator dictionary (3.2.4.2 and appendix B.1): a
//! category for each (content, form) pair and the spacing it implies.
//! Tables are transcribed from the compact dictionary, figure 25.

/// Operator form, from the operator's position in its row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperatorForm {
    Infix,
    Prefix,
    Postfix,
}

/// A dictionary category (figure 26) or the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperatorCategory {
    Default,
    ForceDefault,
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
}

impl OperatorCategory {
    /// Leading and trailing space in 1/18 em (figure 26).
    pub fn spacing_units(self) -> (f64, f64) {
        match self {
            Self::Default | Self::ForceDefault | Self::A => (5.0, 5.0),
            Self::B => (4.0, 4.0),
            Self::C => (3.0, 3.0),
            Self::H | Self::J => (3.0, 3.0),
            Self::L => (3.0, 0.0),
            Self::M => (0.0, 3.0),
            Self::D | Self::E | Self::F | Self::G | Self::I | Self::K => (0.0, 0.0),
        }
    }

    /// Whether the operator is a large operator (`largeop`).
    pub fn is_large(self) -> bool {
        matches!(self, Self::H | Self::J)
    }

    /// Whether the operator also has `movablelimits` (category J: sums,
    /// products and the n-ary set operators, but not integrals).
    pub fn is_movable_limits(self) -> bool {
        matches!(self, Self::J)
    }
}

use OperatorCategory as Category;

/// Whether `text` can be the content of an operator token: possibly empty
/// (an empty `mo` is valid MathML), with no ASCII whitespace, control or
/// invisible-operator characters.
pub fn is_operator_text(text: &str) -> bool {
    text.chars().all(|c| {
        !c.is_ascii_whitespace() && !c.is_control() && !('\u{2061}'..='\u{2064}').contains(&c)
    })
}

const ASCII_PAIRS: [&str; 18] = [
    "!!", "!=", "&&", "**", "*=", "++", "+=", "--", "-=", "->", "//", "/=", ":=", "<=", "<>", "==",
    ">=", "||",
];

/// The category of `(text, form)`, following the "algorithm to determine the
/// category of an operator".
pub fn category(text: &str, form: OperatorForm) -> OperatorCategory {
    let mut chars = text.chars();
    let (Some(first), second, None) = (chars.next(), chars.next(), chars.next()) else {
        return Category::Default;
    };
    let key = match second {
        None => {
            if ('\u{0320}'..='\u{03ff}').contains(&first) {
                return Category::Default;
            }
            first as u32
        }
        Some('\u{0338}' | '\u{20d2}') => first as u32,
        Some(_) => match ASCII_PAIRS.iter().position(|pair| *pair == text) {
            Some(index) => 0x0320 + index as u32,
            None => return Category::Default,
        },
    };
    if form == OperatorForm::Infix && matches!(key, 0x7c | 0x223c) {
        return Category::ForceDefault;
    }
    let table = match form {
        OperatorForm::Infix => INFIX,
        OperatorForm::Prefix => PREFIX,
        OperatorForm::Postfix => POSTFIX,
    };
    table
        .iter()
        .find(|(lo, hi, _)| (*lo..=*hi).contains(&key))
        .map_or(Category::Default, |entry| entry.2)
}

/// The category with the dictionary fallback for an operator whose form was
/// not given explicitly: the position form, then infix, postfix and prefix.
pub fn resolve(text: &str, form: OperatorForm) -> OperatorCategory {
    [
        form,
        OperatorForm::Infix,
        OperatorForm::Postfix,
        OperatorForm::Prefix,
    ]
    .into_iter()
    .map(|candidate| category(text, candidate))
    .find(|found| *found != Category::Default)
    .unwrap_or(Category::Default)
}

const INFIX: &[(u32, u32, Category)] = &[
    (0x0025, 0x0025, Category::C),
    (0x002A, 0x002A, Category::C),
    (0x002B, 0x002B, Category::B),
    (0x002C, 0x002C, Category::M),
    (0x002D, 0x002D, Category::B),
    (0x002E, 0x002E, Category::C),
    (0x002F, 0x002F, Category::K),
    (0x003A, 0x003A, Category::M),
    (0x003B, 0x003B, Category::M),
    (0x003F, 0x0040, Category::C),
    (0x005C, 0x005C, Category::K),
    (0x005E, 0x005E, Category::C),
    (0x005F, 0x005F, Category::K),
    (0x00B1, 0x00B1, Category::B),
    (0x00B7, 0x00B7, Category::C),
    (0x00D7, 0x00D7, Category::C),
    (0x00F7, 0x00F7, Category::B),
    (0x0322, 0x0322, Category::B),
    (0x0323, 0x0323, Category::C),
    (0x032E, 0x032E, Category::C),
    (0x2022, 0x2022, Category::C),
    (0x2043, 0x2043, Category::C),
    (0x2044, 0x2044, Category::B),
    (0x2061, 0x2064, Category::K),
    (0x2190, 0x2195, Category::A),
    (0x219A, 0x21AE, Category::A),
    (0x21B0, 0x21B5, Category::A),
    (0x21B9, 0x21B9, Category::A),
    (0x21BC, 0x21D5, Category::A),
    (0x21DA, 0x21F0, Category::A),
    (0x21F3, 0x21FF, Category::A),
    (0x2206, 0x2206, Category::K),
    (0x2212, 0x2216, Category::B),
    (0x2217, 0x2219, Category::C),
    (0x2227, 0x222A, Category::B),
    (0x2236, 0x2236, Category::B),
    (0x2238, 0x2238, Category::B),
    (0x2240, 0x2240, Category::C),
    (0x228C, 0x228E, Category::B),
    (0x2293, 0x2296, Category::B),
    (0x2297, 0x2297, Category::C),
    (0x2298, 0x2298, Category::B),
    (0x2299, 0x229B, Category::C),
    (0x229D, 0x229F, Category::B),
    (0x22A0, 0x22A1, Category::C),
    (0x22BA, 0x22BA, Category::C),
    (0x22BB, 0x22BD, Category::B),
    (0x22C4, 0x22C7, Category::C),
    (0x22C9, 0x22CC, Category::C),
    (0x22CE, 0x22CF, Category::B),
    (0x22D2, 0x22D3, Category::B),
    (0x2305, 0x2306, Category::C),
    (0x2794, 0x2794, Category::A),
    (0x2795, 0x2797, Category::B),
    (0x2799, 0x2799, Category::A),
    (0x279B, 0x27A1, Category::A),
    (0x27A5, 0x27A6, Category::A),
    (0x27A8, 0x27AF, Category::A),
    (0x27B1, 0x27B1, Category::A),
    (0x27B3, 0x27B3, Category::A),
    (0x27B5, 0x27B5, Category::A),
    (0x27B8, 0x27B8, Category::A),
    (0x27BA, 0x27BE, Category::A),
    (0x27CB, 0x27CB, Category::C),
    (0x27CD, 0x27CD, Category::C),
    (0x27F0, 0x27F1, Category::A),
    (0x27F4, 0x27FF, Category::A),
    (0x2900, 0x2920, Category::A),
    (0x2934, 0x2937, Category::A),
    (0x2942, 0x2975, Category::A),
    (0x297C, 0x297F, Category::A),
    (0x29B8, 0x29B8, Category::B),
    (0x29BC, 0x29BC, Category::B),
    (0x29C4, 0x29C5, Category::B),
    (0x29C6, 0x29C8, Category::C),
    (0x29D4, 0x29D7, Category::C),
    (0x29E2, 0x29E2, Category::C),
    (0x29F5, 0x29FB, Category::B),
    (0x2A1D, 0x2A1E, Category::C),
    (0x2A1F, 0x2A2E, Category::B),
    (0x2A2F, 0x2A37, Category::C),
    (0x2A38, 0x2A3A, Category::B),
    (0x2A3B, 0x2A3D, Category::C),
    (0x2A3E, 0x2A3E, Category::B),
    (0x2A3F, 0x2A3F, Category::C),
    (0x2A40, 0x2A4F, Category::B),
    (0x2A50, 0x2A50, Category::C),
    (0x2A51, 0x2A63, Category::B),
    (0x2A64, 0x2A65, Category::C),
    (0x2ADB, 0x2ADB, Category::B),
    (0x2ADC, 0x2ADD, Category::C),
    (0x2AF6, 0x2AF6, Category::B),
    (0x2AFB, 0x2AFB, Category::B),
    (0x2AFD, 0x2AFD, Category::B),
    (0x2AFE, 0x2AFE, Category::C),
    (0x2B04, 0x2B07, Category::A),
    (0x2B0C, 0x2B11, Category::A),
    (0x2B30, 0x2B3E, Category::A),
    (0x2B40, 0x2B4C, Category::A),
    (0x2B60, 0x2B65, Category::A),
    (0x2B6A, 0x2B6D, Category::A),
    (0x2B70, 0x2B73, Category::A),
    (0x2B7A, 0x2B7D, Category::A),
    (0x2B80, 0x2B87, Category::A),
    (0x2B95, 0x2B95, Category::A),
    (0x2BA0, 0x2BAF, Category::A),
    (0x2BB8, 0x2BB8, Category::A),
];

const PREFIX: &[(u32, u32, Category)] = &[
    (0x0021, 0x0021, Category::D),
    (0x0028, 0x0028, Category::F),
    (0x002B, 0x002B, Category::D),
    (0x002D, 0x002D, Category::D),
    (0x005B, 0x005B, Category::F),
    (0x007B, 0x007B, Category::F),
    (0x007C, 0x007C, Category::F),
    (0x00AC, 0x00AC, Category::D),
    (0x00B1, 0x00B1, Category::D),
    (0x0331, 0x0331, Category::D),
    (0x2016, 0x2016, Category::F),
    (0x2018, 0x2018, Category::D),
    (0x201C, 0x201C, Category::D),
    (0x2145, 0x2146, Category::L),
    (0x2200, 0x2201, Category::D),
    (0x2202, 0x2202, Category::L),
    (0x2203, 0x2204, Category::D),
    (0x2207, 0x2207, Category::D),
    (0x220F, 0x2211, Category::J),
    (0x2212, 0x2213, Category::D),
    (0x221A, 0x221C, Category::L),
    (0x221F, 0x2222, Category::D),
    (0x222B, 0x2233, Category::H),
    (0x2234, 0x2235, Category::D),
    (0x223C, 0x223C, Category::D),
    (0x22BE, 0x22BF, Category::D),
    (0x22C0, 0x22C3, Category::J),
    (0x2308, 0x2308, Category::F),
    (0x230A, 0x230A, Category::F),
    (0x2310, 0x2310, Category::D),
    (0x2319, 0x2319, Category::D),
    (0x2329, 0x2329, Category::F),
    (0x2772, 0x2772, Category::F),
    (0x2795, 0x2796, Category::D),
    (0x27C0, 0x27C0, Category::D),
    (0x27E6, 0x27E6, Category::F),
    (0x27E8, 0x27E8, Category::F),
    (0x27EA, 0x27EA, Category::F),
    (0x27EC, 0x27EC, Category::F),
    (0x27EE, 0x27EE, Category::F),
    (0x2980, 0x2980, Category::F),
    (0x2983, 0x2983, Category::F),
    (0x2985, 0x2985, Category::F),
    (0x2987, 0x2987, Category::F),
    (0x2989, 0x2989, Category::F),
    (0x298B, 0x298B, Category::F),
    (0x298D, 0x298D, Category::F),
    (0x298F, 0x298F, Category::F),
    (0x2991, 0x2991, Category::F),
    (0x2993, 0x2993, Category::F),
    (0x2995, 0x2995, Category::F),
    (0x2997, 0x2997, Category::F),
    (0x2999, 0x2999, Category::F),
    (0x299B, 0x29AF, Category::D),
    (0x29D8, 0x29D8, Category::F),
    (0x29DA, 0x29DA, Category::F),
    (0x29FC, 0x29FC, Category::F),
    (0x2A00, 0x2A0A, Category::J),
    (0x2A0B, 0x2A1C, Category::H),
    (0x2A1D, 0x2A1E, Category::J),
    (0x2AEC, 0x2AED, Category::D),
    (0x2AFC, 0x2AFC, Category::J),
    (0x2AFF, 0x2AFF, Category::J),
];

const POSTFIX: &[(u32, u32, Category)] = &[
    (0x0021, 0x0022, Category::E),
    (0x0025, 0x0027, Category::E),
    (0x0029, 0x0029, Category::G),
    (0x005D, 0x005D, Category::G),
    (0x005E, 0x005F, Category::I),
    (0x0060, 0x0060, Category::E),
    (0x007C, 0x007C, Category::G),
    (0x007D, 0x007D, Category::G),
    (0x007E, 0x007E, Category::I),
    (0x00A8, 0x00A8, Category::E),
    (0x00AF, 0x00AF, Category::I),
    (0x00B0, 0x00B0, Category::E),
    (0x00B2, 0x00B4, Category::E),
    (0x00B8, 0x00B9, Category::E),
    (0x02C6, 0x02C7, Category::I),
    (0x02C9, 0x02C9, Category::I),
    (0x02CA, 0x02CB, Category::E),
    (0x02CD, 0x02CD, Category::I),
    (0x02D8, 0x02DA, Category::E),
    (0x02DC, 0x02DC, Category::I),
    (0x02DD, 0x02DD, Category::E),
    (0x02F7, 0x02F7, Category::I),
    (0x0302, 0x0302, Category::I),
    (0x0311, 0x0311, Category::E),
    (0x0320, 0x0320, Category::E),
    (0x0325, 0x0325, Category::E),
    (0x0327, 0x0327, Category::E),
    (0x0331, 0x0331, Category::E),
    (0x2016, 0x2016, Category::G),
    (0x2019, 0x201B, Category::E),
    (0x201D, 0x201F, Category::E),
    (0x2032, 0x2037, Category::E),
    (0x203E, 0x203E, Category::I),
    (0x2057, 0x2057, Category::E),
    (0x20DB, 0x20DC, Category::E),
    (0x2309, 0x2309, Category::G),
    (0x230B, 0x230B, Category::G),
    (0x2322, 0x2323, Category::I),
    (0x232A, 0x232A, Category::G),
    (0x23B4, 0x23B5, Category::I),
    (0x23CD, 0x23CD, Category::E),
    (0x23DC, 0x23E1, Category::I),
    (0x2773, 0x2773, Category::G),
    (0x27E7, 0x27E7, Category::G),
    (0x27E9, 0x27E9, Category::G),
    (0x27EB, 0x27EB, Category::G),
    (0x27ED, 0x27ED, Category::G),
    (0x27EF, 0x27EF, Category::G),
    (0x2980, 0x2980, Category::G),
    (0x2984, 0x2984, Category::G),
    (0x2986, 0x2986, Category::G),
    (0x2988, 0x2988, Category::G),
    (0x298A, 0x298A, Category::G),
    (0x298C, 0x298C, Category::G),
    (0x298E, 0x298E, Category::G),
    (0x2990, 0x2990, Category::G),
    (0x2992, 0x2992, Category::G),
    (0x2994, 0x2994, Category::G),
    (0x2996, 0x2996, Category::G),
    (0x2998, 0x2998, Category::G),
    (0x2999, 0x2999, Category::G),
    (0x29D9, 0x29D9, Category::G),
    (0x29DB, 0x29DB, Category::G),
    (0x29FD, 0x29FD, Category::G),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categories_follow_the_dictionary() {
        let cases = [
            ("\u{2264}", OperatorForm::Infix, Category::Default),
            ("\u{2192}", OperatorForm::Infix, Category::A),
            ("\u{b1}", OperatorForm::Infix, Category::B),
            ("\u{b1}", OperatorForm::Prefix, Category::D),
            ("\u{22c5}", OperatorForm::Infix, Category::C),
            ("\u{2211}", OperatorForm::Prefix, Category::J),
            ("\u{222b}", OperatorForm::Prefix, Category::H),
            ("\u{af}", OperatorForm::Postfix, Category::I),
            (",", OperatorForm::Infix, Category::M),
            ("(", OperatorForm::Prefix, Category::F),
            ("|", OperatorForm::Infix, Category::ForceDefault),
            ("<=", OperatorForm::Infix, Category::Default),
            ("lim", OperatorForm::Infix, Category::Default),
        ];
        for (text, form, expected) in cases {
            assert_eq!(category(text, form), expected, "{text:?} {form:?}");
        }
    }

    #[test]
    fn resolution_falls_back_through_the_other_forms() {
        assert_eq!(resolve("+", OperatorForm::Postfix), Category::B);
        assert_eq!(resolve("\u{af}", OperatorForm::Infix), Category::I);
        assert_eq!(resolve("x", OperatorForm::Infix), Category::Default);
        assert_eq!(Category::M.spacing_units(), (0.0, 3.0));
        assert_eq!(Category::L.spacing_units(), (3.0, 0.0));
        assert!(Category::J.is_large() && !Category::B.is_large());
        assert!(Category::J.is_movable_limits() && !Category::H.is_movable_limits());
    }

    #[test]
    fn operator_text_refuses_ascii_space_control_and_invisible_operators() {
        for text in ["\u{2192}", "lim", "..", "\u{2004}", ""] {
            assert!(is_operator_text(text), "{text:?}");
        }
        for text in [" + ", "a b", "\u{2062}", "\n", "\u{0}"] {
            assert!(!is_operator_text(text), "{text:?}");
        }
    }
}
