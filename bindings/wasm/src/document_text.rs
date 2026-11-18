//! `document.text` (KitText 1.0, kition-sdk `docs/platform/sets/text.md`): the image's words as
//! plain text, so a host can index them for search. An image's words live in its type layers,
//! outside the object model a host reads.
//!
//! Each visible type layer is one block and one part, labelled `Layer: <name>`, in the Layers
//! panel's order (top to bottom, into groups). Hidden layers, and every layer of a hidden group,
//! are left out. Paragraph breaks become newlines. Reading changes nothing.

use kition_kit::text::{DocumentText, TextBuilder};
use kition_photo_engine::doc::{Document, Layer, LayerContent};

/// Adds the type layers of `stack` (bottom to top) top first; false once the limit is reached.
fn layers(stack: &[Layer], out: &mut TextBuilder) -> bool {
    for layer in stack.iter().rev().filter(|layer| layer.visible) {
        let more = match &layer.content {
            LayerContent::Text(text) => {
                let words = text.text.replace("\r\n", "\n").replace('\r', "\n");
                words.trim().is_empty() || out.part(format!("Layer: {}", layer.name), &words)
            }
            LayerContent::Group(group) => layers(&group.children, out),
            _ => true,
        };
        if !more {
            return false;
        }
    }
    true
}

/// The image's words, type layer by type layer (see the module docs), at most `limit` characters.
pub fn document_text(doc: &Document, limit: usize) -> DocumentText {
    let mut out = TextBuilder::new(limit);
    layers(&doc.layers, &mut out);
    out.finish()
}
