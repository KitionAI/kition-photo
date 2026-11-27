//! What a Photo kit's `check` (`KitDocument::check`, `file.check`) reports on an image (KHP
//! A21): object layers whose source is missing or only linked, layers that lie entirely outside
//! the canvas, pixel layers without pixels, hidden layers that hold content, empty type layers, and
//! fonts drawn with a substitute or characters no font can draw. Layers are named as `layer:<id>` (the ids the engine's commands
//! take in `layer`).
//!
//! Codes: `photo.empty-canvas`, `photo.missing-object-source`, `photo.linked-object-layer`,
//! `photo.off-canvas`, `photo.font-substituted`, `photo.empty-layer`, `photo.empty-text`,
//! `photo.hidden-layer`, and `photo.truncated` when a code is found more often than
//! [`PER_CODE`].

use std::collections::BTreeMap;

use kdm_model::kition_kit::diagnostics::{Diagnostic, Severity, Target};
use kition_photo_doc::{Document, Layer, LayerContent, Metadata, SmartSource};
use kition_photo_geom::Rect;
use serde_json::json;

/// At most this many diagnostics of one code; the rest are counted in one `photo.truncated`.
pub const PER_CODE: usize = 100;

/// What the check asks of the fonts that can draw text here.
pub trait Fonts {
    /// Whether `family` can be drawn here.
    fn has_family(&mut self, family: &str) -> bool;
    /// The characters of `text` that neither `family` nor any fallback font can draw.
    fn missing(&mut self, family: &str, text: &str) -> Vec<char>;
    /// The weight `family` draws `weight` at here (the nearest a face has); `None` when unknown.
    fn drawn_weight(&mut self, _family: &str, _weight: u16, _italic: bool) -> Option<f32> {
        None
    }
}

impl Fonts for kition_photo_text::FontDb {
    fn has_family(&mut self, family: &str) -> bool {
        kition_photo_text::FontDb::has_family(self, family)
    }
    fn missing(&mut self, family: &str, text: &str) -> Vec<char> {
        self.missing_chars(family, text)
    }
    fn drawn_weight(&mut self, family: &str, weight: u16, italic: bool) -> Option<f32> {
        kition_photo_text::FontDb::drawn_weight(self, family, weight, italic)
    }
}

/// Families only (every character is taken as drawable).
struct Families<'a>(&'a mut dyn FnMut(&str) -> bool);

impl Fonts for Families<'_> {
    fn has_family(&mut self, family: &str) -> bool {
        (self.0)(family)
    }
    fn missing(&mut self, _: &str, _: &str) -> Vec<char> {
        Vec::new()
    }
}

/// [`check_fonts`] with the engine's own font database (bundled, served, and system fonts).
pub fn check_document(doc: &Document) -> Vec<Diagnostic> {
    let mut text = kition_photo_text::shared().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    check_fonts(doc, &mut text.fonts)
}

/// [`check_fonts`] where `has_font` says whether a family can be drawn here, and every
/// character is taken as drawable.
pub fn check(doc: &Document, has_font: &mut dyn FnMut(&str) -> bool) -> Vec<Diagnostic> {
    check_fonts(doc, &mut Families(has_font))
}

/// At most this many missing characters are named in one diagnostic.
const NAMED_CHARS: usize = 8;

/// Everything worth fixing in the image, errors first. `fonts` says which families and characters
/// can be drawn here (the engine's font database).
pub fn check_fonts(doc: &Document, fonts: &mut dyn Fonts) -> Vec<Diagnostic> {
    let mut found = Found::default();
    if doc.size.width == 0 || doc.size.height == 0 {
        found.push(Diagnostic::new("photo.empty-canvas", Severity::Error, "The canvas has no area: nothing can be drawn or exported."));
    }
    let canvas = Rect::new(0, 0, i32::try_from(doc.size.width).unwrap_or(i32::MAX), i32::try_from(doc.size.height).unwrap_or(i32::MAX));
    let mut families = BTreeMap::new();
    let mut pieces = Vec::new();
    layers(&doc.layers, &doc.metadata, canvas, &mut families, &mut pieces, &mut found);
    for (family, layer) in families {
        if !family.is_empty() && !fonts.has_family(&family) {
            found.push(
                Diagnostic::new(
                    "photo.font-substituted",
                    Severity::Info,
                    format!("The font \"{family}\" is not available here, so its text is drawn with a substitute."),
                )
                .on(on(layer)),
            );
        }
    }
    // Weights no face of the family has, once per layer and family (drawn at the nearest weight).
    let mut weights = std::collections::BTreeSet::new();
    for piece in &pieces {
        let Some(drawn) = fonts.drawn_weight(&piece.family, piece.weight, piece.italic) else { continue };
        if (drawn - f32::from(piece.weight)).abs() >= 0.5 && weights.insert((piece.layer, piece.family.clone(), piece.weight)) {
            found.push(
                Diagnostic::new(
                    "photo.font-substituted",
                    Severity::Info,
                    format!(
                        "Type layer \"{}\" asks for {} at weight {}, which no font here has, so it is drawn at weight {drawn}. Add a font with that weight (kition-fonts' variable Inter has every weight).",
                        piece.name, piece.family, piece.weight
                    ),
                )
                .on(on(piece.layer)),
            );
        }
    }
    // Characters no font has, per type layer (drawn as the missing-glyph box).
    let mut missing: BTreeMap<u64, (String, std::collections::BTreeSet<char>)> = BTreeMap::new();
    for piece in &pieces {
        let chars = fonts.missing(&piece.family, &piece.text);
        if !chars.is_empty() {
            missing.entry(piece.layer).or_insert_with(|| (piece.name.clone(), Default::default())).1.extend(chars);
        }
    }
    for (layer, (name, chars)) in missing {
        let mut named: Vec<String> = chars.iter().take(NAMED_CHARS).map(|c| format!("\"{c}\" (U+{:04X})", u32::from(*c))).collect();
        if chars.len() > NAMED_CHARS {
            named.push(format!("{} more", chars.len() - NAMED_CHARS));
        }
        let (what, verb) = if chars.len() == 1 { ("a character", "it is") } else { ("characters", "they are") };
        found.push(
            Diagnostic::new(
                "photo.font-substituted",
                Severity::Info,
                format!(
                    "Type layer \"{name}\" has {what} that no available font can draw: {}, so {verb} drawn as a box. Use a font that has {}, or change the text.",
                    named.join(", "),
                    if chars.len() == 1 { "it" } else { "them" }
                ),
            )
            .on(on(layer)),
        );
    }
    found.finish()
}

/// A run of a type layer's text and the family and weight it asks for.
struct Piece {
    layer: u64,
    name: String,
    family: String,
    weight: u16,
    italic: bool,
    text: String,
}

#[derive(Default)]
struct Found {
    list: Vec<Diagnostic>,
    counts: BTreeMap<String, usize>,
}

impl Found {
    fn push(&mut self, diagnostic: Diagnostic) {
        let count = self.counts.entry(diagnostic.code.clone()).or_default();
        *count += 1;
        if *count <= PER_CODE {
            self.list.push(diagnostic);
        }
    }

    fn finish(mut self) -> Vec<Diagnostic> {
        for (code, count) in &self.counts {
            if *count > PER_CODE {
                self.list.push(Diagnostic::new("photo.truncated", Severity::Info, format!("{} more `{code}` diagnostics are not listed.", count - PER_CODE)));
            }
        }
        self.list.sort_by_key(|diagnostic| diagnostic.severity);
        self.list
    }
}

fn on(layer: u64) -> Target {
    Target { path: Some(format!("layer:{layer}")), ..Target::default() }
}

/// The pixels a layer shows, in document coordinates, when the engine holds them: a pixel layer's
/// content, or the rendered appearance of a type, shape, or object layer. `None` when the
/// layer has no pixels of its own (groups, adjustments, fills) or none rendered yet.
fn drawn(layer: &Layer) -> Option<Rect> {
    let surface = match &layer.content {
        LayerContent::Raster(surface) => surface,
        LayerContent::Text(text) => text.cache.as_ref()?,
        LayerContent::Shape(shape) => shape.cache.as_ref()?,
        LayerContent::Smart(smart) => smart.cache.as_ref()?,
        LayerContent::Group(_) | LayerContent::Adjustment(_) | LayerContent::Fill(_) => return None,
    };
    Some(surface.content_bounds())
}

/// Whether the source an object layer links to can be read here. While the host owns every file (a
/// kit's documents, KHP A29, A42) that is only a file the host lends from the workspace (or one the
/// PSD itself embeds): a link outside the workspace is the document's data, never read.
fn lent(meta: &Metadata, path: &str) -> bool {
    !kition_photo_format::host_files::host_files_only()
        || kition_photo_io::linked::find_linked_file(meta, path).is_some()
        || kition_photo_format::host_files::read_linked(path).is_some()
}

fn layers(list: &[Layer], meta: &Metadata, canvas: Rect, fonts: &mut BTreeMap<String, u64>, pieces: &mut Vec<Piece>, found: &mut Found) {
    for layer in list {
        let id = layer.id.0;
        let name = &layer.name;
        match &layer.content {
            LayerContent::Group(group) => layers(&group.children, meta, canvas, fonts, pieces, found),
            LayerContent::Smart(smart) => match &smart.source {
                SmartSource::Embedded { bytes, .. } if bytes.is_empty() && smart.cache.is_none() => found.push(
                    Diagnostic::new(
                        "photo.missing-object-source",
                        Severity::Error,
                        format!("Object layer \"{name}\" holds no source and no preview, so it shows nothing."),
                    )
                    .on(on(id)),
                ),
                SmartSource::Embedded { .. } => {}
                SmartSource::Linked { path } if !lent(meta, path) => {
                    let shows = if smart.cache.is_some() { "shows its saved preview" } else { "shows nothing" };
                    found.push(
                        Diagnostic::new(
                            "photo.missing-object-source",
                            Severity::Error,
                            format!(
                                "Object layer \"{name}\" links to \"{path}\", which is not a file of the workspace, so its source is missing: it {shows} and cannot be updated. Put the file in the workspace beside the image and link it there, or embed it."
                            ),
                        )
                        .on(on(id)),
                    );
                }
                SmartSource::Linked { path } => {
                    let (severity, shows) =
                        if smart.cache.is_some() { (Severity::Warning, "shows its saved preview") } else { (Severity::Error, "shows nothing") };
                    found.push(
                        Diagnostic::new(
                            if smart.cache.is_some() { "photo.linked-object-layer" } else { "photo.missing-object-source" },
                            severity,
                            format!("Object layer \"{name}\" links to \"{path}\" outside the document; it {shows} and cannot be updated here."),
                        )
                        .on(on(id)),
                    );
                }
            },
            LayerContent::Text(text) => {
                if text.text.trim().is_empty() {
                    found.push(
                        Diagnostic::new("photo.empty-text", Severity::Info, format!("Type layer \"{name}\" has no text."))
                            .on(on(id))
                            .fix("layer.delete", Some(json!({ "layer": id }))),
                    );
                }
                // One diagnostic per family, on the first layer that uses it.
                fonts.entry(text.font_family.clone()).or_insert(id);
                for run in &text.runs {
                    fonts.entry(run.style.font_family.clone()).or_insert(id);
                }
                let mut at = 0usize;
                for run in text.char_runs() {
                    let end = at.saturating_add(run.len);
                    if let Some(piece) = text.text.get(at..end) {
                        pieces.push(Piece {
                            layer: id,
                            name: name.clone(),
                            family: run.style.font_family,
                            weight: run.style.weight,
                            italic: run.style.italic,
                            text: piece.to_owned(),
                        });
                    }
                    at = end;
                }
            }
            _ => {}
        }
        let Some(bounds) = drawn(layer) else { continue };
        if bounds.is_empty() {
            if matches!(layer.content, LayerContent::Raster(_)) {
                found.push(
                    Diagnostic::new("photo.empty-layer", Severity::Info, format!("Pixel layer \"{name}\" has no pixels."))
                        .on(on(id))
                        .fix("layer.delete", Some(json!({ "layer": id }))),
                );
            }
            continue;
        }
        if bounds.intersect(&canvas).is_empty() {
            found.push(
                Diagnostic::new("photo.off-canvas", Severity::Warning, format!("Layer \"{name}\" lies entirely outside the canvas, so it never shows."))
                    .on(on(id)),
            );
        } else if !layer.visible {
            found.push(
                Diagnostic::new("photo.hidden-layer", Severity::Info, format!("Layer \"{name}\" is hidden: its content is saved but does not show."))
                    .on(on(id))
                    .fix("layer.showLayers", Some(json!({ "layer": id }))),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Session;

    fn image() -> Session {
        let mut session = Session::new();
        session.execute("file.new", json!({ "width": 64, "height": 48, "background": "white" })).expect("new image");
        session
    }

    fn codes(session: &Session, fonts: &[&str]) -> Vec<String> {
        let doc = &session.active().expect("open").doc;
        check(doc, &mut |family| fonts.contains(&family)).into_iter().map(|d| d.code).collect()
    }

    fn doc(session: &mut Session) -> &mut Document {
        std::sync::Arc::make_mut(&mut session.active_mut().expect("open").doc)
    }

    fn top(session: &mut Session) -> &mut Layer {
        doc(session).layers.last_mut().expect("a layer")
    }

    #[test]
    fn a_fresh_image_is_clean() {
        assert_eq!(codes(&image(), &[]), Vec::<String>::new());
    }

    #[test]
    fn reports_an_empty_pixel_layer_with_a_delete_hint() {
        let mut session = image();
        session.execute("layer.new.layer", json!({})).expect("new layer");
        let id = top(&mut session).id.0;
        let doc = &session.active().expect("open").doc;
        let found = check(doc, &mut |_| true);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].code, "photo.empty-layer");
        assert_eq!(found[0].target.path.as_deref(), Some(format!("layer:{id}").as_str()));
        assert_eq!(found[0].fix.as_ref().map(|f| f.command.as_str()), Some("layer.delete"));
    }

    #[test]
    fn reports_hidden_content_and_content_off_the_canvas() {
        let mut session = image();
        top(&mut session).visible = false;
        assert_eq!(codes(&session, &[]), ["photo.hidden-layer"]);
        let mut session = image();
        let LayerContent::Raster(surface) = &top(&mut session).content else { panic!("pixel layer") };
        let moved = surface.translated(500, 0, surface.content_bounds());
        top(&mut session).content = LayerContent::Raster(moved);
        assert_eq!(codes(&session, &[]), ["photo.off-canvas"]);
    }

    #[test]
    fn reports_linked_and_missing_smart_sources_errors_first() {
        let mut session = image();
        session.execute("layer.new.layer", json!({})).expect("new layer");
        let smart = |source| LayerContent::Smart(kition_photo_doc::ObjectLayer::new(source, kition_photo_geom::Affine::IDENTITY, None));
        top(&mut session).content = smart(SmartSource::Linked { path: "photos/beach.jpg".into() });
        let doc = &session.active().expect("open").doc;
        let found = check(doc, &mut |_| true);
        assert_eq!(found.iter().map(|d| (d.code.as_str(), d.severity)).collect::<Vec<_>>(), [("photo.missing-object-source", Severity::Error)]);
        assert!(found[0].message.contains("photos/beach.jpg"));
        top(&mut session).content = smart(SmartSource::Embedded { file_name: "x.png".into(), bytes: std::sync::Arc::new(Vec::new()) });
        assert_eq!(codes(&session, &[]), ["photo.missing-object-source"]);
    }

    #[test]
    fn reports_empty_text_and_each_missing_font_once() {
        let mut session = image();
        let made = session.execute("text.add", json!({ "x": 4, "y": 20, "text": "Hello", "size": 12, "color": "#000000" })).expect("type layer");
        let id = kition_photo_doc::LayerId(made["layer"].as_u64().expect("layer id"));
        session.execute("text.add", json!({ "x": 4, "y": 40, "text": "World", "size": 12, "color": "#000000" })).expect("type layer");
        let text = |session: &mut Session| match &mut doc(session).layer_mut(id).expect("layer").content {
            LayerContent::Text(text) => text.font_family.clone(),
            _ => panic!("type layer"),
        };
        let family = text(&mut session);
        assert_eq!(codes(&session, &[family.as_str()]), Vec::<String>::new());
        assert_eq!(codes(&session, &[]), ["photo.font-substituted"]);
        if let LayerContent::Text(text) = &mut doc(&mut session).layer_mut(id).expect("layer").content {
            text.text.clear();
        }
        assert!(codes(&session, &[family.as_str()]).contains(&"photo.empty-text".to_owned()));
    }

    #[test]
    fn caps_each_code_and_says_how_many_more() {
        let mut session = image();
        for _ in 0..(PER_CODE + 3) {
            session.execute("layer.new.layer", json!({})).expect("new layer");
        }
        let doc = &session.active().expect("open").doc;
        let found = check(doc, &mut |_| true);
        assert_eq!(found.iter().filter(|d| d.code == "photo.empty-layer").count(), PER_CODE);
        let truncated: Vec<_> = found.iter().filter(|d| d.code == "photo.truncated").collect();
        assert_eq!(truncated.len(), 1);
        assert!(truncated[0].message.starts_with("3 more `photo.empty-layer`"));
        kdm_model::kition_kit::diagnostics::check_diagnostics(&found).expect("valid diagnostics");
    }
}
