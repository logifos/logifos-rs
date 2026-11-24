//! `mathvariant` as the Unicode Mathematical Alphanumeric Symbols mapping
//! (Unicode block U+1D400..U+1D7FF). Characters without a styled form in the
//! requested variant stay as written; font coverage is checked by the caller.

use logifos_model::MathVariant;

/// Reserved holes in the block, whose characters live in Letterlike Symbols.
const ITALIC_HOLES: &[(char, char)] = &[('h', '\u{210e}')];
const SCRIPT_HOLES: &[(char, char)] = &[
    ('B', '\u{212c}'),
    ('E', '\u{2130}'),
    ('F', '\u{2131}'),
    ('H', '\u{210b}'),
    ('I', '\u{2110}'),
    ('L', '\u{2112}'),
    ('M', '\u{2133}'),
    ('R', '\u{211b}'),
    ('e', '\u{212f}'),
    ('g', '\u{210a}'),
    ('o', '\u{2134}'),
];
const FRAKTUR_HOLES: &[(char, char)] = &[
    ('C', '\u{212d}'),
    ('H', '\u{210c}'),
    ('I', '\u{2111}'),
    ('R', '\u{211c}'),
    ('Z', '\u{2128}'),
];
const DOUBLE_STRUCK_HOLES: &[(char, char)] = &[
    ('C', '\u{2102}'),
    ('H', '\u{210d}'),
    ('N', '\u{2115}'),
    ('P', '\u{2119}'),
    ('Q', '\u{211a}'),
    ('R', '\u{211d}'),
    ('Z', '\u{2124}'),
];

/// First code points of the capital letters, small letters, digits and
/// Greek block for one variant (`0` where the variant has none).
struct Blocks {
    upper: u32,
    lower: u32,
    digit: u32,
    greek: u32,
    holes: &'static [(char, char)],
}

fn blocks(variant: MathVariant) -> Option<Blocks> {
    let b = |upper, lower, digit, greek, holes| {
        Some(Blocks {
            upper,
            lower,
            digit,
            greek,
            holes,
        })
    };
    match variant {
        MathVariant::Normal => None,
        MathVariant::Bold => b(0x1d400, 0x1d41a, 0x1d7ce, 0x1d6a8, &[]),
        MathVariant::Italic => b(0x1d434, 0x1d44e, 0, 0x1d6e2, ITALIC_HOLES),
        MathVariant::BoldItalic => b(0x1d468, 0x1d482, 0, 0x1d71c, &[]),
        MathVariant::Script => b(0x1d49c, 0x1d4b6, 0, 0, SCRIPT_HOLES),
        MathVariant::BoldScript => b(0x1d4d0, 0x1d4ea, 0, 0, &[]),
        MathVariant::Fraktur => b(0x1d504, 0x1d51e, 0, 0, FRAKTUR_HOLES),
        MathVariant::DoubleStruck => b(0x1d538, 0x1d552, 0x1d7d8, 0, DOUBLE_STRUCK_HOLES),
        MathVariant::BoldFraktur => b(0x1d56c, 0x1d586, 0, 0, &[]),
        MathVariant::SansSerif => b(0x1d5a0, 0x1d5ba, 0x1d7e2, 0, &[]),
        MathVariant::BoldSansSerif => b(0x1d5d4, 0x1d5ee, 0x1d7ec, 0x1d756, &[]),
        MathVariant::SansSerifItalic => b(0x1d608, 0x1d622, 0, 0, &[]),
        MathVariant::SansSerifBoldItalic => b(0x1d63c, 0x1d656, 0, 0x1d790, &[]),
        MathVariant::Monospace => b(0x1d670, 0x1d68a, 0x1d7f6, 0, &[]),
    }
}

pub(super) fn map(character: char, variant: MathVariant) -> char {
    let Some(blocks) = blocks(variant) else {
        return character;
    };
    if let Some(&(_, mapped)) = blocks.holes.iter().find(|(from, _)| *from == character) {
        return mapped;
    }
    let code = character as u32;
    let mapped = match character {
        'A'..='Z' => blocks.upper + (code - 'A' as u32),
        'a'..='z' => blocks.lower + (code - 'a' as u32),
        '0'..='9' if blocks.digit != 0 => blocks.digit + (code - '0' as u32),
        '\u{391}'..='\u{3a9}' if blocks.greek != 0 && code != 0x3a2 => {
            blocks.greek + (code - 0x391)
        }
        '\u{3b1}'..='\u{3c9}' if blocks.greek != 0 => blocks.greek + 26 + (code - 0x3b1),
        _ => return character,
    };
    char::from_u32(mapped).unwrap_or(character)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_with_the_block_exceptions() {
        assert_eq!(map('A', MathVariant::Bold), '\u{1d400}');
        assert_eq!(map('z', MathVariant::Monospace), '\u{1d6a3}');
        assert_eq!(map('h', MathVariant::Italic), '\u{210e}');
        assert_eq!(map('R', MathVariant::DoubleStruck), '\u{211d}');
        assert_eq!(map('7', MathVariant::DoubleStruck), '\u{1d7df}');
        assert_eq!(map('7', MathVariant::Italic), '7');
        assert_eq!(map('\u{3b1}', MathVariant::Bold), '\u{1d6c2}');
        assert_eq!(map('\u{3a9}', MathVariant::Italic), '\u{1d6fa}');
        assert_eq!(map('x', MathVariant::Normal), 'x');
        assert_eq!(map('+', MathVariant::Bold), '+');
    }
}
