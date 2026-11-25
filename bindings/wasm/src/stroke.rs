//! Live strokes (plan 2026-11-24, stage 3): a brush-family tool's stroke shown while it is drawn,
//! and a render that leaves one layer out while the Move tool drags it.
//!
//! The editor begins a stroke with the command and params its release commits, appends the
//! pointer's points as they come (each call answers the rectangle that changed, which the canvas
//! renders again), and ends it, or cancels it with Escape. Until it ends, the document is
//! untouched: the stroke so far is painted onto a copy (the engine's `LiveStroke`, `LiveRetouch`
//! and `LiveDab`), and [`kition_kit::KitDocument::render`] and `render_area` draw that copy. Ending
//! runs the command once with every point (and, for the Brush and Pencil, the seed the preview
//! drew its jitter with): exactly one history step, the same pixels as running the command
//! directly. Cancelling leaves the document as it was. The preview is dropped as soon as the
//! document changes under it (an undo, an agent's edit, a collaborator's change): it would show a
//! stroke on a picture that is gone.
//!
//! Live: `paint.stroke` (Brush, Eraser), `paint.pencil`, `paint.cloneStamp`,
//! `paint.healingBrush` (its texture; the blend comes on release), `paint.historyBrush`,
//! `paint.dodge`, `paint.burn`, `paint.sponge`, `paint.blur`, `paint.sharpen`, `paint.smudge`
//! (not with Sample All Layers). The others (Spot Healing, Mixer Brush, Background Eraser) have
//! no live preview in the engine and still paint on release.
//!
//! These are Photo's own methods beside the Kit SDK's contract (`strokeBegin`, `strokePush`,
//! `strokeEnd`, `strokeCancel`, `omitLayer` in [`crate::web`]), not commands: nothing an agent
//! runs, and nothing a host records.

use std::sync::Arc;

use kition_photo_engine::brush_cmds::LiveStroke;
use kition_photo_engine::doc::{DocId, Document, LayerId};
use kition_photo_engine::paint::StrokePoint;
use kition_photo_engine::retouch_cmds::{LiveDab, LiveRetouch};
use kition_photo_geom::Rect;
use serde_json::{Value, json};

use crate::PhotoDocument;
use crate::context::{disabled, invalid};

/// The Brush and the Pencil: their seed must reach the commit.
const BRUSHES: &[&str] = &["paint.stroke", "paint.pencil"];
/// The retouching strokes the engine shows through the clone source or a history state.
const RETOUCH: &[&str] = &["paint.cloneStamp", "paint.healingBrush", "paint.historyBrush"];
/// The sequential-dab tools.
const DABS: &[&str] = &["paint.dodge", "paint.burn", "paint.sponge", "paint.blur", "paint.sharpen", "paint.smudge"];

/// The most points one stroke holds (a long stroke of a fast pen is a few tens of thousands), and
/// the most one call appends.
pub const MAX_POINTS: usize = 4_000_000;
pub const MAX_PUSH: usize = 1_000_000;

/// Whether `command` has a live preview.
pub fn is_live(command: &str) -> bool {
    BRUSHES.contains(&command) || RETOUCH.contains(&command) || DABS.contains(&command)
}

/// The engine's preview of one stroke.
enum Paint {
    Brush(Box<LiveStroke>),
    Retouch(Box<LiveRetouch>),
    Dab(Box<LiveDab>),
}

impl Paint {
    fn doc(&self) -> &Arc<Document> {
        match self {
            Self::Brush(live) => &live.doc,
            Self::Retouch(live) => &live.doc,
            Self::Dab(live) => &live.doc,
        }
    }

    fn bounds(&self) -> Rect {
        match self {
            Self::Brush(live) => live.bounds(),
            Self::Retouch(live) => live.bounds(),
            Self::Dab(live) => live.bounds(),
        }
    }

    fn push(&mut self, points: &[StrokePoint]) -> kition_photo_engine::Result<Rect> {
        match self {
            Self::Brush(live) => live.push(points),
            Self::Retouch(live) => live.push(points),
            Self::Dab(live) => live.push(points),
        }
    }
}

/// Which document state a preview belongs to: it shows only while the document is still there.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Stamp {
    doc: DocId,
    revision: u64,
}

/// A stroke being drawn.
pub(crate) struct Live {
    id: u32,
    command: String,
    /// The params as given (with the seed), without the points.
    params: Value,
    points: Vec<Value>,
    stamp: Stamp,
    paint: Paint,
}

/// The live stroke and the layer the Move tool's drag leaves out, kept beside the session.
#[derive(Default)]
pub(crate) struct LiveState {
    live: Option<Live>,
    last: u32,
    omitted: Option<(LayerId, Stamp)>,
}

/// `[x0, y0, x1, y1]`, the empty rectangle as `[0, 0, 0, 0]`.
fn corners(rect: Rect) -> [i32; 4] {
    if rect.is_empty() { [0; 4] } else { [rect.x0, rect.y0, rect.x1, rect.y1] }
}

/// The JSON points of `params` (`[x, y, pressure?, …]` arrays or `{x, y, …}` objects), at most
/// [`MAX_POINTS`].
fn given_points(command: &str, params: &Value) -> Result<Vec<Value>, String> {
    let points = params.get("points").and_then(Value::as_array).ok_or_else(|| invalid(format!("`{command}` needs `points`")))?;
    if points.is_empty() {
        return Err(invalid("`points` is empty"));
    }
    if points.len() > MAX_POINTS {
        return Err(invalid(format!("a stroke holds at most {MAX_POINTS} points")));
    }
    Ok(points.clone())
}

impl PhotoDocument {
    fn stamp(&self) -> Option<Stamp> {
        let st = self.session.active()?;
        Some(Stamp { doc: st.doc.id, revision: st.revision })
    }

    /// Begins a live stroke of `command` with `params`, the params its release commits with the
    /// points so far (at least one). Answers `{stroke, damage: [x0, y0, x1, y1], seed}`: the
    /// stroke's number for the calls that follow, what it painted, and the jitter seed the commit
    /// uses (the Brush's and Pencil's; null for the others). A stroke already under way is dropped.
    /// Refused, changing nothing, when the command has no live preview or could not run now.
    pub fn stroke_begin(&mut self, command: &str, params: Value) -> Result<Value, String> {
        if self.is_agent() {
            return Err(disabled("a live stroke is a person's gesture on the canvas; run the command instead"));
        }
        if !is_live(command) {
            return Err(invalid(format!("`{command}` has no live stroke; run it on release")));
        }
        crate::params::check(command, &params)?;
        if !params.is_object() {
            return Err(invalid("params must be a JSON object"));
        }
        let points = given_points(command, &params)?;
        self.strokes.live = None;
        let mut params = params;
        let brush = BRUSHES.contains(&command);
        if brush && params.get("seed").and_then(Value::as_u64).is_none() {
            // Chosen here so the editor can hand it back exactly: a JavaScript number holds 53 bits.
            let seed = kition_photo_engine::paint::rng::seed_from_bytes(Value::Array(points.clone()).to_string().as_bytes()) & ((1 << 53) - 1);
            params["seed"] = json!(seed);
        }
        let run_params = self.session.preflight(command, &params).map_err(crate::engine_error)?;
        let session = &self.session;
        let paint = if brush {
            Paint::Brush(Box::new(LiveStroke::begin_with(session, command, &run_params).map_err(crate::engine_error)?))
        } else if RETOUCH.contains(&command) {
            Paint::Retouch(Box::new(LiveRetouch::begin(session, command, &run_params).map_err(crate::engine_error)?))
        } else {
            Paint::Dab(Box::new(LiveDab::begin(session, command, &run_params).map_err(crate::engine_error)?))
        };
        let stamp = self.stamp().ok_or_else(|| disabled("no image is open"))?;
        self.strokes.last = self.strokes.last.wrapping_add(1).max(1);
        let id = self.strokes.last;
        let damage = corners(paint.bounds());
        let seed = params.get("seed").cloned().filter(|_| brush).unwrap_or(Value::Null);
        if let Some(fields) = params.as_object_mut() {
            fields.remove("points");
        }
        self.strokes.live = Some(Live { id, command: command.to_owned(), params, points, stamp, paint });
        Ok(json!({ "stroke": id, "damage": damage, "seed": seed }))
    }

    /// The live stroke `stroke`, when it is the one under way and the document is still the one it
    /// began on; a stale one is dropped.
    fn live_mut(&mut self, stroke: u32) -> Result<&mut Live, String> {
        let stamp = self.stamp();
        match &self.strokes.live {
            Some(live) if live.id == stroke && Some(live.stamp) == stamp => {}
            Some(live) if live.id == stroke => {
                self.strokes.live = None;
                return Err(disabled("the image changed under the stroke; its preview is gone"));
            }
            _ => return Err(disabled(format!("no live stroke {stroke}"))),
        }
        self.strokes.live.as_mut().ok_or_else(|| disabled(format!("no live stroke {stroke}")))
    }

    /// Appends points to stroke `stroke`, as flat `[x, y, pressure, …]` triples (pressure 0 to 1),
    /// and answers the rectangle that changed `[x0, y0, x1, y1]` (`[0, 0, 0, 0]` when none). A
    /// point off the coordinate range, a pressure that is not a number, or too many points refuse
    /// the whole call and leave the stroke as it was.
    pub fn stroke_push(&mut self, stroke: u32, points: &[f64]) -> Result<[i32; 4], String> {
        if !points.len().is_multiple_of(3) {
            return Err(invalid("points are [x, y, pressure] triples"));
        }
        let count = points.len() / 3;
        if count > MAX_PUSH {
            return Err(invalid(format!("at most {MAX_PUSH} points at a time")));
        }
        let live = self.live_mut(stroke)?;
        if live.points.len().saturating_add(count) > MAX_POINTS {
            return Err(invalid(format!("a stroke holds at most {MAX_POINTS} points")));
        }
        let mut stroke_points = Vec::with_capacity(count);
        let mut given = Vec::with_capacity(count);
        for &[x, y, pressure] in points.as_chunks::<3>().0 {
            if !pressure.is_finite() {
                return Err(invalid("a point's pressure must be a number from 0 to 1"));
            }
            let pressure = pressure.clamp(0.0, 1.0);
            stroke_points.push(StrokePoint::new(x, y, pressure as f32));
            given.push((x, y, pressure));
        }
        // The engine refuses a point off the coordinate range (or not a number) before it paints.
        let damage = live.paint.push(&stroke_points).map_err(crate::engine_error)?;
        live.points.extend(given.into_iter().map(|(x, y, pressure)| json!([x, y, pressure])));
        Ok(corners(damage))
    }

    /// Ends stroke `stroke`: runs its command once with every point, the step it leaves in the
    /// history, and answers the command's answer. Runs even when the image changed under the
    /// preview (the stroke is the person's), as the release always did.
    pub fn stroke_end(&mut self, stroke: u32) -> Result<Value, String> {
        let live = match self.strokes.live.take() {
            Some(live) if live.id == stroke => live,
            other => {
                self.strokes.live = other;
                return Err(disabled(format!("no live stroke {stroke}")));
            }
        };
        let mut params = live.params;
        params["points"] = Value::Array(live.points);
        self.run(&live.command, params, None)
    }

    /// Drops stroke `stroke` without changing the document; answers the area its preview covered,
    /// to draw again (`[0, 0, 0, 0]` when there was none, or another stroke is under way).
    pub fn stroke_cancel(&mut self, stroke: u32) -> [i32; 4] {
        match self.strokes.live.take() {
            Some(live) if live.id == stroke => corners(live.paint.bounds()),
            other => {
                self.strokes.live = other;
                [0; 4]
            }
        }
    }

    /// Leaves layer `layer` out of what the canvas renders until the document next changes (the
    /// Move tool's drag shows the layer at the pointer, not where it was); `None` puts it back.
    pub fn omit_layer(&mut self, layer: Option<u32>) -> Result<(), String> {
        let Some(layer) = layer else {
            self.strokes.omitted = None;
            return Ok(());
        };
        let id = LayerId(u64::from(layer));
        let st = self.state()?;
        if st.doc.layer(id).is_none() {
            return Err(invalid(format!("no layer {layer}")));
        }
        self.strokes.omitted = self.stamp().map(|stamp| (id, stamp));
        Ok(())
    }

    /// What the canvas shows: the document, with the live stroke so far, without the layer a
    /// drag leaves out.
    pub(crate) fn shown(&self) -> Result<std::borrow::Cow<'_, Document>, String> {
        let st = self.state()?;
        let stamp = self.stamp();
        let doc: &Document = match &self.strokes.live {
            Some(live) if Some(live.stamp) == stamp => live.paint.doc(),
            _ => &st.doc,
        };
        match self.strokes.omitted {
            Some((layer, at)) if Some(at) == stamp && doc.layer(layer).is_some() => {
                let mut out = doc.clone();
                if let Some(found) = out.layer_mut(layer) {
                    found.visible = false;
                }
                Ok(std::borrow::Cow::Owned(out))
            }
            _ => Ok(std::borrow::Cow::Borrowed(doc)),
        }
    }
}
