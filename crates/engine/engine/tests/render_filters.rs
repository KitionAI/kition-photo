//! Filter › Render generators (#6): Fibers fills an empty layer, Flame follows SVG path data and
//! reports when it reaches its primitive limit instead of stopping silently, Picture Frame's
//! scallops join at the corners, and spruce and fir needles grow on their branches.

use kition_photo_algo::render2::{self, FlameSpec, FlameType, FrameSpec, Prim, Shape, TreeSpec};
use kition_photo_engine::Session;
use kition_photo_geom::Rect;
use serde_json::{Value, json};

fn session(width: u32, height: u32) -> Session {
    let mut s = Session::new();
    s.execute("file.new", json!({"width": width, "height": height})).unwrap();
    s.execute("layer.new.layer", json!({})).unwrap();
    s
}

fn layer_pixels(s: &Session, area: Rect) -> Vec<f32> {
    let d = s.active().unwrap();
    d.doc.layer(d.active_layer.unwrap()).unwrap().surface().unwrap().read_region(area)
}

#[test]
fn fibers_fill_an_empty_layer() {
    let mut s = session(64, 48);
    s.execute("filter.render.fibers", json!({"foreground": "#000000", "background": "#ffffff"})).unwrap();
    let px = layer_pixels(&s, Rect::new(0, 0, 64, 48));
    assert!(px.as_chunks::<4>().0.iter().all(|p| p[3] > 0.999), "fibers left transparent pixels on an empty layer");
    // Inside a selection only.
    let mut s = session(64, 48);
    s.execute("select.rect", json!({"x": 0, "y": 0, "width": 32, "height": 48})).unwrap();
    s.execute("filter.render.fibers", json!({})).unwrap();
    let px = layer_pixels(&s, Rect::new(0, 0, 64, 48));
    let (inside, outside): (Vec<_>, Vec<_>) = px.as_chunks::<4>().0.iter().enumerate().partition(|(i, _)| i % 64 < 32);
    assert!(inside.iter().all(|(_, p)| p[3] > 0.999));
    assert!(outside.iter().all(|(_, p)| p[3] == 0.0));
}

fn bounds(r: &Value) -> Vec<i64> {
    r["bounds"].as_array().unwrap_or_else(|| panic!("no bounds in {r}")).iter().map(|v| v.as_i64().unwrap()).collect()
}

#[test]
fn flame_follows_svg_path_data() {
    let mut s = session(160, 120);
    let r = s.execute("filter.render.flame", json!({"path": "M 20 110 L 20 20", "width": 16, "length": 60})).unwrap();
    assert_eq!(r["usedPath"], json!(true), "{r}");
    let b = bounds(&r);
    assert!(b[0] < 20 && b[2] < 60, "flame bounds {b:?} hug the path at x=20");
    // Relative commands, curves and closepath.
    s.execute("edit.undo", json!({})).unwrap();
    let r = s.execute("filter.render.flame", json!({"path": "m100,100 c10,-20 30,-20 40,0 q10,10 0,10 z", "width": 10, "length": 20})).unwrap();
    assert_eq!(r["usedPath"], json!(true), "{r}");
    let b = bounds(&r);
    assert!(b[0] > 70, "flame bounds {b:?} follow the curve at x≥100");
    // A name that is neither a saved path nor path data is an error, not the default arc.
    s.execute("edit.undo", json!({})).unwrap();
    let e = s.execute("filter.render.flame", json!({"path": "No Such Path"})).unwrap_err().to_string();
    assert!(e.contains("No Such Path"), "{e}");
    // A saved path is still found by name.
    s.execute("path.set", json!({"name": "Wick", "path": {"subpaths": [{"closed": false, "knots": [[40, 100], [40, 30]]}]}})).unwrap();
    let r = s.execute("filter.render.flame", json!({"path": "Wick", "width": 12, "length": 40})).unwrap();
    assert_eq!(r["usedPath"], json!(true), "{r}");
    assert!(bounds(&r)[0] < 40);
}

#[test]
fn flame_draws_past_the_old_primitive_cap_and_reports_its_limit() {
    // About 70 flames of about 2,300 primitives each: well past the 65,536 at which Flame used to stop.
    let mut s = session(400, 300);
    let r = s
        .execute(
            "filter.render.flame",
            json!({"flameType": "multipleFlamesAlongPath", "interval": 4, "quality": "fine", "flameLines": 100, "length": 40, "width": 20}),
        )
        .unwrap();
    let prims = r["primitives"].as_u64().unwrap();
    assert!(prims > 65_536, "{prims} primitives");
    assert_eq!(r["truncated"], json!(false), "{r}");
    assert_eq!(r["flames"].as_u64().unwrap(), r["flamesRequested"].as_u64().unwrap(), "{r}");
    // At the limit, the flames that don't fit are counted, not dropped silently.
    let path = render2::default_flame_path(Rect::new(0, 0, 400, 300));
    let spec = FlameSpec { flame_type: FlameType::MultipleFlamesAlongPath, interval: 4.0, quality: 4, flame_lines: 100.0, ..Default::default() };
    let all = render2::flame_limited(&spec, std::slice::from_ref(&path), usize::MAX);
    let cut = render2::flame_limited(&spec, std::slice::from_ref(&path), 10_000);
    assert!(cut.prims.len() <= 10_000 && cut.flames > 0 && cut.flames < all.flames);
    assert_eq!(cut.flames + cut.skipped, all.flames);
    assert_eq!(all.skipped, 0);
}

/// A capsule's ends and its larger radius.
type Capsule = ((f32, f32), (f32, f32), f32);

fn capsules(prims: &[Prim]) -> Vec<Capsule> {
    prims
        .iter()
        .filter_map(|p| match p.shape {
            Shape::Capsule { a, b, ra, rb } => Some((a, b, ra.max(rb))),
            Shape::Ellipse { .. } => None,
        })
        .collect()
}

fn dist(a: (f32, f32), b: (f32, f32)) -> f32 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

#[test]
fn scallops_join_at_the_corners() {
    for (w, h) in [(400, 300), (317, 211), (120, 480)] {
        let canvas = Rect::new(0, 0, w, h);
        let prims = render2::picture_frame(&FrameSpec { frame: 12, ..Default::default() }, canvas);
        let caps = capsules(&prims);
        assert!(caps.len() > 16);
        // One unbroken chain of arcs: every arc end meets another arc's end.
        let ends: Vec<(f32, f32)> = caps.iter().flat_map(|c| [c.0, c.1]).collect();
        for (i, e) in ends.iter().enumerate() {
            let met = ends.iter().enumerate().any(|(j, o)| j / 2 != i / 2 && dist(*e, *o) < 0.5);
            assert!(met, "{w}×{h}: an arc ends alone at {e:?}");
        }
        // The arcs bulge outwards from a frame rectangle, never back into the picture.
        let xs = ends.iter().map(|p| p.0);
        let ys = ends.iter().map(|p| p.1);
        let (x0, x1) = (xs.clone().fold(f32::MAX, f32::min), xs.fold(f32::MIN, f32::max));
        let (y0, y1) = (ys.clone().fold(f32::MAX, f32::min), ys.fold(f32::MIN, f32::max));
        let r = caps.iter().map(|c| dist(c.0, c.1)).fold(0.0, f32::max) * 8.0;
        for p in &ends {
            let inner = (p.0 - x0).min(x1 - p.0).min(p.1 - y0).min(y1 - p.1);
            assert!(inner <= r / 2.0 + 0.5, "{w}×{h}: {p:?} reaches {inner} px into the picture");
        }
    }
}

#[test]
fn spruce_and_fir_needles_grow_on_branches() {
    let names = render2::tree_type_names();
    for name in ["Spruce", "Fir", "Pine", "Cedar"] {
        let kind = names.iter().position(|n| *n == name).unwrap() as u32 + 1;
        for seed in [0, 3] {
            let prims = render2::tree(&TreeSpec { base_tree_type: kind, seed, leaves_amount: 100.0, ..Default::default() }, Rect::new(0, 0, 400, 400));
            // Branches are drawn in one colour; needles shade from base to tip.
            let (branches, needles): (Vec<_>, Vec<_>) = prims.iter().filter(|p| matches!(p.shape, Shape::Capsule { .. })).partition(|p| p.c0 == p.c1);
            let branches = capsules(&branches.into_iter().copied().collect::<Vec<_>>());
            let needles = capsules(&needles.into_iter().copied().collect::<Vec<_>>());
            assert!(needles.len() > 50, "{name}: {} needles", needles.len());
            let off = |p: (f32, f32)| {
                branches
                    .iter()
                    .map(|&(a, b, r)| {
                        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                        let l2 = dx * dx + dy * dy;
                        let t = if l2 > 1e-9 { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
                        dist(p, (a.0 + dx * t, a.1 + dy * t)) - r
                    })
                    .fold(f32::MAX, f32::min)
            };
            let floating: Vec<_> = needles.iter().map(|n| off(n.0)).filter(|d| *d > 1.0).collect();
            assert!(
                floating.is_empty(),
                "{name} (seed {seed}): {} of {} needle tufts float off the branches, up to {} px",
                floating.len(),
                needles.len(),
                floating.iter().fold(0.0f32, |a, b| a.max(*b))
            );
        }
    }
}
