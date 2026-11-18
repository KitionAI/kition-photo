//! SVG path data (the `d` attribute, SVG 1.1 §8.3 and its implementation notes F.6) as a
//! [`Path`]: moveto, lineto (also horizontal and vertical), cubic and quadratic Béziers with
//! their smooth forms, elliptical arcs (as cubics) and closepath, absolute and relative.

use kition_photo_doc::{Knot, Path, Subpath};
use kition_photo_geom::Point;

/// Parses SVG path data. Errors name what is wrong and where (a byte offset).
pub fn parse_svg_path(d: &str) -> Result<Path, String> {
    let mut b = Builder::default();
    let mut lx = Lexer { s: d.as_bytes(), i: 0 };
    let mut cmd: Option<u8> = None;
    loop {
        lx.skip_separators();
        let Some(&c) = lx.s.get(lx.i) else { break };
        let at = lx.i;
        let letter = if c.is_ascii_alphabetic() {
            if !b"MmLlHhVvCcSsQqTtAaZz".contains(&c) {
                return Err(format!("unknown path command `{}` at {at}", c as char));
            }
            lx.i += 1;
            c
        } else {
            // Repeated parameters repeat the command; after a moveto they are linetos.
            match cmd {
                Some(b'M') => b'L',
                Some(b'm') => b'l',
                Some(c) if !matches!(c, b'Z' | b'z') => c,
                _ => return Err(format!("path data must start with a command (M or m), found `{}` at {at}", c as char)),
            }
        };
        if b.knots.is_empty() && b.subpaths.is_empty() && !matches!(letter, b'M' | b'm') {
            return Err(format!("path data must start with M or m, not `{}`", letter as char));
        }
        let rel = letter.is_ascii_lowercase();
        let base = if rel { b.cur } else { Point::new(0.0, 0.0) };
        let pt = |lx: &mut Lexer| -> Result<Point, String> {
            let x = lx.number()?;
            let y = lx.number()?;
            Ok(Point::new(base.x + x, base.y + y))
        };
        match letter.to_ascii_uppercase() {
            b'M' => {
                let p = pt(&mut lx)?;
                b.move_to(p);
            }
            b'L' => {
                let p = pt(&mut lx)?;
                b.line_to(p);
            }
            b'H' => {
                let x = lx.number()? + if rel { b.cur.x } else { 0.0 };
                b.line_to(Point::new(x, b.cur.y));
            }
            b'V' => {
                let y = lx.number()? + if rel { b.cur.y } else { 0.0 };
                b.line_to(Point::new(b.cur.x, y));
            }
            b'C' => {
                let (c1, c2, p) = (pt(&mut lx)?, pt(&mut lx)?, pt(&mut lx)?);
                b.cubic_to(c1, c2, p);
            }
            b'S' => {
                let c1 = b.reflected(b.last_cubic);
                let (c2, p) = (pt(&mut lx)?, pt(&mut lx)?);
                b.cubic_to(c1, c2, p);
            }
            b'Q' => {
                let (q, p) = (pt(&mut lx)?, pt(&mut lx)?);
                b.quad_to(q, p);
            }
            b'T' => {
                let q = b.reflected(b.last_quad);
                let p = pt(&mut lx)?;
                b.quad_to(q, p);
            }
            b'A' => {
                let (rx, ry, rot) = (lx.number()?, lx.number()?, lx.number()?);
                let (large, sweep) = (lx.flag()?, lx.flag()?);
                let p = pt(&mut lx)?;
                b.arc_to(rx, ry, rot, large, sweep, p);
            }
            _ => b.close(),
        }
        cmd = Some(letter);
    }
    b.finish();
    if b.subpaths.is_empty() {
        return Err("path data draws nothing".into());
    }
    Ok(Path { subpaths: b.subpaths, ..Default::default() })
}

struct Lexer<'a> {
    s: &'a [u8],
    i: usize,
}

impl Lexer<'_> {
    fn skip_separators(&mut self) {
        while self.s.get(self.i).is_some_and(|c| c.is_ascii_whitespace() || *c == b',') {
            self.i += 1;
        }
    }

    fn number(&mut self) -> Result<f64, String> {
        self.skip_separators();
        let start = self.i;
        let digits = |lx: &mut Lexer| {
            let from = lx.i;
            while lx.s.get(lx.i).is_some_and(u8::is_ascii_digit) {
                lx.i += 1;
            }
            lx.i > from
        };
        if self.s.get(self.i).is_some_and(|c| matches!(c, b'+' | b'-')) {
            self.i += 1;
        }
        let mut any = digits(self);
        if self.s.get(self.i) == Some(&b'.') {
            self.i += 1;
            any |= digits(self);
        }
        if !any {
            self.i = start;
            return Err(format!("expected a number at {start}"));
        }
        if self.s.get(self.i).is_some_and(|c| matches!(c, b'e' | b'E')) {
            let mark = self.i;
            self.i += 1;
            if self.s.get(self.i).is_some_and(|c| matches!(c, b'+' | b'-')) {
                self.i += 1;
            }
            if !digits(self) {
                self.i = mark;
            }
        }
        let text = std::str::from_utf8(self.s.get(start..self.i).unwrap_or_default()).unwrap_or_default();
        text.parse::<f64>().ok().filter(|v| v.is_finite() && v.abs() <= 1.0e9).ok_or_else(|| format!("number `{text}` at {start} is out of range"))
    }

    /// An arc flag: `0` or `1`, which needs no separator from what follows.
    fn flag(&mut self) -> Result<bool, String> {
        self.skip_separators();
        match self.s.get(self.i) {
            Some(b'0') => {
                self.i += 1;
                Ok(false)
            }
            Some(b'1') => {
                self.i += 1;
                Ok(true)
            }
            _ => Err(format!("expected an arc flag (0 or 1) at {}", self.i)),
        }
    }
}

#[derive(Default)]
struct Builder {
    subpaths: Vec<Subpath>,
    knots: Vec<Knot>,
    cur: Point,
    start: Point,
    /// The second control point of the last cubic (for `S`), the control point of the last
    /// quadratic (for `T`); `None` when the previous command was not of that kind.
    last_cubic: Option<Point>,
    last_quad: Option<Point>,
    /// The last command was closepath: the next drawing command starts at `start`.
    closed: bool,
}

impl Builder {
    fn flush(&mut self, closed: bool) {
        if !self.knots.is_empty() {
            self.subpaths.push(Subpath { closed, knots: std::mem::take(&mut self.knots), ..Default::default() });
        }
    }

    fn move_to(&mut self, p: Point) {
        self.flush(false);
        self.closed = false;
        self.knots.push(Knot::corner(p.x, p.y));
        self.cur = p;
        self.start = p;
        self.last_cubic = None;
        self.last_quad = None;
    }

    /// The knot the next segment leaves from (a new subpath at the start after a closepath).
    fn from(&mut self) -> Option<&mut Knot> {
        if self.closed || self.knots.is_empty() {
            self.closed = false;
            let s = self.start;
            self.knots.push(Knot::corner(s.x, s.y));
            self.cur = s;
        }
        self.knots.last_mut()
    }

    fn line_to(&mut self, p: Point) {
        self.from();
        self.knots.push(Knot::corner(p.x, p.y));
        self.cur = p;
        self.last_cubic = None;
        self.last_quad = None;
    }

    fn cubic_to(&mut self, c1: Point, c2: Point, p: Point) {
        if let Some(k) = self.from() {
            k.out_ctrl = c1;
        }
        self.knots.push(Knot { anchor: p, in_ctrl: c2, out_ctrl: p, smooth: false });
        self.cur = p;
        self.last_cubic = Some(c2);
        self.last_quad = None;
    }

    fn quad_to(&mut self, q: Point, p: Point) {
        self.from();
        let c = self.cur;
        let c1 = Point::new(c.x + (q.x - c.x) * 2.0 / 3.0, c.y + (q.y - c.y) * 2.0 / 3.0);
        let c2 = Point::new(p.x + (q.x - p.x) * 2.0 / 3.0, p.y + (q.y - p.y) * 2.0 / 3.0);
        self.cubic_to(c1, c2, p);
        self.last_cubic = None;
        self.last_quad = Some(q);
    }

    /// The reflection of `ctrl` about the current point (the current point without one).
    fn reflected(&self, ctrl: Option<Point>) -> Point {
        let c = self.cur;
        ctrl.map_or(c, |k| Point::new(2.0 * c.x - k.x, 2.0 * c.y - k.y))
    }

    /// Endpoint arc → centre form (SVG F.6.5), drawn as cubics of at most 90° each.
    fn arc_to(&mut self, rx: f64, ry: f64, rotation: f64, large: bool, sweep: bool, p: Point) {
        let p0 = self.cur;
        let (mut rx, mut ry) = (rx.abs(), ry.abs());
        if rx < 1e-9 || ry < 1e-9 || (p0.x - p.x).hypot(p0.y - p.y) < 1e-9 {
            if (p0.x - p.x).hypot(p0.y - p.y) >= 1e-9 {
                self.line_to(p);
            }
            return;
        }
        let (sin, cos) = rotation.to_radians().sin_cos();
        let (dx, dy) = ((p0.x - p.x) / 2.0, (p0.y - p.y) / 2.0);
        let (x1, y1) = (cos * dx + sin * dy, -sin * dx + cos * dy);
        let lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
        if lambda > 1.0 {
            rx *= lambda.sqrt();
            ry *= lambda.sqrt();
        }
        let num = (rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1).max(0.0);
        let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
        let mut k = if den > 0.0 { (num / den).sqrt() } else { 0.0 };
        if large == sweep {
            k = -k;
        }
        let (cx1, cy1) = (k * rx * y1 / ry, -k * ry * x1 / rx);
        let (cx, cy) = (cos * cx1 - sin * cy1 + (p0.x + p.x) / 2.0, sin * cx1 + cos * cy1 + (p0.y + p.y) / 2.0);
        let angle = |ux: f64, uy: f64, vx: f64, vy: f64| {
            let a = (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
            if a.is_finite() { a } else { 0.0 }
        };
        let (ux, uy) = ((x1 - cx1) / rx, (y1 - cy1) / ry);
        let (vx, vy) = ((-x1 - cx1) / rx, (-y1 - cy1) / ry);
        let theta = angle(1.0, 0.0, ux, uy);
        let mut delta = angle(ux, uy, vx, vy);
        let tau = std::f64::consts::TAU;
        if !sweep && delta > 0.0 {
            delta -= tau;
        } else if sweep && delta < 0.0 {
            delta += tau;
        }
        let pieces = (delta.abs() / std::f64::consts::FRAC_PI_2).ceil().clamp(1.0, 4.0) as usize;
        let step = delta / pieces as f64;
        let handle = 4.0 / 3.0 * (step / 4.0).tan();
        let at = |a: f64| {
            let (s, c) = a.sin_cos();
            (
                Point::new(cx + rx * c * cos - ry * s * sin, cy + rx * c * sin + ry * s * cos),
                Point::new(-rx * s * cos - ry * c * sin, -rx * s * sin + ry * c * cos),
            )
        };
        for j in 0..pieces {
            let (a0, a1) = (theta + step * j as f64, theta + step * (j + 1) as f64);
            let ((q0, d0), (q1, d1)) = (at(a0), at(a1));
            let end = if j + 1 == pieces { p } else { q1 };
            self.cubic_to(Point::new(q0.x + d0.x * handle, q0.y + d0.y * handle), Point::new(q1.x - d1.x * handle, q1.y - d1.y * handle), end);
        }
        self.last_cubic = None;
    }

    fn close(&mut self) {
        if self.closed || self.knots.is_empty() {
            return;
        }
        // A last knot on the start point is the start knot: the closing segment's curve arrives there.
        if self.knots.len() > 1
            && let (Some(first), Some(last)) = (self.knots.first(), self.knots.last())
            && (first.anchor.x - last.anchor.x).abs() < 1e-9
            && (first.anchor.y - last.anchor.y).abs() < 1e-9
        {
            let in_ctrl = last.in_ctrl;
            self.knots.pop();
            if let Some(first) = self.knots.first_mut() {
                first.in_ctrl = in_ctrl;
            }
        }
        self.flush(true);
        self.closed = true;
        self.cur = self.start;
        self.last_cubic = None;
        self.last_quad = None;
    }

    fn finish(&mut self) {
        self.flush(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anchors(p: &Path) -> Vec<Vec<(f64, f64)>> {
        p.subpaths.iter().map(|s| s.knots.iter().map(|k| (k.anchor.x, k.anchor.y)).collect()).collect()
    }

    #[test]
    fn lines_relative_and_implicit_commands() {
        let p = parse_svg_path("M10,20 30 20 v10 h-20 z m5 5 l1 1").unwrap();
        assert_eq!(anchors(&p), vec![vec![(10.0, 20.0), (30.0, 20.0), (30.0, 30.0), (10.0, 30.0)], vec![(15.0, 25.0), (16.0, 26.0)]]);
        assert!(p.subpaths[0].closed && !p.subpaths[1].closed);
    }

    #[test]
    fn curves_keep_their_handles() {
        let p = parse_svg_path("M0 0C10 0 20 10 20 20S30 40 40 40Q50 40 50 50T60 60").unwrap();
        let k = &p.subpaths[0].knots;
        assert_eq!((k[0].out_ctrl.x, k[0].out_ctrl.y), (10.0, 0.0));
        assert_eq!((k[1].in_ctrl.x, k[1].in_ctrl.y), (20.0, 10.0));
        // S reflects the previous second control point.
        assert_eq!((k[1].out_ctrl.x, k[1].out_ctrl.y), (20.0, 30.0));
        assert_eq!(k.len(), 5);
    }

    #[test]
    fn arcs_end_on_their_endpoint_and_stay_on_the_circle() {
        let p = parse_svg_path("M0 50 A50 50 0 0 1 100 50").unwrap();
        let k = &p.subpaths[0].knots;
        let last = k.last().unwrap().anchor;
        assert!((last.x - 100.0).abs() < 1e-9 && (last.y - 50.0).abs() < 1e-9);
        for knot in k {
            assert!(((knot.anchor.x - 50.0).hypot(knot.anchor.y - 50.0) - 50.0).abs() < 1e-6);
            // Sweep 1 (clockwise on screen) from the left goes over the top.
            assert!(knot.anchor.y <= 50.0 + 1e-9);
        }
        // Flags need no separators.
        assert!(parse_svg_path("M0 0a5 5 0 1010 0").is_ok());
    }

    #[test]
    fn bad_data_is_an_error() {
        for d in ["", "L 1 2", "M 1", "M 1 2 X", "Wick", "M 1 2 A 1 1 0 2 0 3 3", "M1e999 0"] {
            assert!(parse_svg_path(d).is_err(), "{d:?}");
        }
    }
}
