//! Convert an SVG file to a list of polylines (aka polygonal chains or polygonal
//! paths).
//!
//! This can be used e.g. for simple drawing robot that just support drawing
//! straight lines and liftoff / drop pen commands.
//!
//! Flattening of Bézier curves is done using the
//! [Lyon](https://github.com/nical/lyon) library.
//!
//! **Note: Currently the path style is completely ignored. Only the path itself is
//! returned.**
//!
//! FFI bindings for this crate can be found [on
//! Github](https://github.com/dbrgn/svg2polylines).
//!
//! You can optionally get serde 1 support by enabling the `serde` feature.

#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::single_match)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::must_use_candidate)]

use std::convert;
use std::mem;
use std::str;

use log::trace;
use lyon_geom::euclid::Point2D;
use lyon_geom::{CubicBezierSegment, QuadraticBezierSegment};
use quick_xml::events::attributes::Attribute;
use quick_xml::events::Event;
use svgtypes::{PathParser, PathSegment};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

const FLATTENING_TOLERANCE: f64 = 0.15;

/// A `CoordinatePair` consists of an x and y coordinate.
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
}

impl convert::From<(f64, f64)> for CoordinatePair {
    fn from(val: (f64, f64)) -> Self {
        Self { x: val.0, y: val.1 }
    }
}

/// A polyline is a vector of `CoordinatePair` instances.
pub type Polyline = Vec<CoordinatePair>;

#[derive(Debug, PartialEq)]
struct CurrentLine {
    /// The polyline containing the coordinate pairs for the current line.
    line: Polyline,

    /// This is set to the start coordinates of the previous polyline if the
    /// path expression contains multiple polylines.
    prev_end: Option<CoordinatePair>,
}

/// Simple data structure that acts as a Polyline buffer.
impl CurrentLine {
    fn new() -> Self {
        Self {
            line: Polyline::new(),
            prev_end: None,
        }
    }

    /// Add a `CoordinatePair` to the internal polyline.
    fn add_absolute(&mut self, pair: CoordinatePair) {
        self.line.push(pair);
    }

    /// Add a relative `CoordinatePair` to the internal polyline.
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

    /// Add a `CoordinatePair` to the internal polyline.
    fn add(&mut self, abs: bool, pair: CoordinatePair) {
        if abs {
            self.add_absolute(pair);
        } else {
            self.add_relative(pair);
        }
    }

    /// A polyline is only valid if it has more than 1 `CoordinatePair`.
    fn is_valid(&self) -> bool {
        self.line.len() > 1
    }

    /// Return the last coordinate pair (if the line is not empty).
    fn last_pair(&self) -> Option<CoordinatePair> {
        self.line.last().cloned()
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
    fn close(&mut self) -> Result<(), String> {
        if self.line.len() < 2 {
            Err("Lines with less than 2 coordinate pairs cannot be closed.".into())
        } else {
            let first = self.line[0];
            self.line.push(first);
            self.prev_end = Some(first);
            Ok(())
        }
    }

    /// Replace the internal polyline with a new instance and return the
    /// previously stored polyline.
    fn finish(&mut self) -> Polyline {
        self.prev_end = self.line.last().cloned();
        let mut tmp = Polyline::new();
        mem::swap(&mut self.line, &mut tmp);
        tmp
    }
}

/// Parse an SVG string, return vector of path expressions.
fn parse_xml(svg: &str) -> Result<Vec<String>, String> {
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
                        trace!("parse_xml: Found path attribute");
                        let path_expr: Option<String> = e
                            .attributes()
                            .filter_map(Result::ok)
                            .find_map(|attr: Attribute| {
                                if attr.key == b"d" {
                                    attr.unescaped_value()
                                        .ok()
                                        .and_then(|v| str::from_utf8(&v).map(str::to_string).ok())
                                } else {
                                    None
                                }
                            });
                        if let Some(expr) = path_expr {
                            paths.push(expr);
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
            Err(e) => return Err(format!("Error when parsing XML: {}", e)),
        }

        // If we don't keep a borrow elsewhere, we can clear the buffer to keep memory usage low
        buf.clear();
    }
    trace!("parse_xml: Return {} paths", paths.len());
    Ok(paths)
}

fn parse_path(expr: &str) -> Result<Vec<Polyline>, String> {
    trace!("parse_path");
    let mut lines = Vec::new();
    let mut line = CurrentLine::new();

    // Process segments in path expression
    let mut prev_segment_store: Option<PathSegment> = None;
    for segment in PathParser::from(expr) {
        let current_segment =
            segment.map_err(|e| format!("Could not parse path segment: {}", e))?;
        let prev_segment = prev_segment_store.replace(current_segment);
        parse_path_segment(&current_segment, prev_segment, &mut line, &mut lines)?;
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
    abs: bool,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    x: f64,
    y: f64,
) -> Result<(), String> {
    let current = current_line
        .last_pair()
        .ok_or("Invalid state: CurveTo or SmoothCurveTo on empty CurrentLine")?;
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
    for point in curve.flattened(FLATTENING_TOLERANCE) {
        current_line.add_absolute(CoordinatePair::new(point.x, point.y));
    }
    Ok(())
}

#[allow(clippy::similar_names)]
fn parse_path_segment(
    segment: &PathSegment,
    prev_segment: Option<PathSegment>,
    current_line: &mut CurrentLine,
    lines: &mut Vec<Polyline>,
) -> Result<(), String> {
    trace!("parse_path_segment");
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
                    return Err("Invalid state: HorizontalLineTo on emtpy CurrentLine".into())
                }
            }
        }
        &PathSegment::VerticalLineTo { abs, y } => {
            trace!("parse_path_segment: VerticalLineTo");
            match (current_line.last_x(), abs) {
                (Some(x), true) => current_line.add_absolute(CoordinatePair::new(x, y)),
                (Some(_), false) => current_line.add_relative(CoordinatePair::new(0.0, y)),
                (None, _) => {
                    return Err("Invalid state: VerticalLineTo on emtpy CurrentLine".into())
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
            _handle_cubic_curve(current_line, abs, x1, y1, x2, y2, x, y)?;
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
                        let current = current_line.last_pair().ok_or(
                            "Invalid state: CurveTo or SmoothCurveTo on empty CurrentLine",
                        )?;
                        (current.x + dx, current.y + dy)
                    } else {
                        (dx, dy)
                    };
                    _handle_cubic_curve(current_line, abs, x1, y1, x2, y2, x, y)?;
                }
                Some(_) | None => {
                    // The previous segment was not a curve. Use the current
                    // point as reference.
                    match current_line.last_pair() {
                        Some(pair) => {
                            let x1 = pair.x;
                            let y1 = pair.y;
                            _handle_cubic_curve(current_line, abs, x1, y1, x2, y2, x, y)?;
                        }
                        None => {
                            return Err(
                                "Invalid state: SmoothCurveTo without a reference point".into()
                            )
                        }
                    }
                }
            }
        }
        &PathSegment::Quadratic { abs, x1, y1, x, y } => {
            trace!("parse_path_segment: Quadratic");
            let current = current_line
                .last_pair()
                .ok_or("Invalid state: Quadratic on empty CurrentLine")?;
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
            for point in curve.flattened(FLATTENING_TOLERANCE) {
                current_line.add_absolute(CoordinatePair::new(point.x, point.y));
            }
        }
        &PathSegment::ClosePath { .. } => {
            trace!("parse_path_segment: ClosePath");
            current_line
                .close()
                .map_err(|e| format!("Invalid state: {}", e))?;
        }
        other => {
            return Err(format!("Unsupported path segment: {:?}", other));
        }
    }
    Ok(())
}

/// Parse an SVG string into a vector of polylines.
pub fn parse(svg: &str) -> Result<Vec<Polyline>, String> {
    trace!("parse");

    // Parse the XML string into a list of path expressions
    let path_exprs = parse_xml(svg)?;
    trace!("parse: Found {} path expressions", path_exprs.len());

    // Vector that will hold resulting polylines
    let mut polylines: Vec<Polyline> = Vec::new();

    // Process path expressions
    for expr in path_exprs {
        polylines.extend(parse_path(&expr)?);
    }

    trace!("parse: This results in {} polylines", polylines.len());
    Ok(polylines)
}

#[cfg(test)]
#[allow(clippy::unreadable_literal)]
mod tests {
    use super::*;

    #[test]
    fn test_current_line() {
        let mut line = CurrentLine::new();
        assert_eq!(line.is_valid(), false);
        assert_eq!(line.last_x(), None);
        assert_eq!(line.last_y(), None);
        line.add_absolute((1.0, 2.0).into());
        assert_eq!(line.is_valid(), false);
        assert_eq!(line.last_x(), Some(1.0));
        assert_eq!(line.last_y(), Some(2.0));
        line.add_absolute((2.0, 3.0).into());
        assert_eq!(line.is_valid(), true);
        assert_eq!(line.last_x(), Some(2.0));
        assert_eq!(line.last_y(), Some(3.0));
        let finished = line.finish();
        assert_eq!(finished.len(), 2);
        assert_eq!(finished[0], (1.0, 2.0).into());
        assert_eq!(finished[1], (2.0, 3.0).into());
        assert_eq!(line.is_valid(), false);
    }

    #[test]
    fn test_current_line_close() {
        let mut line = CurrentLine::new();
        assert_eq!(
            line.close(),
            Err("Lines with less than 2 coordinate pairs cannot be closed.".into())
        );
        line.add_absolute((1.0, 2.0).into());
        assert_eq!(
            line.close(),
            Err("Lines with less than 2 coordinate pairs cannot be closed.".into())
        );
        line.add_absolute((2.0, 3.0).into());
        assert_eq!(line.close(), Ok(()));
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
            &mut lines,
        )
        .unwrap();
        parse_path_segment(
            &PathSegment::HorizontalLineTo { abs: true, x: 3.0 },
            None,
            &mut current_line,
            &mut lines,
        )
        .unwrap();
        parse_path_segment(
            &PathSegment::VerticalLineTo { abs: true, y: -1.0 },
            None,
            &mut current_line,
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
            &mut lines,
        )
        .unwrap();
        assert_eq!(lines.len(), 3);
        assert_eq!(current_line.is_valid(), false);
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
        "#;
        let result = parse(&input).unwrap();
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
        "#;
        let result = parse(&input).unwrap();
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
        "#;
        let result = parse(&input).unwrap();
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
        "#;
        let result = parse(&input).unwrap();

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
        "#;
        let result = parse(&input).unwrap();
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
        "#;
        let result = parse_xml(&input).unwrap();
        assert_eq!(
            result,
            vec!["M 10,100 40,70 h 10 m -20,40 10,-20".to_string()]
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
        "#;
        let result = parse_xml(&input).unwrap();
        assert_eq!(
            result,
            vec![
                "M 10,100 40,70 h 10 m -20,40 10,-20".to_string(),
                "M 20,30".to_string(),
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
        "#;
        let result = parse_xml(&input).unwrap();
        assert_eq!(result, vec!["M 20,30".to_string()]);
    }

    #[test]
    fn test_parse_xml_malformed() {
        let _ = env_logger::try_init();
        let input = r#"
            <svg xmlns="http://www.w3.org/2000/svg" version="1.1">
                <path d="M 20,30" d="M 10,100 40,70 h 10 m -20,40 10,-20"/>
            </baa>
        "#;
        let result = parse_xml(&input);
        assert_eq!(
            result.unwrap_err(),
            "Error when parsing XML: Expecting </svg> found </baa>".to_string()
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
        "#;
        let result = parse(&input).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].len(), 11);
        assert_eq!(
            result[0],
            vec![
                CoordinatePair::new(0.10650371, 93.221877),
                CoordinatePair::new(0.10650371, 93.221877),
                CoordinatePair::new(1.0590999115751005, 92.1819684952793),
                CoordinatePair::new(5.370943458862083, 89.70221166323438),
                CoordinatePair::new(8.823669349110439, 89.5489159835669),
                CoordinatePair::new(9.72849, 89.74737800000001),
                CoordinatePair::new(12.282201899791776, 90.98899075432975),
                CoordinatePair::new(13.679358042116176, 92.76458821557513),
                CoordinatePair::new(14.196220298368665, 94.94365381717776),
                CoordinatePair::new(14.023847964560911, 96.8907337998339),
                CoordinatePair::new(14.004928, 96.96365600000001),
            ]
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
        "#;
        let result = parse(&input).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].len(), 31);
        assert_eq!(
            result[0],
            vec![
                CoordinatePair::new(10.0, 80.0),
                CoordinatePair::new(18.274009596865902, 62.23607902107565),
                CoordinatePair::new(25.54854286110641, 49.356797920419545),
                CoordinatePair::new(32.00061859514943, 40.276471430451714),
                CoordinatePair::new(37.76877706571886, 34.14452804422132),
                CoordinatePair::new(42.977786748045155, 30.28862586112818),
                CoordinatePair::new(47.75948795454129, 28.192810777806955),
                CoordinatePair::new(52.26776775705932, 27.50166400871596),
                CoordinatePair::new(56.67911619890174, 28.03853054445934),
                CoordinatePair::new(61.17477190430957, 29.815620388680145),
                CoordinatePair::new(65.91841423291706, 33.02014722137494),
                CoordinatePair::new(71.04736316596855, 37.986586554742004),
                CoordinatePair::new(76.67889180557972, 45.17533085552483),
                CoordinatePair::new(82.92363487320814, 55.16763405833094),
                CoordinatePair::new(89.90077281862139, 68.67838317092607),
                CoordinatePair::new(95.0, 80.0),
                CoordinatePair::new(103.2740095968659, 97.76392097892435),
                CoordinatePair::new(110.5485428611064, 110.64320207958045),
                CoordinatePair::new(117.00061859514942, 119.72352856954828),
                CoordinatePair::new(122.76877706571884, 125.85547195577867),
                CoordinatePair::new(127.97778674804515, 129.7113741388718),
                CoordinatePair::new(132.7594879545413, 131.80718922219302),
                CoordinatePair::new(137.26776775705935, 132.49833599128402),
                CoordinatePair::new(141.67911619890177, 131.96146945554065),
                CoordinatePair::new(146.17477190430958, 130.18437961131986),
                CoordinatePair::new(150.91841423291706, 126.97985277862506),
                CoordinatePair::new(156.04736316596853, 122.01341344525798),
                CoordinatePair::new(161.67889180557972, 114.82466914447512),
                CoordinatePair::new(167.92363487320813, 104.83236594166902),
                CoordinatePair::new(174.90077281862128, 91.32161682907416),
                CoordinatePair::new(180.0, 80.0),
            ]
        );
    }
}
