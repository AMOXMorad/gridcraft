//! Minimal, zero-dependency TrueType (`.ttf`) parser for embedding Unicode / Arabic text in PDF.
//!
//! Reads the `head`, `hhea`, `hmtx` and `cmap` (Format 4 and Format 12) tables to map Unicode
//! scalar values (including Arabic Presentation Forms-A/B) to 16-bit glyph IDs and advance widths
//! in 1/1000 em units.

use std::sync::{Arc, OnceLock};

/// Parsed TrueType font metadata and raw bytes for `/FontFile2` embedding.
pub struct TtfFont {
    pub data: Vec<u8>,
    pub units_per_em: u16,
    pub bbox: [i32; 4],
    pub ascent: i32,
    pub descent: i32,
    num_h_metrics: u16,
    hmtx_offset: usize,
    hmtx_len: usize,
    cmap_subtable_offset: usize,
    cmap_format: u16,
}

fn u16_be(data: &[u8], off: usize) -> Option<u16> {
    let b = data.get(off..off.checked_add(2)?)?;
    Some(u16::from_be_bytes([b[0], b[1]]))
}

fn i16_be(data: &[u8], off: usize) -> Option<i16> {
    let b = data.get(off..off.checked_add(2)?)?;
    Some(i16::from_be_bytes([b[0], b[1]]))
}

fn u32_be(data: &[u8], off: usize) -> Option<u32> {
    let b = data.get(off..off.checked_add(4)?)?;
    Some(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

impl TtfFont {
    /// Parses a standalone TrueType (`.ttf`) buffer. Rejects OpenType/CFF (`OTTO`) or malformed data.
    pub fn parse(data: Vec<u8>) -> Option<Self> {
        let sfnt = u32_be(&data, 0)?;
        if sfnt != 0x0001_0000 && sfnt != 0x7472_7565 {
            return None;
        }
        let num_tables = usize::from(u16_be(&data, 4)?);
        let mut head: Option<(usize, usize)> = None;
        let mut hhea: Option<(usize, usize)> = None;
        let mut hmtx: Option<(usize, usize)> = None;
        let mut cmap: Option<(usize, usize)> = None;

        for i in 0..num_tables {
            let rec = 12usize.checked_add(i.checked_mul(16)?)?;
            let tag = data.get(rec..rec.checked_add(4)?)?;
            let off = u32_be(&data, rec + 8)? as usize;
            let len = u32_be(&data, rec + 12)? as usize;
            let _ = data.get(off..off.checked_add(len)?)?;
            match tag {
                b"head" => head = Some((off, len)),
                b"hhea" => hhea = Some((off, len)),
                b"hmtx" => hmtx = Some((off, len)),
                b"cmap" => cmap = Some((off, len)),
                _ => {}
            }
        }

        let (head_off, head_len) = head?;
        let (hhea_off, hhea_len) = hhea?;
        let (hmtx_off, hmtx_len) = hmtx?;
        let (cmap_off, _cmap_len) = cmap?;
        if head_len < 54 || hhea_len < 36 {
            return None;
        }

        let units_per_em = u16_be(&data, head_off + 18)?.max(16);
        let scale_1000 = |v: i16| -> i32 { (i32::from(v) * 1000) / i32::from(units_per_em) };
        let bbox = [
            scale_1000(i16_be(&data, head_off + 36)?),
            scale_1000(i16_be(&data, head_off + 38)?),
            scale_1000(i16_be(&data, head_off + 40)?),
            scale_1000(i16_be(&data, head_off + 42)?),
        ];
        let ascent = scale_1000(i16_be(&data, hhea_off + 4)?);
        let descent = scale_1000(i16_be(&data, hhea_off + 6)?);
        let num_h_metrics = u16_be(&data, hhea_off + 34)?.max(1);

        // Locate a Format 12 or Format 4 Unicode cmap subtable.
        let cmap_tables = usize::from(u16_be(&data, cmap_off + 2)?);
        let mut best_f12: Option<usize> = None;
        let mut best_f4: Option<usize> = None;
        for i in 0..cmap_tables {
            let rec = cmap_off.checked_add(4)?.checked_add(i.checked_mul(8)?)?;
            let platform = u16_be(&data, rec)?;
            let encoding = u16_be(&data, rec + 2)?;
            let sub_rel = u32_be(&data, rec + 4)? as usize;
            let sub_off = cmap_off.checked_add(sub_rel)?;
            let fmt = u16_be(&data, sub_off)?;
            let is_unicode = platform == 0 || (platform == 3 && (encoding == 1 || encoding == 10));
            if is_unicode {
                if fmt == 12 && best_f12.is_none() {
                    best_f12 = Some(sub_off);
                } else if fmt == 4 && best_f4.is_none() {
                    best_f4 = Some(sub_off);
                }
            }
        }

        let (cmap_subtable_offset, cmap_format) = if let Some(off) = best_f12 {
            (off, 12)
        } else if let Some(off) = best_f4 {
            (off, 4)
        } else {
            return None;
        };

        Some(Self {
            data,
            units_per_em,
            bbox,
            ascent,
            descent,
            num_h_metrics,
            hmtx_offset: hmtx_off,
            hmtx_len,
            cmap_subtable_offset,
            cmap_format,
        })
    }

    /// Maps a Unicode scalar value to its TrueType glyph ID (`0` if unmapped).
    pub fn glyph_id(&self, c: char) -> u16 {
        let code = c as u32;
        if self.cmap_format == 12 {
            self.glyph_id_f12(code).unwrap_or(0)
        } else {
            u16::try_from(code).ok().and_then(|cp| self.glyph_id_f4(cp)).unwrap_or(0)
        }
    }

    fn glyph_id_f12(&self, code: u32) -> Option<u16> {
        let base = self.cmap_subtable_offset;
        let n_groups = u32_be(&self.data, base.checked_add(12)?)? as usize;
        let groups_off = base.checked_add(16)?;
        let mut lo = 0usize;
        let mut hi = n_groups;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let g = groups_off.checked_add(mid.checked_mul(12)?)?;
            let start = u32_be(&self.data, g)?;
            let end = u32_be(&self.data, g + 4)?;
            if code < start {
                hi = mid;
            } else if code > end {
                lo = mid + 1;
            } else {
                let start_gid = u32_be(&self.data, g + 8)?;
                return Some((start_gid.wrapping_add(code - start)) as u16);
            }
        }
        None
    }

    fn glyph_id_f4(&self, code: u16) -> Option<u16> {
        let base = self.cmap_subtable_offset;
        let seg_count_x2 = usize::from(u16_be(&self.data, base.checked_add(6)?)?);
        let seg_count = seg_count_x2 / 2;
        let end_codes = base.checked_add(14)?;
        let start_codes = end_codes.checked_add(seg_count_x2)?.checked_add(2)?;
        let id_deltas = start_codes.checked_add(seg_count_x2)?;
        let id_range_offsets = id_deltas.checked_add(seg_count_x2)?;

        let mut lo = 0usize;
        let mut hi = seg_count;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let end = u16_be(&self.data, end_codes + mid * 2)?;
            if code > end {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo >= seg_count {
            return None;
        }
        let start = u16_be(&self.data, start_codes + lo * 2)?;
        let end = u16_be(&self.data, end_codes + lo * 2)?;
        if code < start || code > end {
            return None;
        }
        let delta = i32::from(i16_be(&self.data, id_deltas + lo * 2)?);
        let range_off_pos = id_range_offsets + lo * 2;
        let range_off = usize::from(u16_be(&self.data, range_off_pos)?);
        if range_off == 0 {
            Some(((i32::from(code) + delta) & 0xFFFF) as u16)
        } else {
            let idx_off = range_off_pos
                .checked_add(range_off)?
                .checked_add(usize::from(code - start) * 2)?;
            let gid = u16_be(&self.data, idx_off)?;
            if gid == 0 {
                Some(0)
            } else {
                Some(((i32::from(gid) + delta) & 0xFFFF) as u16)
            }
        }
    }

    /// Returns the advance width of `gid` in 1/1000 em units.
    pub fn glyph_width_1000(&self, gid: u16) -> u16 {
        let idx = usize::from(gid.min(self.num_h_metrics.saturating_sub(1)));
        let off = idx.saturating_mul(4);
        if off + 2 > self.hmtx_len {
            return 600;
        }
        let Some(adv) = u16_be(&self.data, self.hmtx_offset.saturating_add(off)) else {
            return 600;
        };
        let upm = u32::from(self.units_per_em.max(1));
        (((u32::from(adv) * 1000) + upm / 2) / upm).min(10_000) as u16
    }
}

/// Returns the cached system Unicode TrueType font (capable of rendering Arabic and other non-Latin
/// scripts), or `None` if no suitable `.ttf` is installed on the host.
pub fn system_unicode_font() -> Option<&'static Arc<TtfFont>> {
    static FONT: OnceLock<Option<Arc<TtfFont>>> = OnceLock::new();
    FONT.get_or_init(load_system_unicode_font).as_ref()
}

#[cfg(not(target_arch = "wasm32"))]
fn load_system_unicode_font() -> Option<Arc<TtfFont>> {
    let win = std::env::var("WINDIR")
        .map(|w| format!("{w}\\Fonts"))
        .unwrap_or_else(|_| "C:\\Windows\\Fonts".into());
    let candidates = [
        format!("{win}\\arial.ttf"),
        format!("{win}\\tahoma.ttf"),
        format!("{win}\\segoeui.ttf"),
        format!("{win}\\times.ttf"),
        "/System/Library/Fonts/Supplemental/Arial Unicode.ttf".into(),
        "/System/Library/Fonts/Supplemental/Arial.ttf".into(),
        "/System/Library/Fonts/Supplemental/Tahoma.ttf".into(),
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf".into(),
        "/usr/share/fonts/dejavu/DejaVuSans.ttf".into(),
        "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf".into(),
        "/usr/share/fonts/truetype/noto/NotoSansArabic-Regular.ttf".into(),
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf".into(),
    ];
    for path in candidates {
        let Ok(bytes) = std::fs::read(&path) else { continue };
        // Cap font size to 32 MB for safety.
        if bytes.len() > 32 * 1024 * 1024 {
            continue;
        }
        if let Some(ttf) = TtfFont::parse(bytes)
            && ttf.glyph_id('\u{0627}') != 0
            && ttf.glyph_id('\u{FE8E}') != 0
        {
            return Some(Arc::new(ttf));
        }
    }
    None
}

#[cfg(target_arch = "wasm32")]
fn load_system_unicode_font() -> Option<Arc<TtfFont>> {
    None
}
