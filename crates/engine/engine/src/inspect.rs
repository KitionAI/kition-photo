//! JSON views of engine state for automation, tests and debugging.

use kition_photo_doc::{Layer, LayerContent, SmartSource};
use kition_photo_geom::Rect;
use serde_json::{Value, json};

use crate::{DocState, Session};

pub fn session(s: &Session) -> Value {
    json!({
        "active": s.active_index(),
        "documents": s.documents().iter().enumerate().map(|(i, d)| json!({
            "index": i,
            "name": d.doc.name,
            "path": d.path,
            "width": d.doc.size.width,
            "height": d.doc.size.height,
            "dirty": d.is_dirty(),
            "revision": d.revision,
        })).collect::<Vec<_>>(),
        "foreground": s.tools.foreground,
        "background": s.tools.background,
    })
}

pub fn document(d: &DocState) -> Value {
    let doc = &d.doc;
    let selected = d.selected_layers();
    json!({
        "name": doc.name,
        "width": doc.size.width,
        "height": doc.size.height,
        "mode": format!("{:?}", doc.mode),
        "depth": doc.depth.bits(),
        "resolution": doc.resolution_dpi,
        "activeLayer": d.active_layer.map(|l| l.0),
        "selectedLayers": selected.iter().map(|l| l.0).collect::<Vec<_>>(),
        "isolatedLayers": d.isolated_layers.iter().map(|l| l.0).collect::<Vec<_>>(),
        // Layers whose effects rows the Layers panel folds (`layer.setEffectsExpanded`).
        "effectsCollapsed": d.fx_collapsed.iter().map(|l| l.0).collect::<Vec<_>>(),
        "hasSelection": doc.selection.is_some(),
        "selectionBounds": doc.selection.as_ref().map(|s| { let r = s.content_bounds(); [r.x0, r.y0, r.width() as i32, r.height() as i32] }),
        "layers": doc.layers.iter().rev().map(|l| layer_sel(l, &selected, doc.bounds())).collect::<Vec<_>>(),
        "history": d.history.entries(),
        // The current step's index in `history` (the History panel's highlighted row).
        "historyIndex": d.history.past_len(),
        // The states Redo steps through, the next one first (the History panel's dimmed rows).
        "historyRedo": d.history.redo_labels().collect::<Vec<_>>(),
        "canUndo": d.history.can_undo(),
        "canRedo": d.history.can_redo(),
        "revision": d.revision,
        "channels": crate::channel_cmds::channels_json(d),
        "quickMask": doc.quick_mask.is_some(),
        "symmetry": d.symmetry.as_ref().map(|s| s.inspect()),
        "layerMaskView": crate::mask_view_cmds::view_json(d),
        "layerComps": doc.layer_comps.iter().map(|c| json!({"id": c.id, "name": c.name})).collect::<Vec<_>>(),
        "lastAppliedComp": doc.last_applied_comp,
        "measurement": {"scale": doc.measurement.scale.describe(), "ruler": doc.measurement.ruler.map(|r| json!({"start": r.start, "end": r.end, "protractor": r.protractor})), "count": doc.measurement.count_total(), "countGroups": doc.measurement.count_groups.len()},
        "notes": doc.notes.iter().map(|n| json!({"author": n.author, "text": n.text, "position": n.position})).collect::<Vec<_>>(),
        "timeline": doc.timeline.as_ref().map(|t| json!({"fps": t.fps, "duration": t.duration, "current": t.current})),
        "variables": (!doc.variables.is_empty()).then(|| json!({
            "defs": doc.variables.defs.iter().map(|vd| json!({"name": vd.name, "layer": vd.layer.0})).collect::<Vec<_>>(),
            "dataSets": doc.variables.data_sets.iter().map(|s| s.name.clone()).collect::<Vec<_>>(),
            "active": doc.variables.active,
        })),
    })
}

/// Layer tree top-to-bottom (display order). Without the canvas, a fill layer has no bounds.
pub fn layer(l: &Layer) -> Value {
    layer_sel(l, &[], Rect::EMPTY)
}

/// `[x, y, width, height]`, the form every command answers bounds in.
pub fn xywh(r: Rect) -> [i32; 4] {
    [r.x0, r.y0, i32::try_from(r.width()).unwrap_or(i32::MAX), i32::try_from(r.height()).unwrap_or(i32::MAX)]
}

/// Where a fill layer draws: the canvas, cut to its layer mask when the mask hides everything it
/// does not reveal, and to its vector mask's path. `None` when that is nothing.
fn fill_bounds(l: &Layer, canvas: Rect) -> Option<[i32; 4]> {
    let mut r = canvas;
    if let Some(m) = l.mask.as_ref().filter(|m| m.enabled && m.density >= 1.0)
        && m.surface.default_pixel().first().is_some_and(|v| *v < 0.5)
    {
        r = r.intersect(&m.surface.content_bounds());
    }
    if let Some(vm) = l.vector_mask.as_ref().filter(|vm| vm.enabled && vm.density >= 1.0 && !vm.path.inverted && !vm.path.is_empty())
        && let Some((x0, y0, x1, y1)) = vm.path.control_bounds()
    {
        let edge = |v: f64| if v.is_finite() { v.clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32 } else { 0 };
        r = r.intersect(&Rect::new(edge(x0.floor()), edge(y0.floor()), edge(x1.ceil()), edge(y1.ceil())));
    }
    (!r.is_empty()).then(|| xywh(r))
}

/// [`layer`] with a `selected` flag on every node (Layers panel multi-selection); `canvas` bounds
/// the fill layers.
fn layer_sel(l: &Layer, selected: &[kition_photo_doc::LayerId], canvas: Rect) -> Value {
    let bounds = match &l.content {
        LayerContent::Fill(_) => fill_bounds(l, canvas),
        _ => l.surface().map(|s| xywh(s.content_bounds())),
    };
    let mut v = json!({
        "id": l.id.0,
        "name": l.name,
        "kind": l.content.kind_name(),
        "visible": l.visible,
        "locks": serde_json::to_value(l.locks).unwrap_or(Value::Null),
        "labelColor": l.label.id(),
        "opacity": l.opacity,
        "fill": l.fill_opacity,
        "blend": l.blend.label(),
        "clipped": l.clipped,
        "hasMask": l.mask.is_some(),
        // Whether the layer mask applies (Shift-click its thumbnail); null without a mask.
        "maskEnabled": l.mask.as_ref().map(|m| m.enabled),
        // Whether the layer mask moves with the layer (the chain between the thumbnails).
        "maskLinked": l.mask.as_ref().map(|m| m.linked),
        // Properties › Masks: how much the mask hides (0 to 1) and its feather (px); null without a mask.
        "maskDensity": l.mask.as_ref().map(|m| m.density),
        "maskFeather": l.mask.as_ref().map(|m| f64::from(m.feather)),
        "bounds": bounds,
        "selected": selected.contains(&l.id),
        "linkGroup": l.link_group,
    });
    match &l.content {
        LayerContent::Group(g) => {
            v["children"] = Value::Array(g.children.iter().rev().map(|c| layer_sel(c, selected, canvas)).collect());
            v["expanded"] = json!(g.expanded);
            if let Some(a) = &g.artboard {
                v["artboard"] = json!({"rect": [a.rect.x0, a.rect.y0, a.rect.width(), a.rect.height()], "background": a.background.name(), "preset": a.preset});
            }
        }
        LayerContent::Adjustment(a) => {
            v["adjustment"] = serde_json::to_value(a).unwrap_or(Value::Null);
            // The kind and its settings as `layer.setAdjustment` takes them (the Properties panel's fields).
            v["adjustmentKind"] = json!(crate::commands::adjustment_kind(a));
            v["adjustmentParams"] = crate::adjust_params::to_params(a);
        }
        LayerContent::Smart(sm) => {
            v["editableFilters"] = Value::Array(
                sm.editable_filters
                    .iter()
                    .map(|f| json!({"command": f.command, "params": f.params, "visible": f.visible, "opacity": f.opacity, "blend": f.blend.label()}))
                    .collect(),
            );
            v["editableFiltersEnabled"] = json!(sm.filters_enabled);
            // Where the contents come from. A PSD placed layer (embedded or linked in Photoshop) is
            // `linked` to the `Idnt` uuid of its file, which its duplicates share; New Object Layer
            // via Copy embeds its own copy of the file instead.
            v["smartSource"] = match &sm.source {
                SmartSource::Embedded { file_name, .. } => json!({"kind": "embedded", "fileName": file_name}),
                SmartSource::Linked { path } => json!({"kind": "linked", "path": path}),
            };
        }
        LayerContent::Text(t) => {
            let weight = t.runs.first().map_or(400, |run| run.style.weight);
            v["text"] = json!({"text": t.text, "font": t.font_family, "sizePt": t.size_pt, "weight": weight});
        }
        // What a fill layer fills with (`fill` is the layer's fill opacity), as the fill commands take it.
        LayerContent::Fill(f) => {
            v["fillContent"] = crate::vector_cmds::fill_json(f);
        }
        // A shape's kind, fill and stroke, as shape.info answers them.
        LayerContent::Shape(sh) => {
            v["shape"] = json!({
                "kind": sh.live.as_ref().map_or("path", crate::vector_cmds::live_kind),
                "fill": sh.fill.as_ref().map(crate::vector_cmds::fill_json),
                "stroke": sh.stroke.as_ref().map(crate::vector_cmds::stroke_json),
            });
        }
        _ => {}
    }
    // Advanced Blending › Channels (only when some channel is left out).
    if l.excluded_channels != 0 {
        v["channels"] = json!((0..4).map(|i| l.excluded_channels & (1 << i) == 0).collect::<Vec<_>>());
    }
    // Blending Options › Advanced Blending switches (only when one differs from Photoshop's default).
    if !l.advanced.is_default() {
        v["advancedBlending"] = serde_json::to_value(l.advanced).unwrap_or(Value::Null);
    }
    // Blending Options › Blend If (only when set): range index (0 = Gray, then the mode's
    // channels) with This Layer / Underlying Layer as [blackLo, blackHi, whiteLo, whiteHi].
    if !l.blend_if.is_default() {
        v["blendIf"] = l
            .blend_if
            .ranges
            .iter()
            .enumerate()
            .filter(|(_, r)| !r.iter().all(kition_photo_doc::BlendRange::is_full))
            .map(|(i, [this, under])| json!({"channel": i, "thisLayer": this.to_bytes(), "underlying": under.to_bytes()}))
            .collect();
    }
    // Layer styles, so agents can verify what they applied (full settings via the style commands);
    // `key` names the effect as the `layer.layerStyle.<key>` commands do.
    if !l.effects.items.is_empty() {
        v["effects"] = json!({
            "enabled": l.effects.enabled,
            "items": l.effects.items.iter().map(|e| json!({"kind": e.label(), "key": crate::layer_style::kind_of(e), "enabled": e.enabled()})).collect::<Vec<_>>(),
        });
    }
    // Video layer (Layer › Video Layers): the frame stack behind the displayed content.
    if let Some(vid) = &l.video {
        let source = match &vid.source {
            kition_photo_doc::VideoSource::Blank => Value::String("blank".into()),
            kition_photo_doc::VideoSource::File { path } => json!({ "file": path }),
        };
        v["video"] = json!({"frames": vid.frames.len(), "fps": vid.fps, "showAltered": vid.show_altered, "source": source});
    }
    v
}
