//! What a file stores of a value, applied when the value enters the document (task card C-photo-2):
//! a document that will be saved in a format holds every value as that format stores it, so the
//! document opened from its saved file is the document in memory, and an agent's premise guard
//! reading the file again sees no change where there was none (kition-sdk's call context).
//!
//! PSD: a layer's opacity and fill opacity are one byte each (`round(v × 255)`, read as `/ 255`).

use kition_photo_doc::{Document, Layer, LayerContent};

/// `v` as PSD stores an 8-bit fraction and reads it back.
pub fn psd_fraction(v: f32) -> f32 {
    f32::from((v.clamp(0.0, 1.0) * 255.0).round() as u8) / 255.0
}

/// Settles every layer of `doc` to what a PSD stores of it: its opacity and fill opacity.
/// Answers whether anything changed.
pub fn settle_psd(doc: &mut Document) -> bool {
    fn layers(list: &mut [Layer], depth: usize) -> bool {
        let mut changed = false;
        for layer in list {
            for value in [&mut layer.opacity, &mut layer.fill_opacity] {
                let stored = psd_fraction(*value);
                if stored.to_bits() != value.to_bits() {
                    *value = stored;
                    changed = true;
                }
            }
            // As deep as a document's groups may nest (the readers' and writers' bound).
            if depth < 64
                && let LayerContent::Group(group) = &mut layer.content
            {
                changed |= layers(&mut group.children, depth + 1);
            }
        }
        changed
    }
    layers(&mut doc.layers, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_opacity_settles_to_the_byte_a_psd_stores() {
        assert_eq!(psd_fraction(0.37), 94.0 / 255.0);
        assert_eq!(psd_fraction(94.0 / 255.0), 94.0 / 255.0, "a stored value stays");
        assert_eq!((psd_fraction(1.0), psd_fraction(0.0), psd_fraction(2.0)), (1.0, 0.0, 1.0));
    }
}
