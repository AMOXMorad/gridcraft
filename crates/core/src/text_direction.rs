//! Text direction and BiDi support types for cells and ranges.

use serde::{Deserialize, Serialize};

/// Text direction for a cell or range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum TextDirection {
    /// Automatic direction detection based on content (default).
    #[default]
    Auto,
    /// Left-to-right.
    Ltr,
    /// Right-to-left (Arabic, Hebrew, Farsi, Urdu).
    Rtl,
}

impl TextDirection {
    /// Returns `true` if the direction is explicitly RTL or auto-detected as RTL.
    pub fn is_rtl(self) -> bool {
        matches!(self, Self::Rtl)
    }
}

/// Detects the dominant text direction of a string by examining its characters.
/// Uses the Unicode Bidirectional Algorithm (simplified).
pub fn detect_direction(text: &str) -> TextDirection {
    let mut rtl_count = 0u32;
    let mut ltr_count = 0u32;
    for ch in text.chars() {
        match bidi_class(ch) {
            BidiClass::R | BidiClass::Al => rtl_count = rtl_count.saturating_add(1),
            BidiClass::L => ltr_count = ltr_count.saturating_add(1),
            _ => {}
        }
    }
    if rtl_count == 0 && ltr_count == 0 {
        TextDirection::Auto
    } else if rtl_count > ltr_count {
        TextDirection::Rtl
    } else {
        TextDirection::Ltr
    }
}

/// Simplified Unicode Bidirectional character classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BidiClass {
    /// Strong left-to-right (Latin, CJK, etc.)
    L,
    /// Strong right-to-left (Hebrew)
    R,
    /// Arabic letter (strong RTL)
    Al,
    /// Everything else (numbers, punctuation, whitespace)
    Other,
}

/// Classify a character's bidirectional type using Unicode ranges.
fn bidi_class(ch: char) -> BidiClass {
    let cp = ch as u32;
    match cp {
        // Arabic block
        0x0600..=0x06FF | 0x0750..=0x077F | 0x08A0..=0x08FF | 0xFB50..=0xFDFF | 0xFE70..=0xFEFF => BidiClass::Al,
        // Hebrew block
        0x0590..=0x05FF | 0xFB1D..=0xFB4F => BidiClass::R,
        // Syriac, Thaana, NKo, Samaritan, Mandaic
        0x0700..=0x074F | 0x0780..=0x07BF | 0x07C0..=0x07FF | 0x0800..=0x083F | 0x0840..=0x085F => BidiClass::R,
        // Basic Latin letters
        0x0041..=0x005A | 0x0061..=0x007A => BidiClass::L,
        // Latin Extended, Greek, Cyrillic, CJK
        0x00C0..=0x024F | 0x0370..=0x03FF | 0x0400..=0x04FF | 0x4E00..=0x9FFF | 0x3040..=0x30FF => BidiClass::L,
        _ => BidiClass::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_direction_arabic() {
        assert_eq!(detect_direction("مرحبا"), TextDirection::Rtl);
        assert_eq!(detect_direction("بسم الله"), TextDirection::Rtl);
    }

    #[test]
    fn detect_direction_hebrew() {
        assert_eq!(detect_direction("שלום"), TextDirection::Rtl);
        assert_eq!(detect_direction("שלום עולם"), TextDirection::Rtl);
    }

    #[test]
    fn detect_direction_english() {
        assert_eq!(detect_direction("Hello world"), TextDirection::Ltr);
        assert_eq!(detect_direction("GridCraft Spreadsheet"), TextDirection::Ltr);
    }

    #[test]
    fn detect_direction_mixed() {
        // More Arabic chars than English
        assert_eq!(detect_direction("مرحبا بكم hello"), TextDirection::Rtl);
        // More English chars than Arabic
        assert_eq!(detect_direction("Hello world مرحبا"), TextDirection::Ltr);
    }

    #[test]
    fn detect_direction_empty_and_neutral() {
        assert_eq!(detect_direction(""), TextDirection::Auto);
        assert_eq!(detect_direction("12345"), TextDirection::Auto);
        assert_eq!(detect_direction("   \t\n"), TextDirection::Auto);
        assert_eq!(detect_direction("!@#$%^&*()_+-=[]{}|;':\",./<>?"), TextDirection::Auto);
    }

    #[test]
    fn text_direction_is_rtl() {
        assert!(!TextDirection::Auto.is_rtl());
        assert!(!TextDirection::Ltr.is_rtl());
        assert!(TextDirection::Rtl.is_rtl());
    }

    #[test]
    fn text_direction_serde() {
        let auto = TextDirection::Auto;
        let ltr = TextDirection::Ltr;
        let rtl = TextDirection::Rtl;

        let auto_json = serde_json::to_string(&auto).expect("serialize auto");
        let ltr_json = serde_json::to_string(&ltr).expect("serialize ltr");
        let rtl_json = serde_json::to_string(&rtl).expect("serialize rtl");

        assert_eq!(serde_json::from_str::<TextDirection>(&auto_json).expect("deserialize auto"), auto);
        assert_eq!(serde_json::from_str::<TextDirection>(&ltr_json).expect("deserialize ltr"), ltr);
        assert_eq!(serde_json::from_str::<TextDirection>(&rtl_json).expect("deserialize rtl"), rtl);
    }
}
