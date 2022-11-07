//! Convert an SVG file to a list of polylines (aka polygonal chains or polygonal
//! paths).
//!
//! This can be used e.g. for simple drawing robot that just support drawing
//! straight lines and liftoff / drop pen commands.
//!
//! Flattening of Bézier curves is done using the
//! [Lyon](https://github.com/nical/lyon) library. SVG files are preprocessed /
//! simplified using [usvg](https://docs.rs/usvg/).
//!
//! **Note: Currently the path style is completely ignored. Only the path itself is
//! returned.**
//!
//! ## MSRV
//!
//! This library does not guarantee a fixed MSRV.
//!
//! ## Serialization
//!
//! You can optionally get serde 1 support by enabling the `serde` feature.

#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::single_match)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::too_many_lines)]

use std::{
    convert::{From, TryInto},
    f64, mem,
    ops::Index,
    str,
};

use log::trace;
use lyon_geom::{
    euclid::{Point2D, Transform2D},
    CubicBezierSegment, QuadraticBezierSegment,
};
use quick_xml::events::Event;
use svgtypes::{PathParser, PathSegment};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

mod error;

pub use error::Error;

/// A pair of x and y coordinates.
#[derive(Debug, PartialEq, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(C)]
pub struct CoordinatePair {
    pub x: f64,
    pub y: f64,
}

impl CoordinatePair {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Apply a 2D transformation.
    pub fn transform(&mut self, t: Transform2D<f64, f64, f64>) {
        let Point2D { x, y, .. } = t.transform_point(Point2D::new(self.x, self.y));
        self.x = x;
        self.y = y;
    }
}

impl From<(f64, f64)> for CoordinatePair {
    fn from(val: (f64, f64)) -> Self {
        Self { x: val.0, y: val.1 }
    }
}

/// A polyline is a vector of [`CoordinatePair`] instances.
///
/// Note: This is a newtype around a [`Vec`] that can be iterated and indexed.
/// To get access to the underlying vector, use [`.as_ref()`](Polyline::as_ref)
/// or [`.unwrap()`](Polyline::unwrap).
#[repr(transparent)]
#[derive(Debug, PartialEq)]
pub struct Polyline(Vec<CoordinatePair>);

impl Polyline {
    /// Create a new, empty polyline.
    pub fn new() -> Self {
        Polyline(vec![])
    }

    /// Create a new polyline from a vector.
    pub fn from_vec(vec: Vec<CoordinatePair>) -> Self {
        Polyline(vec)
    }

    /// Apply a transformation to all coordinate pairs
    fn transform(mut self, t: Transform2D<f64, f64, f64>) -> Self {
        for p in &mut self.0 {
            p.transform(t);
        }
        self
    }

    /// Unwrap and return the inner vector.
    #[must_use]
    pub fn unwrap(self) -> Vec<CoordinatePair> {
        self.0
    }
}

impl AsRef<Vec<CoordinatePair>> for Polyline {
    fn as_ref(&self) -> &Vec<CoordinatePair> {
        &self.0
    }
}

impl Default for Polyline {
    fn default() -> Self {
        Self::new()
    }
}

impl Index<usize> for Polyline {
    type Output = CoordinatePair;

    fn index(&self, id: usize) -> &Self::Output {
        &self.0[id]
    }
}

impl IntoIterator for Polyline {
    type Item = CoordinatePair;
    type IntoIter = std::vec::IntoIter<Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a Polyline {
    type Item = &'a CoordinatePair;
    type IntoIter = std::slice::Iter<'a, CoordinatePair>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl std::ops::Deref for Polyline {
    type Target = Vec<CoordinatePair>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Polyline {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug, PartialEq)]
struct CurrentLine {
    /// The polyline containing the coordinate pairs for the current line.
    line: Polyline,

    /// This is set to the start coordinates of the previous polyline if the
    /// path expression contains multiple polylines.
    prev_end: Option<CoordinatePair>,
}

/// Simple data structure that acts as a [`Polyline`] buffer.
impl CurrentLine {
    fn new() -> Self {
        Self {
            line: Polyline::new(),
            prev_end: None,
        }
    }

    /// Add a [`CoordinatePair`] to the internal polyline.
    fn add_absolute(&mut self, pair: CoordinatePair) {
        self.line.push(pair);
    }

    /// Add a relative [`CoordinatePair`] to the internal polyline.
    fn add_relative(&mut self, pair: CoordinatePair) {
        if let Some(last) = self.line.last() {
            let cp = CoordinatePair::new(last.x + pair.x, last.y + pair.y);
            self.add_absolute(cp);
        } else if let Some(last) = self.prev_end {
            self.add_absolute(CoordinatePair::new(last.x + pair.x, last.y + pair.y));
        } else {
            self.add_absolute(pair);
        }
    }

    /// Add a [`CoordinatePair`] to the internal polyline.
    fn add(&mut self, abs: bool, pair: CoordinatePair) {
        if abs {
            self.add_absolute(pair);
        } else {
            self.add_relative(pair);
        }
    }

    /// A polyline is only valid if it has more than 1 [`CoordinatePair`].
    fn is_valid(&self) -> bool {
        self.line.len() > 1
    }

    /// Return the last [`CoordinatePair`] (if the line is not empty).
    fn last_pair(&self) -> Option<CoordinatePair> {
        self.line.last().copied()
    }

    /// Return the last x coordinate (if the line is not empty).
    fn last_x(&self) -> Option<f64> {
        self.line.last().map(|pair| pair.x)
    }

    /// Return the last y coordinate (if the line is not empty).
    fn last_y(&self) -> Option<f64> {
        self.line.last().map(|pair| pair.y)
    }

    /// Close the line by adding the first entry to the end.
    fn close(&mut self) -> Result<(), Error> {
        if self.line.len() < 2 {
            Err(Error::Polyline(
                "Lines with less than 2 coordinate pairs cannot be closed.".into(),
            ))
        } else {
            let first = self.line[0];
            self.line.push(first);
            self.prev_end = Some(first);
            Ok(())
        }
    }

    /// Replace the internal [`Polyline`] with a new instance and return the
    /// previously stored [`Polyline`].
    fn finish(&mut self) -> Polyline {
        self.prev_end = self.line.last().copied();
        let mut tmp = Polyline::new();
        mem::swap(&mut self.line, &mut tmp);
        tmp
    }
}

/// Parse an SVG string, return vector of `(path expression, transform
/// expression)` tuples.
fn parse_xml(svg: &str) -> Result<Vec<(String, Option<String>)>, Error> {
    trace!("parse_xml");

    let mut reader = quick_xml::Reader::from_str(svg);
    reader.trim_text(true);

    let mut paths = Vec::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                trace!("parse_xml: Matched start of {:?}", e.name());
                match e.name() {
                    b"path" => {
                        trace!("parse_xml: Found path element");
                        let mut path_expr: Option<String> = None;
                        let mut transform_expr: Option<String> = None;
                        for attr in e.attributes().filter_map(Result::ok) {
                            let extract = || {
                                attr.unescaped_value()
                                    .ok()
                                    .and_then(|v| str::from_utf8(&v).map(str::to_string).ok())
                            };
                            match attr.key {
                                b"d" => path_expr = extract(),
                                b"transform" => transform_expr = extract(),
                                _ => {}
                            }
                        }
                        if let Some(expr) = path_expr {
                            paths.push((expr, transform_expr));
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => {
                trace!("parse_xml: EOF");
                break;
            }
            Ok(_) => {}
            Err(e) => return Err(Error::SvgParse(e.to_string())),
        }

        // If we don't keep a borrow elsewhere, we can clear the buffer to keep memory usage low
        buf.clear();
    }
    trace!("parse_xml: Return {} paths", paths.len());
    Ok(paths)
}

fn parse_path(expr: &str, tol: f64) -> Result<Vec<Polyline>, Error> {
    trace!("parse_path");
    let mut lines = Vec::new();
    let mut line = CurrentLine::new();

    // Process segments in path expression
    let mut prev_segment_store: Option<PathSegment> = None;
    for segment in PathParser::from(expr) {
        let current_segment = segment.map_err(|e| Error::PathParse(e.to_string()))?;
        let prev_segment = prev_segment_store.replace(current_segment);
        parse_path_segment(&current_segment, prev_segment, &mut line, tol, &mut lines)?;
    }

    // Path parsing is done, add previously parsing line if valid
    if line.is_valid() {
        lines.push(line.finish());
    }

    Ok(lines)
}

/// Helper method for parsing both `CurveTo` and `SmoothCurveTo`.
#[allow(clippy::too_many_arguments)]
fn _handle_cubic_curve(
    current_line: &mut CurrentLine,
    tol: f64,
    abs: bool,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    x: f64,
    y: f64,
) -> Result<(), Error> {
    let current = current_line.last_pair().ok_or_else(|| {
        Error::PathParse("Invalid state: CurveTo or SmoothCurveTo on empty CurrentLine".to_string())
    })?;
    let curve = if abs {
        CubicBezierSegment {
            from: Point2D::new(current.x, current.y),
            ctrl1: Point2D::new(x1, y1),
            ctrl2: Point2D::new(x2, y2),
            to: Point2D::new(x, y),
        }
    } else {
        CubicBezierSegment {
            from: Point2D::new(current.x, current.y),
            ctrl1: Point2D::new(current.x + x1, current.y + y1),
            ctrl2: Point2D::new(current.x + x2, current.y + y2),
            to: Point2D::new(current.x + x, current.y + y),
        }
    };
    for point in curve.flattened(tol) {
        current_line.add_absolute(CoordinatePair::new(point.x, point.y));
    }
    Ok(())
}

#[allow(clippy::similar_names)]
fn parse_path_segment(
    segment: &PathSegment,
    prev_segment: Option<PathSegment>,
    current_line: &mut CurrentLine,
    tol: f64,
    lines: &mut Vec<Polyline>,
) -> Result<(), Error> {
    trace!("parse_path_segment");
    #[allow(clippy::match_wildcard_for_single_variants)]
    match segment {
        &PathSegment::MoveTo { abs, x, y } => {
            trace!("parse_path_segment: MoveTo");
            if current_line.is_valid() {
                lines.push(current_line.finish());
            }
            current_line.add(abs, CoordinatePair::new(x, y));
        }
        &PathSegment::LineTo { abs, x, y } => {
            trace!("parse_path_segment: LineTo");
            current_line.add(abs, CoordinatePair::new(x, y));
        }
        &PathSegment::HorizontalLineTo { abs, x } => {
            trace!("parse_path_segment: HorizontalLineTo");
            match (current_line.last_y(), abs) {
                (Some(y), true) => current_line.add_absolute(CoordinatePair::new(x, y)),
                (Some(_), false) => current_line.add_relative(CoordinatePair::new(x, 0.0)),
                (None, _) => {
                    return Err(Error::PathParse(
                        "Invalid state: HorizontalLineTo on emtpy CurrentLine".into(),
                    ))
                }
            }
        }
        &PathSegment::VerticalLineTo { abs, y } => {
            trace!("parse_path_segment: VerticalLineTo");
            match (current_line.last_x(), abs) {
                (Some(x), true) => current_line.add_absolute(CoordinatePair::new(x, y)),
                (Some(_), false) => current_line.add_relative(CoordinatePair::new(0.0, y)),
                (None, _) => {
                    return Err(Error::PathParse(
                        "Invalid state: VerticalLineTo on emtpy CurrentLine".into(),
                    ))
                }
            }
        }
        &PathSegment::CurveTo {
            abs,
            x1,
            y1,
            x2,
            y2,
            x,
            y,
        } => {
            trace!("parse_path_segment: CurveTo");
            _handle_cubic_curve(current_line, tol, abs, x1, y1, x2, y2, x, y)?;
        }
        &PathSegment::SmoothCurveTo { abs, x2, y2, x, y } => {
            trace!("parse_path_segment: SmoothCurveTo");

            // Who on earth thought it would be a good idea to add a shortcut
            // for curves with a mirrored control point? It generally makes
            // implementations much more complex, while the data is perfectly
            // equivalent to a fully written-out cubic curve m(
            match prev_segment {
                Some(PathSegment::CurveTo {
                    x2: prev_x2,
                    y2: prev_y2,
                    x: prev_x,
                    y: prev_y,
                    ..
                })
                | Some(PathSegment::SmoothCurveTo {
                    x2: prev_x2,
                    y2: prev_y2,
                    x: prev_x,
                    y: prev_y,
                    ..
                }) => {
                    // We have a previous curve. Mirror the previous control
                    // point 2 along the previous end point.
                    let dx = prev_x - prev_x2;
                    let dy = prev_y - prev_y2;
                    let (x1, y1) = if abs {
                        let current = current_line.last_pair().ok_or_else(|| {
                            Error::PathParse(
                                "Invalid state: CurveTo or SmoothCurveTo on empty CurrentLine"
                                    .into(),
                            )
                        })?;
                        (current.x + dx, current.y + dy)
                    } else {
                        (dx, dy)
                    };
                    _handle_cubic_curve(current_line, tol, abs, x1, y1, x2, y2, x, y)?;
                }
                Some(_) | None => {
                    // The previous segment was not a curve. Use the current
                    // point as reference.
                    match current_line.last_pair() {
                        Some(pair) => {
                            let x1 = pair.x;
                            let y1 = pair.y;
                            _handle_cubic_curve(current_line, tol, abs, x1, y1, x2, y2, x, y)?;
                        }
                        None => {
                            return Err(Error::PathParse(
                                "Invalid state: SmoothCurveTo without a reference point".into(),
                            ))
                        }
                    }
                }
            }
        }
        &PathSegment::Quadratic { abs, x1, y1, x, y } => {
            trace!("parse_path_segment: Quadratic");
            let current = current_line.last_pair().ok_or_else(|| {
                Error::PathParse("Invalid state: Quadratic on empty CurrentLine".into())
            })?;
            let curve = if abs {
                QuadraticBezierSegment {
                    from: Point2D::new(current.x, current.y),
                    ctrl: Point2D::new(x1, y1),
                    to: Point2D::new(x, y),
                }
            } else {
                QuadraticBezierSegment {
                    from: Point2D::new(current.x, current.y),
                    ctrl: Point2D::new(current.x + x1, current.y + y1),
                    to: Point2D::new(current.x + x, current.y + y),
                }
            };
            for point in curve.flattened(tol) {
                current_line.add_absolute(CoordinatePair::new(point.x, point.y));
            }
        }
        &PathSegment::ClosePath { .. } => {
            trace!("parse_path_segment: ClosePath");
            current_line
                .close()
                .map_err(|e| Error::PathParse(format!("Invalid state: {}", e)))?;
        }
        &PathSegment::EllipticalArc {
            abs,
            rx,
            ry,
            x_axis_rotation,
            large_arc,
            sweep,
            x,
            y,
        } => {
            // The following code and comments are based on this project:
            // https://github.com/BigBadaboom/androidsvg (Apache-2 license)
            // And more specifically here:
            // https://github.com/BigBadaboom/androidsvg/blob/1ad1c08c4f7ee09fcdd3dca31f8f31db7cacd3b0/androidsvg/src/main/java/com/caverock/androidsvg/utils/SVGAndroidRenderer.java#L2874-L3021
            //
            // This code in turn is partially based on the Batik library
            // (Apache-2 license).

            // SVG arc representation uses "endpoint parameterization" where we
            // specify the start and endpoint of the arc. This is to be
            // consistent with the other path commands. However we need to
            // convert this to "centre point parameterization" in order to
            // calculate the arc. Handily, the SVG spec provides all the
            // required maths in section "F.6 Elliptical arc implementation
            // notes".
            trace!("parse_path_segment: EllipticalArc");
            let current = current_line.last_pair().ok_or_else(|| {
                Error::PathParse("Invalid state: EllipticalArc on empty CurrentLine".into())
            })?;
            let last_x = current.x;
            let last_y = current.y;

            // Calculating the end points of the curve based on the abs flag
            let x_end = if abs { x } else { current.x + x };
            let y_end = if abs { y } else { current.y + y };

            // If the endpoints (x, y) and (x0, y0) are identical, then this is
            // equivalent to omitting the elliptical arc segment entirely.
            // (behavior specified by the spec)
            let error_margin = f64::EPSILON;
            if (last_x - x_end).abs() < error_margin && (last_y - y_end).abs() < error_margin {
                return Ok(());
            }

            // Handle degenerate case (behavior specified by the spec)
            if rx == 0.0 || ry == 0.0 {
                current_line.add(abs, CoordinatePair::new(x_end, y_end));
                return Ok(());
            }

            // Sign of the radii is ignored (behavior specified by the spec)
            let mut rx = rx.abs();
            let mut ry = ry.abs();

            // Convert angle from degrees to radians
            let angle_rad = (x_axis_rotation % 360.0) * (f64::consts::PI / 180.0);
            let cos_angle = angle_rad.cos();
            let sin_angle = angle_rad.sin();

            // We simplify the calculations by transforming the arc so that the origin is at the
            // midpoint calculated above followed by a rotation to line up the coordinate axes
            // with the axes of the ellipse.

            // Compute the midpoint of the line between the current and the end point
            let dx2 = (last_x - x_end) / 2.0;
            let dy2 = (last_y - y_end) / 2.0;

            // Step 1: Compute (x1', y1')
            // x1,y1 is the midpoint vector rotated to take the arc's angle out of consideration
            let x1 = cos_angle * dx2 + sin_angle * dy2;
            let y1 = -sin_angle * dx2 + cos_angle * dy2;

            let mut rx_sq = rx * rx;
            let mut ry_sq = ry * ry;
            let x1_sq = x1 * x1;
            let y1_sq = y1 * y1;

            // Check that radii are large enough.
            // If they are not, the spec says to scale them up so they are.
            // This is to compensate for potential rounding errors/differences between SVG implementations.
            let radii_check = x1_sq / rx_sq + y1_sq / ry_sq;
            if radii_check > 0.99999 {
                let radii_scale = radii_check.sqrt() * 1.00001;
                rx *= radii_scale;
                ry *= radii_scale;
                rx_sq = rx * rx;
                ry_sq = ry * ry;
            }

            // Step 2 : Compute (cx1, cy1) - the transformed centre point
            let mut sign = if large_arc == sweep { -1.0 } else { 1.0 };
            let sq = ((rx_sq * ry_sq) - (rx_sq * y1_sq) - (ry_sq * x1_sq))
                / ((rx_sq * y1_sq) + (ry_sq * x1_sq));
            let sq = if sq < 0.0 { 0.0 } else { sq };
            let coef = sign * sq.sqrt();
            let cx1 = coef * ((rx * y1) / ry);
            let cy1 = coef * -((ry * x1) / rx);

            // Step 3 : Compute (cx, cy) from (cx1, cy1)
            let sx2 = (last_x + x_end) / 2.0;
            let sy2 = (last_y + y_end) / 2.0;
            let cx = sx2 + (cos_angle * cx1 - sin_angle * cy1);
            let cy = sy2 + (sin_angle * cx1 + cos_angle * cy1);

            // Step 4 : Compute the angleStart (angle1) and the angleExtent (dangle)
            let ux = (x1 - cx1) / rx;
            let uy = (y1 - cy1) / ry;
            let vx = (-x1 - cx1) / rx;
            let vy = (-y1 - cy1) / ry;

            // Angle betwen two vectors is +/- acos( u.v / len(u) * len(v))
            // Where '.' is the dot product. And +/- is calculated from the sign of the cross product (u x v)

            // Compute the start angle
            // The angle between (ux,uy) and the 0deg angle (1,0)
            let mut n = ((ux * ux) + (uy * uy)).sqrt(); // len(u) * len(1,0) == len(u)
            let mut p = ux; // u.v == (ux,uy).(1,0) == (1 * ux) + (0 * uy) == ux
            sign = if uy < 0.0 { -1.0 } else { 1.0 }; // u x v == (1 * uy - ux * 0) == uy
            let mut angle_start = sign * (p / n).acos(); // No need for checking the acos here. (p >= n) should always be true.

            // Compute the angle extent
            n = ((ux * ux + uy * uy) * (vx * vx + vy * vy)).sqrt();
            p = ux * vx + uy * vy;
            sign = if (ux * vy - uy * vx) < 0.0 { -1.0 } else { 1.0 };

            let val = p / n;

            let checked_arc_cos = if val < -1.0 {
                f64::consts::PI
            } else if val > 1.0 {
                0.0
            } else {
                val.acos()
            };
            let mut angle_extent = sign * checked_arc_cos;

            // Catch angleExtents of 0, which will cause problems later in arcToBeziers
            if angle_extent == 0.0 {
                current_line.add(abs, CoordinatePair::new(x_end, y_end));
                return Ok(());
            }

            let two_pi = f64::consts::PI * 2.0;
            if !sweep && angle_extent > 0.0 {
                angle_extent -= two_pi;
            } else if sweep && angle_extent < 0.0 {
                angle_extent += two_pi;
            }
            angle_extent %= two_pi;
            angle_start %= two_pi;

            // Many elliptical arc implementations including the Java2D and Android ones, only
            // support arcs that are axis aligned. Therefore we need to substitute the arc
            // with bezier curves. The following function call will generate the beziers for
            // a unit circle that covers the arc angles we want.

            // The following code generates the control points and endpoints for a set of bezier
            // curves that match a circular arc starting from angle 'angleStart' and sweep
            // the angle 'angleExtent'.
            // The circle the arc follows will be centred on (0,0) and have a radius of 1.0.
            //
            // Each bezier can cover no more than 90 degrees, so the arc will be divided evenly
            // into a maximum of four curves.
            //
            // The resulting control points will later be scaled and rotated to match the final
            // arc required.

            #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
            let num_segments = (angle_extent.abs() * 2.0 / f64::consts::PI).ceil() as u64;

            #[allow(clippy::cast_precision_loss)] // Cannot happen
            let angle_increment: f64 = angle_extent / num_segments as f64;

            // The length of each control point vector is given by the following formula.
            let control_length =
                4.0 / 3.0 * (angle_increment / 2.0).sin() / (1.0 + (angle_increment / 2.0).cos());

            let num_segments_usize: usize = num_segments.try_into().unwrap();
            let mut bezier_points = Vec::with_capacity(num_segments_usize * 3);
            for i in 0..num_segments {
                #[allow(clippy::cast_precision_loss)] // Cannot happen
                let mut angle = angle_start + i as f64 * angle_increment;
                // Calculate the control vector at this angle
                let mut dx = angle.cos();
                let mut dy = angle.sin();

                // First control point
                bezier_points.push((dx - control_length * dy, dy + control_length * dx));

                // Second control point
                angle += angle_increment;
                dx = angle.cos();
                dy = angle.sin();
                bezier_points.push((dx + control_length * dy, dy - control_length * dx));

                // Endpoint of bezier
                bezier_points.push((dx, dy));
            }

            // Check if no points were generated
            let len = bezier_points.len();
            if len == 0 {
                return Ok(());
            }

            // Calculate a transformation matrix that will move and scale these bezier points to the correct location.
            let mut bezier_points: Vec<(f64, f64)> = bezier_points
                .into_iter()
                // Scale
                .map(|(a, b)| (a * rx, b * ry))
                // Rotate around the calculated centre point
                .map(|(a, b)| {
                    let s = angle_rad.sin();
                    let c = angle_rad.cos();

                    let px = a - cx1;
                    let py = b - cy1;

                    let x_new = px * c - py * s;
                    let y_new = px * s + py * c;

                    (x_new + cx1, y_new + cy1)
                })
                // Translate
                .map(|(a, b)| (a + cx, b + cy))
                .collect();

            // The last point in the bezier set should match exactly the last coord pair in the arc (ie: x,y). But
            // considering all the mathematical manipulation we have been doing, it is bound to be off by a tiny
            // fraction. Experiments show that it can be up to around 0.00002. So why don't we just set it to
            // exactly what it ought to be.
            bezier_points[len - 1] = (x_end, y_end);

            // Final step is to add the bezier curves to the path
            let mut last_x = last_x;
            let mut last_y = last_y;
            // Step trough points 3 at a time
            for i in (0..bezier_points.len()).step_by(3) {
                let curve = CubicBezierSegment {
                    from: Point2D::new(last_x, last_y),
                    ctrl1: Point2D::new(bezier_points[i].0, bezier_points[i].1),
                    ctrl2: Point2D::new(bezier_points[i + 1].0, bezier_points[i + 1].1),
                    to: Point2D::new(bezier_points[i + 2].0, bezier_points[i + 2].1),
                };
                // End of last curve is used as start point of next curve
                last_x = bezier_points[i + 2].0;
                last_y = bezier_points[i + 2].1;
                for point in curve.flattened(tol) {
                    current_line.add_absolute(CoordinatePair::new(point.x, point.y));
                }
            }
        }
        other => {
            return Err(Error::PathParse(format!(
                "Unsupported path segment: {:?}",
                other
            )));
        }
    }
    Ok(())
}

/// Parse an SVG transformation into a ``Transform2D``.
///
/// Only matrix transformations are supported at the moment. (This shouldn't be
/// an issue, because usvg converts all transformations into matrices.)
#[allow(clippy::many_single_char_names)]
fn parse_transform(transform: &str) -> Result<Transform2D<f64, f64, f64>, Error> {
    // Extract matrix elements from SVG string
    let transform = transform.trim();
    if !transform.starts_with("matrix(") {
        return Err(Error::Transform(format!(
            "Only 'matrix' transform supported in transform '{}'",
            transform
        )));
    }
    if !transform.ends_with(')') {
        return Err(Error::SvgParse(format!(
            "Missing closing parenthesis in transform '{}'",
            transform
        )));
    }
    let matrix = transform
        .strip_prefix("matrix(")
        .expect("checked before")
        .strip_suffix(')')
        .expect("checked to be there");

    // Convert elements to floats
    let elements = matrix
        .split_whitespace()
        .map(str::parse)
        .collect::<Result<Vec<f64>, _>>()
        .map_err(|_| {
            Error::SvgParse(format!(
                "Invalid matrix elements in transform '{}'",
                transform
            ))
        })?;

    // Convert floats into Transform2D
    let [a, b, c, d, e, f]: [f64; 6] = elements.as_slice().try_into().map_err(|_| {
        Error::Transform(format!(
            "Invalid number of matrix elements in transform '{}'",
            transform
        ))
    })?;
    Ok(Transform2D::new(a, b, c, d, e, f))
}

/// Parse an SVG string into a vector of [`Polyline`]s.
///
/// ## Flattening tolerance
///
/// The `tol` parameter controls the flattening tolerance. A large value (e.g.
/// `10.0`) results in very coarse, jagged curves, while a small value (e.g.
/// `0.05`) results in very smooth curves, but a lot of generated polylines.
///
/// Using a value of `0.15` is a good compromise.
///
/// ## Preprocessing
///
/// If `preprocess` is set to `true`,
pub fn parse(svg: &str, tol: f64, preprocess: bool) -> Result<Vec<Polyline>, Error> {
    trace!("parse");

    // Preprocess and simplify the SVG using the usvg library
    let svg = if preprocess {
        let usvg_input_options = usvg::Options::default();
        let usvg_tree = usvg::Tree::from_str(svg, &usvg_input_options.to_ref())?;
        let usvg_xml_options = usvg::XmlOptions::default();
        usvg_tree.to_string(&usvg_xml_options)
    } else {
        svg.to_string()
    };

    // Parse the XML string into a list of path expressions
    let path_exprs = parse_xml(&svg)?;
    trace!("parse: Found {} path expressions", path_exprs.len());

    // Vector that will hold resulting polylines
    let mut polylines: Vec<Polyline> = Vec::new();

    // Process path expressions
    for (path_expr, transform_expr) in path_exprs {
        let path = parse_path(&path_expr, tol)?;
        if let Some(e) = transform_expr {
            let t = parse_transform(&e)?;
            polylines.extend(path.into_iter().map(|polyline| polyline.transform(t)));
        } else {
            polylines.extend(path);
        }
    }

    trace!("parse: This results in {} polylines", polylines.len());
    Ok(polylines)
}

#[cfg(test)]
#[allow(clippy::unreadable_literal)]
mod tests {
    use super::*;

    const FLATTENING_TOLERANCE: f64 = 0.15;

    #[test]
    fn test_current_line() {
        let mut line = CurrentLine::new();
        assert!(!line.is_valid());
        assert_eq!(line.last_x(), None);
        assert_eq!(line.last_y(), None);
        line.add_absolute((1.0, 2.0).into());
        assert!(!line.is_valid());
        assert_eq!(line.last_x(), Some(1.0));
        assert_eq!(line.last_y(), Some(2.0));
        line.add_absolute((2.0, 3.0).into());
        assert!(line.is_valid());
        assert_eq!(line.last_x(), Some(2.0));
        assert_eq!(line.last_y(), Some(3.0));
        let finished = line.finish();
        assert_eq!(finished.len(), 2);
        assert_eq!(finished[0], (1.0, 2.0).into());
        assert_eq!(finished[1], (2.0, 3.0).into());
        assert!(!line.is_valid());
    }

    #[test]
    fn test_current_line_close() {
        let mut line = CurrentLine::new();
        assert_eq!(
            line.close().unwrap_err().to_string(),
            "Polyline error: Lines with less than 2 coordinate pairs cannot be closed.",
        );
        line.add_absolute((1.0, 2.0).into());
        assert_eq!(
            line.close().unwrap_err().to_string(),
            "Polyline error: Lines with less than 2 coordinate pairs cannot be closed.",
        );
        line.add_absolute((2.0, 3.0).into());
        assert!(line.close().is_ok());
        let finished = line.finish();
        assert_eq!(finished.len(), 3);
        assert_eq!(finished[0], (1.0, 2.0).into());
        assert_eq!(finished[2], (1.0, 2.0).into());
    }

    #[test]
    /// Parse segment data with a single `MoveTo` and three coordinates
    fn test_parse_segment_data() {
        let mut current_line = CurrentLine::new();
        let mut lines = Vec::new();
        parse_path_segment(
            &PathSegment::MoveTo {
                abs: true,
                x: 1.0,
                y: 2.0,
            },
            None,
            &mut current_line,
            FLATTENING_TOLERANCE,
            &mut lines,
        )
        .unwrap();
        parse_path_segment(
            &PathSegment::LineTo {
                abs: true,
                x: 2.0,
                y: 3.0,
            },
            None,
            &mut current_line,
            FLATTENING_TOLERANCE,
            &mut lines,
        )
        .unwrap();
        parse_path_segment(
            &PathSegment::LineTo {
                abs: true,
                x: 3.0,
                y: 2.0,
            },
            None,
            &mut current_line,
            FLATTENING_TOLERANCE,
            &mut lines,
        )
        .unwrap();
        assert_eq!(lines.len(), 0);
        let finished = current_line.finish();
        assert_eq!(lines.len(), 0);
        assert_eq!(finished.len(), 3);
        assert_eq!(finished[0], (1.0, 2.0).into());
        assert_eq!(finished[1], (2.0, 3.0).into());
        assert_eq!(finished[2], (3.0, 2.0).into());
    }

    #[test]
    /// Parse segment data with `HorizontalLineTo` / `VerticalLineTo` entries
    fn test_parse_segment_data_horizontal_vertical() {
        let mut current_line = CurrentLine::new();
        let mut lines = Vec::new();
        parse_path_segment(
            &PathSegment::MoveTo {
                abs: true,
                x: 1.0,
                y: 2.0,
            },
            None,
            &mut current_line,
            FLATTENING_TOLERANCE,
            &mut lines,
        )
        .unwrap();
        parse_path_segment(
            &PathSegment::HorizontalLineTo { abs: true, x: 3.0 },
            None,
            &mut current_line,
            FLATTENING_TOLERANCE,
            &mut lines,
        )
        .unwrap();
        parse_path_segment(
            &PathSegment::VerticalLineTo { abs: true, y: -1.0 },
            None,
            &mut current_line,
            FLATTENING_TOLERANCE,
            &mut lines,
        )
        .unwrap();
        assert_eq!(lines.len(), 0);
        let finished = current_line.finish();
        assert_eq!(lines.len(), 0);
        assert_eq!(finished.len(), 3);
        assert_eq!(finished[0], (1.0, 2.0).into());
        assert_eq!(finished[1], (3.0, 2.0).into());
        assert_eq!(finished[2], (3.0, -1.0).into());
    }

    #[test]
    fn test_parse_segment_data_unsupported() {
        let mut current_line = CurrentLine::new();
        let mut lines = Vec::new();
        parse_path_segment(
            &PathSegment::MoveTo {
                abs: true,
                x: 1.0,
                y: 2.0,
            },
            None,
            &mut current_line,
            FLATTENING_TOLERANCE,
            &mut lines,
        )
        .unwrap();
        let result = parse_path_segment(
            &PathSegment::SmoothQuadratic {
                abs: true,
                x: 3.0,
                y: 4.0,
            },
            None,
            &mut current_line,
            FLATTENING_TOLERANCE,
            &mut lines,
        );
        assert!(result.is_err());
        assert_eq!(lines.len(), 0);
        let finished = current_line.finish();
        assert_eq!(finished.len(), 1);
        assert_eq!(finished[0], (1.0, 2.0).into());
    }

    #[test]
    /// Parse segment data with multiple `MoveTo` commands
    fn test_parse_segment_data_multiple() {
        let mut current_line = CurrentLine::new();
        let mut lines = Vec::new();
        parse_path_segment(
            &PathSegment::MoveTo {
                abs: true,
                x: 1.0,
                y: 2.0,
            },
            None,
            &mut current_line,
            FLATTENING_TOLERANCE,
            &mut lines,
        )
        .unwrap();
        parse_path_segment(
            &PathSegment::LineTo {
                abs: true,
                x: 2.0,
                y: 3.0,
            },
            None,
            &mut current_line,
            FLATTENING_TOLERANCE,
            &mut lines,
        )
        .unwrap();
        parse_path_segment(
            &PathSegment::MoveTo {
                abs: true,
                x: 1.0,
                y: 3.0,
            },
            None,
            &mut current_line,
            FLATTENING_TOLERANCE,
            &mut lines,
        )
        .unwrap();
        parse_path_segment(
            &PathSegment::LineTo {
                abs: true,
                x: 2.0,
                y: 4.0,
            },
            None,
            &mut current_line,
            FLATTENING_TOLERANCE,
            &mut lines,
        )
        .unwrap();
        parse_path_segment(
            &PathSegment::MoveTo {
                abs: true,
                x: 1.0,
                y: 4.0,
            },
            None,
            &mut current_line,
            FLATTENING_TOLERANCE,
            &mut lines,
        )
        .unwrap();
        parse_path_segment(
            &PathSegment::LineTo {
                abs: true,
                x: 2.0,
                y: 5.0,
            },
            None,
            &mut current_line,
            FLATTENING_TOLERANCE,
            &mut lines,
        )
        .unwrap();
        parse_path_segment(
            &PathSegment::MoveTo {
                abs: true,
                x: 1.0,
                y: 5.0,
            },
            None,
            &mut current_line,
            FLATTENING_TOLERANCE,
            &mut lines,
        )
        .unwrap();
        assert_eq!(lines.len(), 3);
        assert!(!current_line.is_valid());
        let finished = current_line.finish();
        assert_eq!(finished.len(), 1);
    }

    #[test]
    fn test_parse_simple_absolute_nonclosed() {
        let _ = env_logger::try_init();
        let input = r#"
            <?xml version="1.0" encoding="UTF-8" standalone="no"?>
            <svg xmlns="http://www.w3.org/2000/svg" version="1.1">
                <path d="M 113,35 H 40 L -39,49 H 40" />
            </svg>
        "#
        .trim();
        let result = parse(input, FLATTENING_TOLERANCE, true).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].len(), 4);
        assert_eq!(result[0][0], (113., 35.).into());
        assert_eq!(result[0][1], (40., 35.).into());
        assert_eq!(result[0][2], (-39., 49.).into());
        assert_eq!(result[0][3], (40., 49.).into());
    }

    #[test]
    fn test_parse_simple_absolute_closed() {
        let _ = env_logger::try_init();
        let input = r#"
            <?xml version="1.0" encoding="UTF-8" standalone="no"?>
            <svg xmlns="http://www.w3.org/2000/svg" version="1.1">
                <path d="M 10,10 20,15 10,20 Z" />
            </svg>
        "#
        .trim();
        let result = parse(input, FLATTENING_TOLERANCE, true).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].len(), 4);
        assert_eq!(result[0][0], (10., 10.).into());
        assert_eq!(result[0][1], (20., 15.).into());
        assert_eq!(result[0][2], (10., 20.).into());
        assert_eq!(result[0][3], (10., 10.).into());
    }

    #[cfg(feature = "use_serde")]
    #[test]
    fn test_serde() {
        let cp = CoordinatePair::new(10.0, 20.0);
        let cp_json = serde_json::to_string(&cp).unwrap();
        let cp2 = serde_json::from_str(&cp_json).unwrap();
        assert_eq!(cp, cp2);
    }

    #[test]
    fn test_regression_issue_5() {
        let input = r#"
            <?xml version="1.0" encoding="UTF-8" standalone="no"?>
            <svg xmlns="http://www.w3.org/2000/svg" version="1.1">
                <path d="M 10,10 20,15 10,20 Z m 0,40 H 0" />
            </svg>
        "#
        .trim();
        let result = parse(input, FLATTENING_TOLERANCE, true).unwrap();
        assert_eq!(result.len(), 2);

        assert_eq!(result[0].len(), 4);
        assert_eq!(result[0][0], (10., 10.).into());
        assert_eq!(result[0][1], (20., 15.).into());
        assert_eq!(result[0][2], (10., 20.).into());
        assert_eq!(result[0][3], (10., 10.).into());

        assert_eq!(result[1].len(), 2);
        assert_eq!(result[1][0], (10., 50.).into());
        assert_eq!(result[1][1], (0., 50.).into());
    }

    #[test]
    fn test_regression_issue_7() {
        let _ = env_logger::try_init();
        let input = r#"
            <?xml version="1.0" encoding="UTF-8" standalone="no"?>
            <svg xmlns="http://www.w3.org/2000/svg" version="1.1">
                <path d="M 10,100 40,70 h 10 m -20,40 10,-20" />
            </svg>
        "#
        .trim();
        let result = parse(input, FLATTENING_TOLERANCE, true).unwrap();

        // 2 Polylines
        assert_eq!(result.len(), 2);

        // First line has three points
        assert_eq!(result[0].len(), 3);
        assert_eq!(result[0][0], (10., 100.).into());
        assert_eq!(result[0][1], (40., 70.).into());
        assert_eq!(result[0][2], (50., 70.).into());

        // First line has two points
        assert_eq!(result[1].len(), 2);
        assert_eq!(result[1][0], (30., 110.).into());
        assert_eq!(result[1][1], (40., 90.).into());
    }

    #[test]
    fn test_smooth() {
        let _ = env_logger::try_init();
        let input = r#"
            <?xml version="1.0" encoding="UTF-8" standalone="no"?>
            <svg xmlns="http://www.w3.org/2000/svg" version="1.1">
                <path d="M 10 20 C 10 20 11 17 12 15 S 2 7 10 20 z" />
                <path d="M 10 20 C 10 20 11 17 12 15 s -10 -8 -2 5 z" />
                <path d="M 10 20 c 0 0 1 -3 2 -5 S 2 7 10 20 z" />
                <path d="M 10 20 c 0 0 1 -3 2 -5 s -10 -8 -2 5 z" />
            </svg>
        "#
        .trim();
        let result = parse(input, FLATTENING_TOLERANCE, true).unwrap();
        assert_eq!(result.len(), 4);
        assert_eq!(result[0], result[1]);
        assert_eq!(result[0], result[2]);
        assert_eq!(result[0], result[3]);
    }

    #[test]
    fn test_parse_xml_single() {
        let _ = env_logger::try_init();
        let input = r#"
            <?xml version="1.0" encoding="UTF-8" standalone="no"?>
            <svg xmlns="http://www.w3.org/2000/svg" version="1.1">
                <path d="M 10,100 40,70 h 10 m -20,40 10,-20" />
            </svg>
        "#
        .trim();
        let result = parse_xml(input).unwrap();
        assert_eq!(
            result,
            vec![("M 10,100 40,70 h 10 m -20,40 10,-20".to_string(), None)]
        );
    }

    #[test]
    fn test_parse_xml_multiple() {
        let _ = env_logger::try_init();
        let input = r#"
            <?xml version="1.0" encoding="UTF-8" standalone="no"?>
            <svg xmlns="http://www.w3.org/2000/svg" version="1.1">
                <path d="M 10,100 40,70 h 10 m -20,40 10,-20" />
                <path d="M 20,30" />
            </svg>
        "#
        .trim();
        let result = parse_xml(input).unwrap();
        assert_eq!(
            result,
            vec![
                ("M 10,100 40,70 h 10 m -20,40 10,-20".to_string(), None),
                ("M 20,30".to_string(), None),
            ]
        );
    }

    /// If multiple "d" attributes are found, simply use the first one.
    #[test]
    fn test_parse_xml_duplicate_attr() {
        let _ = env_logger::try_init();
        let input = r#"
            <?xml version="1.0" encoding="UTF-8" standalone="no"?>
            <svg xmlns="http://www.w3.org/2000/svg" version="1.1">
                <path d="M 20,30" d="M 10,100 40,70 h 10 m -20,40 10,-20"/>
            </svg>
        "#
        .trim();
        let result = parse_xml(input).unwrap();
        assert_eq!(result, vec![("M 20,30".to_string(), None)]);
    }

    #[test]
    fn test_parse_xml_with_transform() {
        let _ = env_logger::try_init();
        let input = r#"
            <?xml version="1.0" encoding="UTF-8" standalone="no"?>
            <svg xmlns="http://www.w3.org/2000/svg" version="1.1">
                <path d="M 20,30" transform="matrix(1 0 0 1 0 0)"/>
                <path d="M 30,40"/>
            </svg>
        "#
        .trim();
        let result = parse_xml(input).unwrap();
        assert_eq!(
            result,
            vec![
                (
                    "M 20,30".to_string(),
                    Some("matrix(1 0 0 1 0 0)".to_string())
                ),
                ("M 30,40".to_string(), None)
            ],
        );
    }

    #[test]
    fn test_parse_xml_malformed() {
        let _ = env_logger::try_init();
        let input = r#"
            <svg xmlns="http://www.w3.org/2000/svg" version="1.1">
                <path d="M 20,30" d="M 10,100 40,70 h 10 m -20,40 10,-20"/>
            </baa>
        "#
        .trim();
        let result = parse_xml(input);
        assert_eq!(
            result.unwrap_err().to_string(),
            "SVG parse error: Expecting </svg> found </baa>",
        );
    }

    /// Test the flattening of a quadratic curve.
    ///
    /// Note: This test may break if `lyon_geom` adapts the flattening algorithm.
    /// It should not break otherwise. When in doubt, check an example visually.
    #[test]
    fn test_quadratic_curve() {
        let _ = env_logger::try_init();
        let input = r#"
            <svg xmlns="http://www.w3.org/2000/svg" version="1.1">
                <path d="m 0.10650371,93.221877 c 0,0 3.74188519,-5.078118 9.62198629,-3.474499 5.880103,1.60362 4.276438,7.216278 4.276438,7.216278"/>
            </svg>
        "#.trim();
        let result = parse(input, FLATTENING_TOLERANCE, false).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].len(), 11);
        assert_eq!(
            result[0],
            Polyline(vec![
                CoordinatePair::new(0.10650371, 93.221877),
                CoordinatePair::new(1.294403614814815, 91.96472118518521),
                CoordinatePair::new(2.6361703106158494, 90.93256152046511),
                CoordinatePair::new(4.620522695185185, 89.9354544814815),
                CoordinatePair::new(6.885789998771603, 89.45353374978681),
                CoordinatePair::new(9.72849, 89.74737800000001),
                CoordinatePair::new(12.196509552744402, 90.92131377228664),
                CoordinatePair::new(13.450575259259264, 92.33098488888892),
                CoordinatePair::new(14.083775088013304, 94.01611039126513),
                CoordinatePair::new(14.20291140740741, 95.44912911111113),
                CoordinatePair::new(14.004928, 96.96365600000001),
            ])
        );
    }

    /// Test the flattening of a mirrored cubic curve (also called "smooth
    /// curve").
    ///
    /// Note: This test may break if `lyon_geom` adapts the flattening algorithm.
    /// It should not break otherwise. When in doubt, check an example visually.
    #[test]
    fn test_smooth_curve() {
        let _ = env_logger::try_init();
        let input = r#"
            <svg xmlns="http://www.w3.org/2000/svg" version="1.1">
                <path d="M10 80 C 40 10, 65 10, 95 80 S 150 150, 180 80"/>
            </svg>
        "#
        .trim();
        let result = parse(input, FLATTENING_TOLERANCE, true).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].len(), 39);
        assert_eq!(
            result[0],
            Polyline(vec![
                CoordinatePair::new(10.0, 80.0),
                CoordinatePair::new(15.78100143969477, 67.25459368406422),
                CoordinatePair::new(21.112891508939025, 56.89021833666841),
                CoordinatePair::new(26.03493691503612, 48.59336957163201),
                CoordinatePair::new(30.583422438239403, 42.07406572971166),
                CoordinatePair::new(34.79388507225312, 37.06697733757036),
                CoordinatePair::new(38.70370370370371, 33.333333333333336),
                CoordinatePair::new(42.88612651359071, 30.34239438296855),
                CoordinatePair::new(46.831649509423386, 28.490212691725404),
                CoordinatePair::new(50.627640135655845, 27.608152315837724),
                CoordinatePair::new(54.37235986434414, 27.608152315837728),
                CoordinatePair::new(58.168350490576614, 28.490212691725404),
                CoordinatePair::new(62.113873486409275, 30.342394382968557),
                CoordinatePair::new(66.2962962962963, 33.33333333333333),
                CoordinatePair::new(70.20611492774688, 37.06697733757035),
                CoordinatePair::new(74.41657756176059, 42.07406572971165),
                CoordinatePair::new(78.96506308496389, 48.593369571632),
                CoordinatePair::new(83.88710849106097, 56.89021833666841),
                CoordinatePair::new(89.21899856030524, 67.2545936840642),
                CoordinatePair::new(95.0, 80.0),
                CoordinatePair::new(100.78100143969478, 92.7454063159358),
                CoordinatePair::new(106.112891508939, 103.10978166333157),
                CoordinatePair::new(111.03493691503611, 111.40663042836799),
                CoordinatePair::new(115.58342243823941, 117.92593427028837),
                CoordinatePair::new(119.79388507225313, 122.93302266242966),
                CoordinatePair::new(123.70370370370371, 126.66666666666669),
                CoordinatePair::new(127.88612651359071, 129.65760561703146),
                CoordinatePair::new(131.83164950942339, 131.50978730827458),
                CoordinatePair::new(135.62764013565584, 132.39184768416223),
                CoordinatePair::new(139.37235986434416, 132.3918476841623),
                CoordinatePair::new(143.16835049057661, 131.50978730827458),
                CoordinatePair::new(147.1138734864093, 129.65760561703146),
                CoordinatePair::new(151.2962962962963, 126.66666666666666),
                CoordinatePair::new(155.2061149277469, 122.93302266242966),
                CoordinatePair::new(159.4165775617606, 117.92593427028835),
                CoordinatePair::new(163.9650630849639, 111.40663042836802),
                CoordinatePair::new(168.88710849106099, 103.1097816633316),
                CoordinatePair::new(174.21899856030524, 92.74540631593578),
                CoordinatePair::new(180.0, 80.0),
            ])
        );
    }

    #[test]
    fn test_parse_transform_matrix() {
        // Identity matrix:
        // |1  0  0|
        // |0  1  0|
        // |0  0  1|
        assert_eq!(
            parse_transform("matrix(1 0 0 1 0 0)").unwrap(),
            Transform2D::identity()
        );

        // Scaling matrix (expand in X, compress in Y)
        // |2  0  0|
        // |0 .5  0|
        // |0  0  1|
        assert_eq!(
            parse_transform("matrix(2 0 0 0.5 0 0)").unwrap(),
            Transform2D::scale(2.0, 0.5)
        );

        // Translation matrix
        // |1  0  3|
        // |0  1 -5|
        // |0  0  1|
        assert_eq!(
            parse_transform("matrix(1 0 0 1 3 -5.0)").unwrap(),
            Transform2D::translation(3.0, -5.0)
        );
    }

    // Given the line `1,2 2,4`, apply the following transformation matrix:
    //
    // |1  0  2|
    // |0 .5 -4|
    // |0  0  1|
    //
    // This applies the following steps:
    //
    // - Scale Y by 0.5
    // - Translate by (2,-4)
    #[test]
    fn test_apply_transformation_matrix() {
        let _ = env_logger::try_init();
        let input = r#"
            <?xml version="1.0" encoding="UTF-8" standalone="no"?>
            <svg xmlns="http://www.w3.org/2000/svg" version="1.1">
                <path d="M 1,2 2,4" transform="matrix(1 0 0 0.5 2 -4)"/>
            </svg>
        "#
        .trim();
        let result = parse(input, FLATTENING_TOLERANCE, true).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].len(), 2);
        assert_eq!(result[0][0], (3., -3.).into());
        assert_eq!(result[0][1], (4., -2.).into());
    }

    // Like `test_apply_transformation_matrix`, but with discrete
    // transformations. These should be simplified by usvg.
    #[test]
    fn test_apply_transformations() {
        let _ = env_logger::try_init();
        let input = r#"
            <?xml version="1.0" encoding="UTF-8" standalone="no"?>
            <svg xmlns="http://www.w3.org/2000/svg" version="1.1">
                <path d="M 1,2 2,4" transform="translate(2 -4) scale(1 0.5)"/>
            </svg>
        "#
        .trim();
        let result = parse(input, FLATTENING_TOLERANCE, true).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].len(), 2);
        assert_eq!(result[0][0], (3., -3.).into());
        assert_eq!(result[0][1], (4., -2.).into());
    }

    #[test]
    fn test_polyline_iterate() {
        let polyline = Polyline(vec![
            CoordinatePair { x: 0.0, y: 1.0 },
            CoordinatePair { x: 1.0, y: 0.0 },
        ]);
        // Ensure that a polyline can be iterated
        for pair in &polyline {
            let _ = pair.x + pair.y;
        }
        for pair in polyline {
            let _ = pair.x + pair.y;
        }
    }

    #[test]
    #[allow(clippy::needless_borrow)]
    fn test_polyline_deref() {
        let polyline = Polyline(vec![
            CoordinatePair { x: 0.0, y: 1.0 },
            CoordinatePair { x: 1.0, y: 0.0 },
        ]);
        // A polyline should deref to the underlying vec
        let _empty = polyline.is_empty();
        let _empty = (&polyline).is_empty();
    }
}
