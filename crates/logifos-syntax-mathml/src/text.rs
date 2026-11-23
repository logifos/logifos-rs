use crate::{ParseError, ParseErrorKind};

pub(crate) fn xml_char(ch: char) -> bool {
    matches!(ch, '\u{9}' | '\u{a}' | '\u{d}' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}')
}

pub(crate) fn normalize_lines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

// XML 1.0 sections 2.11, 3.3.3 and 4.1. Literal attribute whitespace is
// normalized; whitespace produced by a character reference is preserved.
pub(crate) fn decode(text: &str, offset: usize, attribute: bool) -> Result<String, ParseError> {
    let mut value = String::with_capacity(text.len());
    let mut chars = text.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if ch == '&' {
            let start = index + 1;
            let mut end = None;
            for (index, ch) in chars.by_ref() {
                if ch == ';' {
                    end = Some(index);
                    break;
                }
            }
            let end = end.ok_or_else(|| {
                ParseError::new(
                    ParseErrorKind::MalformedXml,
                    offset + index,
                    "unterminated reference",
                )
            })?;
            let reference = &text[start..end];
            let decoded = match reference {
                "amp" => '&',
                "lt" => '<',
                "gt" => '>',
                "apos" => '\'',
                "quot" => '"',
                _ if reference.starts_with('#') => {
                    let (digits, radix) = match reference.strip_prefix("#x") {
                        Some(digits) => (digits, 16),
                        None => (&reference[1..], 10),
                    };
                    let valid_digits = !digits.is_empty()
                        && digits.bytes().all(|b| match radix {
                            16 => b.is_ascii_hexdigit(),
                            _ => b.is_ascii_digit(),
                        });
                    let ch = valid_digits
                        .then(|| u32::from_str_radix(digits, radix).ok())
                        .flatten()
                        .and_then(char::from_u32)
                        .filter(|ch| xml_char(*ch));
                    ch.ok_or_else(|| {
                        ParseError::new(
                            ParseErrorKind::MalformedXml,
                            offset + index,
                            "invalid character reference",
                        )
                    })?
                }
                _ => {
                    return Err(ParseError::new(
                        ParseErrorKind::UnsafeXml,
                        offset + index,
                        "custom entities are prohibited",
                    ))
                }
            };
            value.push(decoded);
        } else {
            let ch = if ch == '\r' {
                if chars.peek().is_some_and(|(_, next)| *next == '\n') {
                    chars.next();
                }
                '\n'
            } else {
                ch
            };
            value.push(if attribute && matches!(ch, '\n' | '\t') {
                ' '
            } else {
                ch
            });
        }
    }
    Ok(value)
}
