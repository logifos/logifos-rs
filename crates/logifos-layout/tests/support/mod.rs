use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub fn put16(b: &mut [u8], at: usize, n: u16) {
    b[at..at + 2].copy_from_slice(&n.to_be_bytes());
}
pub fn put32(b: &mut [u8], at: usize, n: u32) {
    b[at..at + 4].copy_from_slice(&n.to_be_bytes());
}
pub fn get16(b: &[u8], at: usize) -> usize {
    usize::from(u16::from_be_bytes([b[at], b[at + 1]]))
}
pub fn get32(b: &[u8], at: usize) -> usize {
    u32::from_be_bytes(b[at..at + 4].try_into().unwrap()) as usize
}
pub fn words(values: &[u16]) -> Vec<u8> {
    values.iter().flat_map(|n| n.to_be_bytes()).collect()
}
fn link(parent: &mut Vec<u8>, field: usize, child: &[u8]) {
    let at = parent.len();
    put16(parent, field, at as u16);
    parent.extend_from_slice(child);
}
pub fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn value_table(value: i16, ranges: bool) -> Vec<u8> {
    let mut b = words(&[8, 1, value as u16, 0]);
    b.extend(words(if ranges { &[2, 1, 1, 1, 0] } else { &[1, 1, 1] }));
    b
}

/// A version-2 OS/2 table whose only nonzero metric is sxHeight: 96 bytes —
/// 78 header bytes, the v1 code-page range, then the five v2 fields
/// (sxHeight, sCapHeight, usDefaultChar, usBreakChar, usMaxContext).
fn os2_table(x_height: i16) -> Vec<u8> {
    let mut b = vec![0; 96];
    put16(&mut b, 0, 2); // version
    put16(&mut b, 86, x_height as u16); // sxHeight
    put16(&mut b, 88, 500); // sCapHeight
    b
}

/// Independently chosen values in each specification field, in design units.
pub fn math_table(axis: i16, stretch: bool) -> Vec<u8> {
    let mut c = words(&[80, 60, 900, 1100]);
    for i in 0..51 {
        c.extend(words(&[(100 + i) as u16, 0]));
    }
    put16(&mut c, 12, axis as u16);
    put16(&mut c, 8 + 50 * 4, (-120i16) as u16);
    c.extend(words(&[65]));
    let mut info = vec![0; 8];
    link(&mut info, 0, &value_table(25, false));
    link(&mut info, 2, &value_table(340, true));
    link(&mut info, 4, &words(&[1, 1, 2]));
    let mut kern = words(&[12, 1, 18, 0, 0, 0]);
    kern.extend(words(&[1, 1, 1]));
    kern.extend(words(&[1, 300, 0, (-20i16) as u16, 0, 10, 0]));
    link(&mut info, 6, &kern);
    let mut variants = words(&[40, 0, 0, 0, 0]);
    if stretch {
        variants = words(&[40, 0, 0, 1, 1, 0, 0]);
        link(&mut variants, 2, &words(&[1, 1, 2]));
        link(&mut variants, 4, &words(&[2, 1, 1, 1, 0]));
        let mut construction = words(&[12, 2, 2, 800, 3, 1200]);
        construction.extend(words(&[
            35, 0, 3, 3, 0, 60, 500, 0, 4, 60, 60, 600, 1, 5, 60, 0, 500, 0,
        ]));
        link(&mut variants, 10, &construction);
        link(&mut variants, 12, &words(&[0, 1, 5, 1600]));
    }
    let mut b = words(&[1, 0, 0, 0, 0]);
    link(&mut b, 4, &c);
    link(&mut b, 6, &info);
    link(&mut b, 8, &variants);
    b
}

pub struct FontBuilder {
    pub tables: BTreeMap<[u8; 4], Vec<u8>>,
}
impl FontBuilder {
    pub fn new(characters: &[(char, u16)], axis: i16, stretch: bool) -> Self {
        let mut head = vec![0; 54];
        put32(&mut head, 0, 0x0001_0000);
        put32(&mut head, 12, 0x5f0f_3cf5);
        put16(&mut head, 18, 1000);
        put16(&mut head, 50, 1);
        let mut hhea = vec![0; 36];
        put32(&mut hhea, 0, 0x0001_0000);
        put16(&mut hhea, 4, 800);
        put16(&mut hhea, 6, (-200i16) as u16);
        put16(&mut hhea, 8, 50);
        put16(&mut hhea, 10, 600);
        put16(&mut hhea, 34, 2);
        let mut maxp = vec![0; 32];
        put32(&mut maxp, 0, 0x0001_0000);
        put16(&mut maxp, 4, 6);
        put16(&mut maxp, 6, 3);
        put16(&mut maxp, 8, 1);
        put16(&mut maxp, 14, 1);
        let hmtx = words(&[500, 0, 600, (-10i16) as u16, 5, 10, 15, 20]);
        let mut loca = vec![0; 28];
        let mut glyf = Vec::new();
        for i in 1..6 {
            put32(&mut loca, i * 4, glyf.len() as u32);
            let mut triangle = words(&[1, 0, (-200i16) as u16, 500, 700, 2, 0]);
            triangle.extend([1, 1, 1]);
            triangle.extend(words(&[
                0,
                500,
                (-500i16) as u16,
                (-200i16) as u16,
                900,
                (-900i16) as u16,
            ]));
            triangle.push(0);
            glyf.extend(triangle);
        }
        put32(&mut loca, 24, glyf.len() as u32);
        let mut cmap = words(&[0, 1, 3, 10]);
        cmap.extend(12u32.to_be_bytes());
        cmap.extend(words(&[12, 0]));
        cmap.extend((16 + characters.len() as u32 * 12).to_be_bytes());
        cmap.extend(0u32.to_be_bytes());
        cmap.extend((characters.len() as u32).to_be_bytes());
        let mut chars = characters.to_vec();
        chars.sort();
        for (c, id) in chars {
            cmap.extend((c as u32).to_be_bytes());
            cmap.extend((c as u32).to_be_bytes());
            cmap.extend(u32::from(id).to_be_bytes());
        }
        Self {
            tables: BTreeMap::from([
                (*b"head", head),
                (*b"hhea", hhea),
                (*b"maxp", maxp),
                (*b"hmtx", hmtx),
                (*b"cmap", cmap),
                (*b"glyf", glyf),
                (*b"loca", loca),
                (*b"MATH", math_table(axis, stretch)),
                (*b"OS/2", os2_table(450)),
            ]),
        }
    }
    pub fn finish(&self) -> Vec<u8> {
        let count = self.tables.len();
        let power = 1usize << count.ilog2();
        let mut b = vec![0; 12 + count * 16];
        put32(&mut b, 0, 0x0001_0000);
        put16(&mut b, 4, count as u16);
        put16(&mut b, 6, (power * 16) as u16);
        put16(&mut b, 8, power.ilog2() as u16);
        put16(&mut b, 10, ((count - power) * 16) as u16);
        let mut head_at = None;
        for (i, (tag, table)) in self.tables.iter().enumerate() {
            let start = b.len();
            let at = 12 + i * 16;
            b[at..at + 4].copy_from_slice(tag);
            put32(&mut b, at + 4, checksum(table));
            put32(&mut b, at + 8, start as u32);
            put32(&mut b, at + 12, table.len() as u32);
            if tag == b"head" {
                head_at = Some(start);
            }
            b.extend(table);
            b.resize(b.len().next_multiple_of(4), 0);
        }
        if let Some(at) = head_at {
            let adjustment = 0xb1b0_afbau32.wrapping_sub(checksum(&b));
            put32(&mut b, at + 8, adjustment);
        }
        b
    }
}
fn checksum(bytes: &[u8]) -> u32 {
    bytes.chunks(4).fold(0u32, |sum, chunk| {
        let mut b = [0; 4];
        b[..chunk.len()].copy_from_slice(chunk);
        sum.wrapping_add(u32::from_be_bytes(b))
    })
}

pub fn font() -> Vec<u8> {
    FontBuilder::new(&[('(', 2), ('x', 1), ('𝑥', 4)], 250, true).finish()
}

/// Two independent faces; all sfnt table offsets are relative to the full file.
pub fn collection() -> Vec<u8> {
    let mut a = font();
    let mut b = FontBuilder::new(&[('y', 1)], 375, true).finish();
    let first = 20;
    let second = first + a.len();
    for (bytes, shift) in [(&mut a, first), (&mut b, second)] {
        for i in 0..get16(bytes, 4) {
            let at = 12 + i * 16 + 8;
            let offset = get32(bytes, at) + shift;
            put32(bytes, at, offset as u32);
        }
    }
    let mut bytes = b"ttcf".to_vec();
    bytes.extend(0x0001_0000u32.to_be_bytes());
    bytes.extend(2u32.to_be_bytes());
    bytes.extend((first as u32).to_be_bytes());
    bytes.extend((second as u32).to_be_bytes());
    bytes.extend(a);
    bytes.extend(b);
    bytes
}
