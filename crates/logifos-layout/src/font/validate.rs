//! Strict checks before Fontisan's intentionally tolerant table readers run.
use super::error::{FontError, FontErrorKind, Result};
use super::reader::{Budget, Reader};
use std::collections::BTreeSet;

pub(super) fn directory(bytes: &[u8], face: u16) -> Result<()> {
    let r = Reader::new(bytes, "sfnt");
    let offset = if r.range(0, 4, 1)? == b"ttcf" {
        let version = r.u32(4)?;
        if version != 0x0001_0000 && version != 0x0002_0000 {
            return Err(r.err(4));
        }
        let count = usize::try_from(r.u32(8)?).map_err(|_| r.err(8))?;
        r.range(12, count, 4)?;
        if usize::from(face) >= count {
            return Err(FontError::new(FontErrorKind::InvalidFont));
        }
        let end = 12 + count * 4;
        if version == 0x0002_0000 {
            r.range(end, 3, 4)?;
            let length = r.u32(end + 4)? as usize;
            let offset = r.u32(end + 8)? as usize;
            if length != 0 {
                r.range(offset, length, 1)?;
            }
        }
        for i in 0..count {
            let f = r.sub(r.u32(12 + i * 4)? as usize, end)?;
            signature(f)?;
        }
        r.u32(12 + usize::from(face) * 4)? as usize
    } else {
        if face != 0 {
            return Err(FontError::new(FontErrorKind::InvalidFont));
        }
        0
    };
    let dir = if offset == 0 { r } else { r.sub(offset, 12)? };
    signature(dir)?;
    let n = usize::from(dir.u16(4)?);
    dir.range(12, n, 16)?;
    let mut tags = BTreeSet::new();
    for i in 0..n {
        let at = 12 + i * 16;
        if !tags.insert(dir.range(at, 4, 1)?) {
            return Err(dir.err(at));
        }
        let start = dir.u32(at + 8)? as usize;
        let len = dir.u32(at + 12)? as usize;
        if start < 12 {
            return Err(dir.err(at + 8));
        }
        r.range(start, len, 1)?;
    }
    Ok(())
}

fn signature(r: Reader<'_>) -> Result<()> {
    match r.range(0, 4, 1)? {
        [0, 1, 0, 0] | b"OTTO" => Ok(()),
        _ => Err(FontError::new(FontErrorKind::UnsupportedFormat)),
    }
}

pub(super) fn loca(loca: &[u8], glyf: &[u8], format: i16, count: u16) -> Result<()> {
    let r = Reader::new(loca, "loca");
    let width = match format {
        0 => 2,
        1 => 4,
        _ => return Err(r.err(0)),
    };
    r.range(0, usize::from(count) + 1, width)?;
    let mut previous = 0;
    for i in 0..=usize::from(count) {
        let offset = if width == 2 {
            usize::from(r.u16(i * 2)?) * 2
        } else {
            r.u32(i * 4)? as usize
        };
        if offset > glyf.len() || offset < previous || (i == 0 && offset != 0) {
            return Err(r.err(i * width));
        }
        if i != 0 && offset != previous && offset - previous < 10 {
            return Err(r.err(i * width));
        }
        previous = offset;
    }
    Ok(())
}

pub(super) fn cmap(bytes: &[u8], glyphs: u16) -> Result<()> {
    let r = Reader::new(bytes, "cmap");
    if r.u16(0)? != 0 {
        return Err(r.err(0));
    }
    let count = usize::from(r.u16(2)?);
    let end = 4 + count * 8;
    r.range(4, count, 8)?;
    let mut selected = None;
    let mut seen = BTreeSet::new();
    let mut budget = Budget::new();
    for i in 0..count {
        let at = 4 + i * 8;
        let platform = r.u16(at)?;
        let encoding = r.u16(at + 2)?;
        let offset = r.u32(at + 4)? as usize;
        let s = r.sub(offset, end)?;
        let format = s.u16(0)?;
        // Nonselected encodings are not decoded. Their declared ranges must fit.
        let len = match format {
            0 | 2 | 4 | 6 => usize::from(s.u16(2)?),
            8 | 10 | 12 | 13 => s.u32(4)? as usize,
            14 => s.u32(2)? as usize,
            _ => return Err(s.err(0)),
        };
        if len < 6 {
            return Err(s.err(2));
        }
        let s = Reader::new(s.range(0, len, 1)?, "cmap");
        if seen.insert(offset) {
            match format {
                4 => format4(s, glyphs, &mut budget)?,
                12 => format12(s, glyphs, &mut budget)?,
                _ => (),
            }
        }
        let rank = match (platform, encoding) {
            (3, 10) => 0,
            (3, 1) => 1,
            (0, _) => 2,
            _ => 3,
        };
        if rank < 3 && selected.is_none_or(|(previous, _)| rank < previous) {
            selected = Some((rank, format));
        }
    }
    if !matches!(selected, Some((_, 4 | 12))) {
        return Err(FontError::new(FontErrorKind::UnsupportedFormat));
    }
    Ok(())
}

fn format12(r: Reader<'_>, glyphs: u16, budget: &mut Budget) -> Result<()> {
    let n = r.u32(12)? as usize;
    r.range(16, n, 12)?;
    budget.spend(n)?;
    let mut previous = None;
    for i in 0..n {
        let at = 16 + i * 12;
        let start = r.u32(at)?;
        let end = r.u32(at + 4)?;
        let glyph = r.u32(at + 8)?;
        if start > end
            || end > 0x10_ffff
            || previous.is_some_and(|p| start <= p)
            || u64::from(glyph) + u64::from(end - start) >= u64::from(glyphs)
        {
            return Err(r.err(at));
        }
        previous = Some(end);
    }
    Ok(())
}

fn format4(r: Reader<'_>, glyphs: u16, budget: &mut Budget) -> Result<()> {
    let n2 = usize::from(r.u16(6)?);
    if n2 == 0 || n2 % 2 != 0 {
        return Err(r.err(6));
    }
    let n = n2 / 2;
    r.range(14, 2 + n * 8, 1)?;
    let glyph_array = 16 + n * 8;
    let range_array = 16 + n * 6;
    let mut previous = None;
    for i in 0..n {
        let start = r.u16(16 + n * 2 + i * 2)?;
        let end = r.u16(14 + i * 2)?;
        let delta = r.u16(16 + n * 4 + i * 2)?;
        let offset = usize::from(r.u16(range_array + i * 2)?);
        if start > end || previous.is_some_and(|p| start <= p) || offset % 2 != 0 {
            return Err(r.err(14 + i * 2));
        }
        budget.spend(usize::from(end - start) + 1)?;
        for code in start..=end {
            let id = if offset == 0 {
                code.wrapping_add(delta)
            } else {
                let at = range_array + i * 2 + offset + usize::from(code - start) * 2;
                if at < glyph_array {
                    return Err(r.err(at));
                }
                match r.u16(at)? {
                    0 => 0,
                    id => id.wrapping_add(delta),
                }
            };
            if id >= glyphs {
                return Err(r.err(range_array + i * 2));
            }
        }
        previous = Some(end);
    }
    if previous != Some(0xffff) {
        return Err(r.err(14));
    }
    Ok(())
}
