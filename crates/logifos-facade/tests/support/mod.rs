use std::collections::BTreeMap;

pub fn put16(b: &mut [u8], at: usize, n: u16) {
    b[at..at + 2].copy_from_slice(&n.to_be_bytes());
}
pub fn put32(b: &mut [u8], at: usize, n: u32) {
    b[at..at + 4].copy_from_slice(&n.to_be_bytes());
}
pub fn words(values: &[u16]) -> Vec<u8> {
    values.iter().flat_map(|n| n.to_be_bytes()).collect()
}
fn link(parent: &mut Vec<u8>, field: usize, child: &[u8]) {
    let at = parent.len();
    put16(parent, field, at as u16);
    parent.extend_from_slice(child);
}

fn value_table(value: i16, ranges: bool) -> Vec<u8> {
    let mut b = words(&[8, 1, value as u16, 0]);
    b.extend(words(if ranges { &[2, 1, 1, 1, 0] } else { &[1, 1, 1] }));
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
        Self {
            tables: BTreeMap::from([
                (*b"head", head),
                (*b"hhea", hhea),
                (*b"maxp", maxp),
                (*b"hmtx", hmtx),
                (*b"cmap", cmap_table(characters)),
                (*b"glyf", glyf),
                (*b"loca", loca),
                (*b"MATH", math_table(axis, stretch)),
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

fn cmap_table(characters: &[(char, u16)]) -> Vec<u8> {
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
    cmap
}

/// Type 1 cipher encryption of a charstring (R = 4330), with a `len_iv`
/// byte nonce prepended.
fn encrypt(plain: &[u8], len_iv: usize) -> Vec<u8> {
    let mut data: Vec<u8> = (0..len_iv).map(|i| i as u8 ^ 0xa5).collect();
    data.extend_from_slice(plain);
    let mut r: u16 = 4330;
    let mut out = Vec::with_capacity(data.len());
    for &p in &data {
        let c = p ^ (r >> 8) as u8;
        out.push(c);
        r = (u16::from(c).wrapping_add(r))
            .wrapping_mul(52845)
            .wrapping_add(22719);
    }
    out
}

/// An OTF-flavored face: 'x' is a real lenIV-encrypted Type 2 charstring
/// whose rmoveto, rrcurveto and hlineto make one cubic segment and one line
/// (font-unit points (0,0) (125,0) (125,125) (125,125) (250,125)); the
/// coordinates are 125-multiples so 24px/1000upem lands on exact pixels.
pub fn cff_font() -> Vec<u8> {
    let mut head = vec![0; 54];
    put32(&mut head, 0, 0x0001_0000);
    put32(&mut head, 12, 0x5f0f_3cf5);
    put16(&mut head, 18, 1000);
    put16(&mut head, 50, 1);
    let mut hhea = vec![0; 36];
    put32(&mut hhea, 0, 0x0001_0000);
    put16(&mut hhea, 4, 800);
    put16(&mut hhea, 6, (-200i16) as u16);
    put16(&mut hhea, 10, 600);
    put16(&mut hhea, 34, 2);
    let maxp = vec![0, 0, 0x50, 0, 0, 6];
    let hmtx = words(&[500, 0, 500, 0, 5, 10, 15, 20]);
    let glyph_x = [
        139, 139, 21, // rmoveto 0 0
        247, 17, 139, 247, 17, 247, 17, 139, 247, 17, 8, // rrcurveto 125 0 125 125 0 125
        247, 142, 6,  // hlineto 250
        14, // endchar
    ];
    fn index(items: &[&[u8]]) -> Vec<u8> {
        let mut b = (items.len() as u16).to_be_bytes().to_vec();
        if items.is_empty() {
            return b;
        }
        b.push(1);
        let mut off = 1u8;
        for item in items {
            b.push(off);
            off += item.len() as u8;
        }
        b.push(off);
        for item in items {
            b.extend_from_slice(item);
        }
        b
    }
    fn operand(v: usize) -> Vec<u8> {
        debug_assert!((-107..=107).contains(&(v as i64)));
        vec![v as u8 + 139]
    }
    let names = index(&[b"LogifosCFF"]);
    let strings = index(&[]);
    let gsubrs = index(&[]);
    let blank: &[u8] = &[14];
    let charstrings = index(&[blank, &encrypt(&glyph_x, 4), blank, blank, blank, blank]);
    // The Private DICT names its lenIV (4) explicitly; the Top DICT's
    // operands all fit the 1-byte encoding, so sizes are stable.
    let private = [139 + 4, 20];
    let topdict_at = 4 + names.len();
    let charstrings_at = topdict_at + 10 + strings.len() + gsubrs.len();
    let private_at = charstrings_at + charstrings.len();
    let mut dict = operand(charstrings_at);
    dict.push(17);
    dict.extend(operand(private.len()));
    dict.extend(operand(private_at));
    dict.push(18);
    debug_assert_eq!(index(&[&dict]).len(), 10);
    let topdict = index(&[&dict]);
    let mut cff = vec![1u8, 0, 4, 1];
    cff.extend_from_slice(&names);
    cff.extend_from_slice(&topdict);
    cff.extend_from_slice(&strings);
    cff.extend_from_slice(&gsubrs);
    cff.extend_from_slice(&charstrings);
    cff.extend_from_slice(&private);
    FontBuilder {
        tables: BTreeMap::from([
            (*b"head", head),
            (*b"hhea", hhea),
            (*b"maxp", maxp),
            (*b"hmtx", hmtx),
            (*b"cmap", cmap_table(&[('x', 1)])),
            (*b"CFF ", cff),
            (*b"MATH", math_table(250, false)),
        ]),
    }
    .finish()
}
