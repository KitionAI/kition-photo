//! Automatic hyphenation of paragraph text (Paragraph panel › Hyphenate).
//!
//! Hyphenation points come from Liang's algorithm over the TeX hyphenation patterns embedded by
//! the `hypher` crate. The layout puts a soft hyphen (U+00AD) at each point, so the line breaker
//! may break a word there; a line that ends at one draws a hyphen (see [`hyphen_glyph`]).

use parley::FontData;
use skrifa::MetadataProvider;
use skrifa::instance::{LocationRef, NormalizedCoord, Size};

/// The soft hyphen the layout inserts at hyphenation points.
pub(crate) const SOFT_HYPHEN: char = '\u{AD}';
/// Words longer than this (chars) are left alone: no real word is, and it bounds the work.
const MAX_WORD_CHARS: usize = 64;
/// Shorter words are never hyphenated (Photoshop's default "Words Longer Than 5 letters").
const MIN_WORD_CHARS: usize = 6;

/// The hyphenation language for a BCP-47 tag (English when absent or unknown).
fn language(tag: Option<&str>) -> hypher::Lang {
    let code = tag.and_then(|t| t.get(..2)).map(str::to_ascii_lowercase);
    code.and_then(|c| {
        let b = c.as_bytes();
        hypher::Lang::from_iso([*b.first()?, *b.get(1)?])
    })
    .unwrap_or(hypher::Lang::English)
}

/// Byte offsets in `text` where a word may be broken with a hyphen (before the char there).
/// Only words of letters whose lowercase form has the same UTF-8 length are hyphenated.
pub(crate) fn points(text: &str, language_tag: Option<&str>) -> Vec<usize> {
    let lang = language(language_tag);
    let mut out = Vec::new();
    let mut word_start: Option<usize> = None;
    let simple = |c: char| c.is_alphabetic() && c.to_lowercase().count() == 1 && c.to_lowercase().all(|l| l.len_utf8() == c.len_utf8());
    let flush = |start: usize, end: usize, out: &mut Vec<usize>| {
        let Some(word) = text.get(start..end) else { return };
        let n = word.chars().count();
        if !(MIN_WORD_CHARS..=MAX_WORD_CHARS).contains(&n) || !word.chars().all(simple) {
            return;
        }
        let mut at = start;
        let mut syllables = hypher::hyphenate(word, lang).peekable();
        while let Some(s) = syllables.next() {
            at += s.len();
            if syllables.peek().is_some() && at < end {
                out.push(at);
            }
        }
    };
    for (i, c) in text.char_indices() {
        if c.is_alphabetic() {
            word_start.get_or_insert(i);
        } else if let Some(s) = word_start.take() {
            flush(s, i, &mut out);
        }
    }
    if let Some(s) = word_start {
        flush(s, text.len(), &mut out);
    }
    out
}

/// The glyph drawn at a hyphenated line end in `font` at `size` px: (glyph id, advance in px).
pub(crate) fn hyphen_glyph(font: &FontData, coords: &[i16], size: f32) -> Option<(u32, f32)> {
    let face = skrifa::FontRef::from_index(font.data.as_ref(), font.index).ok()?;
    let charmap = face.charmap();
    let gid = charmap.map('-').or_else(|| charmap.map('\u{2010}'))?;
    let coords: Vec<NormalizedCoord> = coords.iter().map(|&c| NormalizedCoord::from_bits(c)).collect();
    let advance = face.glyph_metrics(Size::new(size), LocationRef::new(&coords)).advance_width(gid)?;
    advance.is_finite().then_some((gid.to_u32(), advance))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_points_inside_long_words_only() {
        let text = "An extraordinarily comprehensive cat.";
        let pts = points(text, None);
        assert!(!pts.is_empty());
        for p in &pts {
            let (before, after) = (text[..*p].chars().last().unwrap(), text[*p..].chars().next().unwrap());
            assert!(before.is_alphabetic() && after.is_alphabetic(), "{p}");
        }
        assert!(pts.iter().all(|&p| p > 3 && p < 36));
        assert!(points("cat dog", None).is_empty());
        // Non-Latin, mixed and unusual text never panics.
        for odd in ["İstanbulİstanbul", "ǅemaljǅemalj", "日本語のテキスト", "", "ß".repeat(200).as_str()] {
            let _ = points(odd, Some("tr"));
            let _ = points(odd, Some("x"));
        }
    }
}
