//! Text direction, Arabic contextual shaping, and BiDi visual reordering for cells and UI.

use std::borrow::Cow;

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
    /// Returns `true` if the direction is explicitly RTL.
    pub fn is_rtl(self) -> bool {
        matches!(self, Self::Rtl)
    }
}

/// Detects the dominant text direction of a string by examining its characters.
/// Uses the Unicode Bidirectional Algorithm (first/dominant strong character rule).
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

/// Returns `true` if `text` contains any right-to-left character (Arabic, Hebrew, etc.).
pub fn contains_rtl(text: &str) -> bool {
    text.chars().any(|ch| matches!(bidi_class(ch), BidiClass::R | BidiClass::Al))
}

/// Simplified Unicode Bidirectional character classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BidiClass {
    /// Strong left-to-right (Latin, CJK, etc.)
    L,
    /// Strong right-to-left (Hebrew, Syriac, etc.)
    R,
    /// Arabic letter (strong RTL)
    Al,
    /// European / Arabic-Indic digits (weak LTR inside RTL)
    En,
    /// Everything else (punctuation, whitespace)
    Other,
}

/// Classify a character's bidirectional type using Unicode ranges.
fn bidi_class(ch: char) -> BidiClass {
    let cp = ch as u32;
    match cp {
        // Arabic blocks
        0x0600..=0x065F | 0x066A..=0x06EF | 0x06FA..=0x06FF | 0x0750..=0x077F | 0x08A0..=0x08FF | 0xFB50..=0xFDFF | 0xFE70..=0xFEFF => BidiClass::Al,
        // ASCII digits and Arabic-Indic / Extended Arabic-Indic digits
        0x0030..=0x0039 | 0x0660..=0x0669 | 0x06F0..=0x06F9 => BidiClass::En,
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

/// Joining behavior of an Arabic character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Joining {
    /// Does not join with neighbors.
    None,
    /// Joins only with the preceding character (on the right): ا د ذ ر ز و ة ء ...
    Right,
    /// Joins on both sides: ب ت ث ج ح خ س ش ص ض ط ظ ع غ ف ق ك ل م ن ه ي ...
    Dual,
    /// Diacritic / haraka: transparent to joining.
    Transparent,
}

/// Contextual glyph forms in Unicode Arabic Presentation Forms-A/B:
/// `(isolated, final, initial, medial)`, plus `Joining`.
struct ArabicForms {
    isolated: char,
    final_form: char,
    initial: char,
    medial: char,
    joining: Joining,
}

const fn right_forms(isolated: char, final_form: char) -> ArabicForms {
    ArabicForms { isolated, final_form, initial: isolated, medial: final_form, joining: Joining::Right }
}

const fn dual_forms(isolated: char, final_form: char, initial: char, medial: char) -> ArabicForms {
    ArabicForms { isolated, final_form, initial, medial, joining: Joining::Dual }
}

/// Looks up the presentation forms and joining class for an Arabic character.
fn arabic_forms(ch: char) -> Option<ArabicForms> {
    Some(match ch {
        '\u{0621}' => ArabicForms { isolated: '\u{FE80}', final_form: '\u{FE80}', initial: '\u{FE80}', medial: '\u{FE80}', joining: Joining::None }, // ء
        '\u{0622}' => right_forms('\u{FE81}', '\u{FE82}'),                        // آ
        '\u{0623}' => right_forms('\u{FE83}', '\u{FE84}'),                        // أ
        '\u{0624}' => right_forms('\u{FE85}', '\u{FE86}'),                        // ؤ
        '\u{0625}' => right_forms('\u{FE87}', '\u{FE88}'),                        // إ
        '\u{0626}' => dual_forms('\u{FE89}', '\u{FE8A}', '\u{FE8B}', '\u{FE8C}'), // ئ
        '\u{0627}' => right_forms('\u{FE8D}', '\u{FE8E}'),                        // ا
        '\u{0628}' => dual_forms('\u{FE8F}', '\u{FE90}', '\u{FE91}', '\u{FE92}'), // ب
        '\u{0629}' => right_forms('\u{FE93}', '\u{FE94}'),                        // ة
        '\u{062A}' => dual_forms('\u{FE95}', '\u{FE96}', '\u{FE97}', '\u{FE98}'), // ت
        '\u{062B}' => dual_forms('\u{FE99}', '\u{FE9A}', '\u{FE9B}', '\u{FE9C}'), // ث
        '\u{062C}' => dual_forms('\u{FE9D}', '\u{FE9E}', '\u{FE9F}', '\u{FEA0}'), // ج
        '\u{062D}' => dual_forms('\u{FEA1}', '\u{FEA2}', '\u{FEA3}', '\u{FEA4}'), // ح
        '\u{062E}' => dual_forms('\u{FEA5}', '\u{FEA6}', '\u{FEA7}', '\u{FEA8}'), // خ
        '\u{062F}' => right_forms('\u{FEA9}', '\u{FEAA}'),                        // د
        '\u{0630}' => right_forms('\u{FEAB}', '\u{FEAC}'),                        // ذ
        '\u{0631}' => right_forms('\u{FEAD}', '\u{FEAE}'),                        // ر
        '\u{0632}' => right_forms('\u{FEAF}', '\u{FEB0}'),                        // ز
        '\u{0633}' => dual_forms('\u{FEB1}', '\u{FEB2}', '\u{FEB3}', '\u{FEB4}'), // س
        '\u{0634}' => dual_forms('\u{FEB5}', '\u{FEB6}', '\u{FEB7}', '\u{FEB8}'), // ش
        '\u{0635}' => dual_forms('\u{FEB9}', '\u{FEBA}', '\u{FEBB}', '\u{FEBC}'), // ص
        '\u{0636}' => dual_forms('\u{FEBD}', '\u{FEBE}', '\u{FEBF}', '\u{FEC0}'), // ض
        '\u{0637}' => dual_forms('\u{FEC1}', '\u{FEC2}', '\u{FEC3}', '\u{FEC4}'), // ط
        '\u{0638}' => dual_forms('\u{FEC5}', '\u{FEC6}', '\u{FEC7}', '\u{FEC8}'), // ظ
        '\u{0639}' => dual_forms('\u{FEC9}', '\u{FECA}', '\u{FECB}', '\u{FECC}'), // ع
        '\u{063A}' => dual_forms('\u{FECD}', '\u{FECE}', '\u{FECF}', '\u{FED0}'), // غ
        '\u{0640}' => dual_forms('\u{0640}', '\u{0640}', '\u{0640}', '\u{0640}'), // ـ tatweel
        '\u{0641}' => dual_forms('\u{FED1}', '\u{FED2}', '\u{FED3}', '\u{FED4}'), // ف
        '\u{0642}' => dual_forms('\u{FED5}', '\u{FED6}', '\u{FED7}', '\u{FED8}'), // ق
        '\u{0643}' => dual_forms('\u{FED9}', '\u{FEDA}', '\u{FEDB}', '\u{FEDC}'), // ك
        '\u{0644}' => dual_forms('\u{FEDD}', '\u{FEDE}', '\u{FEDF}', '\u{FEE0}'), // ل
        '\u{0645}' => dual_forms('\u{FEE1}', '\u{FEE2}', '\u{FEE3}', '\u{FEE4}'), // م
        '\u{0646}' => dual_forms('\u{FEE5}', '\u{FEE6}', '\u{FEE7}', '\u{FEE8}'), // ن
        '\u{0647}' => dual_forms('\u{FEE9}', '\u{FEEA}', '\u{FEEB}', '\u{FEEC}'), // ه
        '\u{0648}' => right_forms('\u{FEED}', '\u{FEEE}'),                        // و
        '\u{0649}' => right_forms('\u{FEEF}', '\u{FEF0}'),                        // ى
        '\u{064A}' => dual_forms('\u{FEF1}', '\u{FEF2}', '\u{FEF3}', '\u{FEF4}'), // ي
        // Persian / Urdu extensions
        '\u{0679}' => dual_forms('\u{FB66}', '\u{FB67}', '\u{FB68}', '\u{FB69}'), // ٹ
        '\u{067E}' => dual_forms('\u{FB56}', '\u{FB57}', '\u{FB58}', '\u{FB59}'), // پ
        '\u{0686}' => dual_forms('\u{FB7A}', '\u{FB7B}', '\u{FB7C}', '\u{FB7D}'), // چ
        '\u{0688}' => right_forms('\u{FB88}', '\u{FB89}'),                        // ڈ
        '\u{0691}' => right_forms('\u{FB8C}', '\u{FB8D}'),                        // ڑ
        '\u{0698}' => right_forms('\u{FB8A}', '\u{FB8B}'),                        // ژ
        '\u{06A9}' => dual_forms('\u{FB8E}', '\u{FB8F}', '\u{FB90}', '\u{FB91}'), // ک
        '\u{06AF}' => dual_forms('\u{FB92}', '\u{FB93}', '\u{FB94}', '\u{FB95}'), // گ
        '\u{06BA}' => right_forms('\u{FB9E}', '\u{FB9F}'),                        // ں
        '\u{06C1}' => dual_forms('\u{FBA6}', '\u{FBA7}', '\u{FBA8}', '\u{FBA9}'), // ہ
        '\u{06CC}' => dual_forms('\u{FBFC}', '\u{FBFD}', '\u{FBFE}', '\u{FBFF}'), // ی
        '\u{06D2}' => right_forms('\u{FBAE}', '\u{FBAF}'),                        // ے
        _ if is_arabic_diacritic(ch) => ArabicForms { isolated: ch, final_form: ch, initial: ch, medial: ch, joining: Joining::Transparent },
        _ => return None,
    })
}

fn is_arabic_diacritic(ch: char) -> bool {
    matches!(ch as u32, 0x064B..=0x065F | 0x0670 | 0x06D6..=0x06ED)
}

fn joining_of(ch: char) -> Joining {
    arabic_forms(ch).map_or(Joining::None, |f| f.joining)
}

/// Checks if `lam` (`ل`) followed by `alef` forms a mandatory Lam-Alef ligature:
/// returns `(isolated_ligature, final_ligature)`.
fn lam_alef_ligature(alef: char) -> Option<(char, char)> {
    Some(match alef {
        '\u{0622}' => ('\u{FEF5}', '\u{FEF6}'), // لآ
        '\u{0623}' => ('\u{FEF7}', '\u{FEF8}'), // لأ
        '\u{0625}' => ('\u{FEF9}', '\u{FEFA}'), // لإ
        '\u{0627}' => ('\u{FEFB}', '\u{FEFC}'), // لا
        _ => return None,
    })
}

/// Shapes a logical sequence of characters by replacing Arabic letters with their contextual
/// Presentation Forms (including Lam-Alef ligatures), preserving logical order.
pub fn shape_arabic_logical(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut out = String::with_capacity(text.len());

    // Helper: does the previous non-transparent character join to the left (Dual)?
    let prev_joins_left = |idx: usize| -> bool {
        let mut j = idx;
        while j > 0 {
            j -= 1;
            let jj = joining_of(chars[j]);
            if jj != Joining::Transparent {
                return jj == Joining::Dual;
            }
        }
        false
    };

    // Helper: does the next non-transparent character join to the right (Dual or Right)?
    let next_joins_right = |idx: usize| -> bool {
        let mut j = idx + 1;
        while j < n {
            let jj = joining_of(chars[j]);
            if jj != Joining::Transparent {
                return matches!(jj, Joining::Dual | Joining::Right);
            }
            j += 1;
        }
        false
    };

    let mut i = 0;
    while i < n {
        let ch = chars[i];
        // Check Lam-Alef ligature (ل followed immediately by an Alef variant)
        if ch == '\u{0644}'
            && let Some(&next_ch) = chars.get(i + 1)
            && let Some((iso_lig, fin_lig)) = lam_alef_ligature(next_ch)
        {
            if prev_joins_left(i) {
                out.push(fin_lig);
            } else {
                out.push(iso_lig);
            }
            i += 2;
            continue;
        }

        if let Some(forms) = arabic_forms(ch) {
            if forms.joining == Joining::Transparent || forms.joining == Joining::None {
                out.push(forms.isolated);
            } else {
                let join_prev = prev_joins_left(i);
                let join_next = forms.joining == Joining::Dual && next_joins_right(i);
                let shaped = match (join_prev, join_next) {
                    (false, false) => forms.isolated,
                    (true, false) => forms.final_form,
                    (false, true) => forms.initial,
                    (true, true) => forms.medial,
                };
                out.push(shaped);
            }
        } else {
            out.push(ch);
        }
        i += 1;
    }
    out
}

/// Mirrors paired brackets when displayed in a right-to-left run.
fn mirror_bracket(ch: char) -> char {
    match ch {
        '(' => ')',
        ')' => '(',
        '[' => ']',
        ']' => '[',
        '{' => '}',
        '}' => '{',
        '<' => '>',
        '>' => '<',
        '«' => '»',
        '»' => '«',
        _ => ch,
    }
}

/// Shapes Arabic letters into contextual presentation forms and reorders bidirectional runs into
/// left-to-right visual order for renderers (like `egui`) that draw glyphs strictly left-to-right.
///
/// If `text` contains no RTL characters, returns `Cow::Borrowed(text)` without allocating.
pub fn shape_bidi(text: &str) -> Cow<'_, str> {
    if !contains_rtl(text) {
        return Cow::Borrowed(text);
    }
    let mut result = String::with_capacity(text.len());
    for (line_idx, line) in text.split('\n').enumerate() {
        if line_idx > 0 {
            result.push('\n');
        }
        let shaped = shape_arabic_logical(line);
        reorder_line_visual(&shaped, &mut result);
    }
    Cow::Owned(result)
}

/// Visual reordering for a single line containing RTL characters.
fn reorder_line_visual(shaped_line: &str, out: &mut String) {
    let chars: Vec<char> = shaped_line.chars().collect();
    if chars.is_empty() {
        return;
    }

    // Determine whether each character belongs to an LTR island (Latin letters / digits) or RTL flow.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum RunDir {
        Rtl,
        Ltr,
    }

    let n = chars.len();
    let mut dirs = vec![RunDir::Rtl; n];
    for (i, &ch) in chars.iter().enumerate() {
        match bidi_class(ch) {
            BidiClass::L | BidiClass::En => dirs[i] = RunDir::Ltr,
            BidiClass::R | BidiClass::Al => dirs[i] = RunDir::Rtl,
            BidiClass::Other => {
                // Keep numeric separators (.,+-/:%) and template braces ({n}, {page}) inside an LTR run
                if matches!(ch, '.' | ',' | ':' | '/' | '-' | '+')
                    && i > 0
                    && dirs[i - 1] == RunDir::Ltr
                    && chars.get(i + 1).is_some_and(|&next| matches!(bidi_class(next), BidiClass::L | BidiClass::En))
                {
                    dirs[i] = RunDir::Ltr;
                } else if ch == '{' && chars.get(i + 1).is_some_and(|&next| matches!(bidi_class(next), BidiClass::L | BidiClass::En)) {
                    dirs[i] = RunDir::Ltr;
                } else if ch == '}' && i > 0 && dirs[i - 1] == RunDir::Ltr {
                    dirs[i] = RunDir::Ltr;
                }
            }
        }
    }

    // Group into contiguous runs of (RunDir, Range)
    let mut runs: Vec<(RunDir, usize, usize)> = Vec::new();
    let mut start = 0;
    while start < n {
        let d = dirs[start];
        let mut end = start + 1;
        while end < n && dirs[end] == d {
            end += 1;
        }
        runs.push((d, start, end));
        start = end;
    }

    // Base paragraph direction for a line with RTL: if predominantly LTR, keep run order and reverse
    // only RTL runs; if RTL (or Auto with RTL), reverse the sequence of runs and reverse RTL chars.
    let base_rtl = detect_direction(shaped_line) != TextDirection::Ltr;
    if base_rtl {
        for &(dir, s, e) in runs.iter().rev() {
            let slice = chars.get(s..e).unwrap_or_default();
            match dir {
                RunDir::Rtl => {
                    for &ch in slice.iter().rev() {
                        out.push(mirror_bracket(ch));
                    }
                }
                RunDir::Ltr => {
                    for &ch in slice {
                        out.push(ch);
                    }
                }
            }
        }
    } else {
        for &(dir, s, e) in &runs {
            let slice = chars.get(s..e).unwrap_or_default();
            match dir {
                RunDir::Rtl => {
                    for &ch in slice.iter().rev() {
                        out.push(mirror_bracket(ch));
                    }
                }
                RunDir::Ltr => {
                    for &ch in slice {
                        out.push(ch);
                    }
                }
            }
        }
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

    #[test]
    fn shape_arabic_contextual_forms_and_lam_alef() {
        // "مرحبا" -> م (initial FEE3), ر (final FEAE), ح (initial FEA3), ب (medial FE92), ا (final FE8E)
        let shaped = shape_arabic_logical("مرحبا");
        assert_eq!(shaped.chars().collect::<Vec<_>>(), vec!['\u{FEE3}', '\u{FEAE}', '\u{FEA3}', '\u{FE92}', '\u{FE8E}']);

        // Visual BiDi order for "مرحبا" reverses the RTL sequence for left-to-right drawing:
        let visual = shape_bidi("مرحبا");
        assert_eq!(visual.chars().collect::<Vec<_>>(), vec!['\u{FE8E}', '\u{FE92}', '\u{FEA3}', '\u{FEAE}', '\u{FEE3}']);

        // Lam-Alef ligature: "لا" -> isolated FEFB; "بلا" -> ب (initial FE91) + لا (final FEFC)
        assert_eq!(shape_arabic_logical("لا"), "\u{FEFB}");
        assert_eq!(shape_arabic_logical("بلا"), "\u{FE91}\u{FEFC}");
    }

    #[test]
    fn shape_bidi_preserves_ltr_without_allocating_and_keeps_numbers_ltr() {
        let ascii = "Hello 12345";
        assert!(matches!(shape_bidi(ascii), Cow::Borrowed(_)));

        // Arabic with embedded number "المجموع 125" keeps "125" in left-to-right digit order
        let mixed = shape_bidi("المجموع 125");
        assert!(mixed.starts_with("125 "), "numbers stay LTR on the visual left of an RTL phrase: {mixed:?}");
    }
}
