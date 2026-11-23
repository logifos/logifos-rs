use super::error::{FontError, FontErrorKind, Result};

#[derive(Clone, Copy)]
pub(super) struct Reader<'a> {
    pub bytes: &'a [u8],
    pub tag: &'static str,
    origin: usize,
}

impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8], tag: &'static str) -> Self {
        Self {
            bytes,
            tag,
            origin: 0,
        }
    }

    pub fn err(self, at: usize) -> FontError {
        FontError::malformed(self.tag, self.origin.saturating_add(at))
    }

    pub fn range(self, at: usize, count: usize, width: usize) -> Result<&'a [u8]> {
        let end = count
            .checked_mul(width)
            .and_then(|n| at.checked_add(n))
            .ok_or_else(|| self.err(at))?;
        self.bytes.get(at..end).ok_or_else(|| self.err(at))
    }

    pub fn u16(self, at: usize) -> Result<u16> {
        let b = self.range(at, 1, 2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }

    pub fn i16(self, at: usize) -> Result<i16> {
        Ok(self.u16(at)? as i16)
    }

    pub fn u32(self, at: usize) -> Result<u32> {
        let b = self.range(at, 1, 4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn sub(self, offset: usize, minimum: usize) -> Result<Self> {
        if offset < minimum {
            return Err(self.err(offset));
        }
        let bytes = self.bytes.get(offset..).ok_or_else(|| self.err(offset))?;
        if bytes.is_empty() {
            return Err(self.err(offset));
        }
        Ok(Self {
            bytes,
            tag: self.tag,
            origin: self.origin + offset,
        })
    }

    pub fn offset16(self, at: usize, minimum: usize) -> Result<Self> {
        self.sub(usize::from(self.u16(at)?), minimum)
    }

    pub fn optional16(self, at: usize, minimum: usize) -> Result<Option<Self>> {
        match self.u16(at)? {
            0 => Ok(None),
            n => self.sub(usize::from(n), minimum).map(Some),
        }
    }
}

/// Bounds work and allocation even when many offsets alias the same large table.
pub(super) struct Budget(usize);
impl Budget {
    pub fn new() -> Self {
        Self(262_144)
    }
    pub fn spend(&mut self, count: usize) -> Result<()> {
        self.0 = self
            .0
            .checked_sub(count)
            .ok_or_else(|| FontError::new(FontErrorKind::ResourceLimit))?;
        Ok(())
    }
}
