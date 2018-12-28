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
//! Minimal supported Rust version: 1.31 (Rust 2018).
//! 
//! FFI bindings for this crate can be found [on
//! Github](https://github.com/dbrgn/svg2polylines).
//! 
//! You can optionally get serde 1 support by enabling the `serde` feature.

use std::convert;
use std::mem;
use std::str;

use log::trace;
use lyon_bezier::{QuadraticBezierSegment, CubicBezierSegment, Vec2};
use quick_xml::Result as XmlResult;
use quick_xml::events::Event;
use quick_xml::events::attributes::Attribute;
use svgtypes::{PathParser, PathSegment};

#[cfg(feature = "serde")]
use serde::{Serialize, Deserialize};

const FLATTENING_TOLERANCE: f32 = 0.15;

/// A CoordinatePair consists of an x and y coordinate.
#[derive(Debug, PartialEq, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(C)]
pub struct CoordinatePair {
    pub x: f64,
    pub y: f64,
}

impl CoordinatePair {
    fn new(x: f64, y: f64) -> Self {
        CoordinatePair { x: x, y: y }
    }
}

impl convert::From<(f64, f64)> for CoordinatePair {
    fn from(val: (f64, f64)) -> CoordinatePair {
        CoordinatePair { x: val.0, y: val.1 }
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
        CurrentLine {
            line: Polyline::new(),
            prev_end: None,
        }
    }

    /// Add a CoordinatePair to the internal polyline.
    fn add_absolute(&mut self, pair: CoordinatePair) {
        self.line.push(pair);
    }

    /// Add a relative CoordinatePair to the internal polyline.
    fn add_relative(&mut self, pair: CoordinatePair) {
        if let Some(last) = self.line.last().cloned() {
            self.add_absolute(CoordinatePair::new(last.x + pair.x, last.y + pair.y));
        } else if let Some(last) = self.prev_end {
            self.add_absolute(CoordinatePair::new(last.x + pair.x, last.y + pair.y));
        } else {
            self.add_absolute(pair);
        }
    }

    /// Add a CoordinatePair to the internal polyline.
    fn add(&mut self, abs: bool, pair: CoordinatePair) {
        if abs {
            self.add_absolute(pair);
        } else {
            self.add_relative(pair);
        }
    }

    /// A polyline is only valid if it has more than 1 CoordinatePair.
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
            Ok(Event::Start(ref e)) |
            Ok(Event::Empty(ref e)) => {
                trace!("parse_xml: Matched start of {:?}", e.name());
                match e.name() {
                    b"path" => {
                        trace!("parse_xml: Found path attribute");
                        let path_expr: Option<String> = e
                            .attributes()
                            .filter_map(|a: XmlResult<Attribute>| a.ok())
                            .filter_map(|attr: Attribute| {
                                if attr.key == b"d" {
                                    attr.unescaped_value()
                                        .ok()
                                        .and_then(|v| str::from_utf8(&v).map(str::to_string).ok())
                                } else {
                                    None
                                }
                            })
                            .next();
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
            },
            Err(e) => return Err(format!("Error when parsing XML: {}", e)),
            _ => {},
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
    for segment in PathParser::from(expr) {
        parse_path_segment(
            &segment.map_err(|e| format!("Could not parse path segment: {}", e))?,
            &mut line,
            &mut lines,
        )?;
    }

    // Path parsing is done, add previously parsing line if valid
    if line.is_valid() {
        lines.push(line.finish());
    }

    Ok(lines)
}

fn parse_path_segment(
    segment: &PathSegment,
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
        },
        &PathSegment::LineTo { abs, x, y } => {
            trace!("parse_path_segment: LineTo");
            current_line.add(abs, CoordinatePair::new(x, y));
        },
        &PathSegment::HorizontalLineTo { abs, x } => {
            trace!("parse_path_segment: HorizontalLineTo");
            match (current_line.last_y(), abs) {
                (Some(y), true) => current_line.add_absolute(CoordinatePair::new(x, y)),
                (Some(_), false) => current_line.add_relative(CoordinatePair::new(x, 0.0)),
                (None, _) => return Err("Invalid state: HorizontalLineTo on emtpy CurrentLine".into()),
            }
        },
        &PathSegment::VerticalLineTo { abs, y } => {
            trace!("parse_path_segment: VerticalLineTo");
            match (current_line.last_x(), abs) {
                (Some(x), true) => current_line.add_absolute(CoordinatePair::new(x, y)),
                (Some(_), false) => current_line.add_relative(CoordinatePair::new(0.0, y)),
                (None, _) => return Err("Invalid state: VerticalLineTo on emtpy CurrentLine".into()),
            }
        },
        &PathSegment::CurveTo { abs, x1, y1, x2, y2, x, y } => {
            trace!("parse_path_segment: CurveTo");
            let current = current_line.last_pair()
                .ok_or("Invalid state: CurveTo on empty CurrentLine")?;
            let curve = if abs {
                CubicBezierSegment {
                    from: Vec2::new(current.x as f32, current.y as f32),
                    ctrl1: Vec2::new(x1 as f32, y1 as f32),
                    ctrl2: Vec2::new(x2 as f32, y2 as f32),
                    to: Vec2::new(x as f32, y as f32),
                }
            } else {
                CubicBezierSegment {
                    from: Vec2::new(current.x as f32, current.y as f32),
                    ctrl1: Vec2::new((current.x + x1) as f32, (current.y + y1) as f32),
                    ctrl2: Vec2::new((current.x + x2) as f32, (current.y + y2) as f32),
                    to: Vec2::new((current.x + x) as f32, (current.y + y) as f32),
                }
            };
            for point in curve.flattening_iter(FLATTENING_TOLERANCE) {
                current_line.add_absolute(CoordinatePair::new(f64::from(point.x), f64::from(point.y)));
            }
        },
        &PathSegment::Quadratic { abs, x1, y1, x, y } => {
            trace!("parse_path_segment: Quadratic");
            let current = current_line.last_pair()
                .ok_or("Invalid state: Quadratic on empty CurrentLine")?;
            let curve = if abs {
                QuadraticBezierSegment {
                    from: Vec2::new(current.x as f32, current.y as f32),
                    ctrl: Vec2::new(x1 as f32, y1 as f32),
                    to: Vec2::new(x as f32, y as f32),
                }
            } else {
                QuadraticBezierSegment {
                    from: Vec2::new(current.x as f32, current.y as f32),
                    ctrl: Vec2::new((current.x + x1) as f32, (current.y + y1) as f32),
                    to: Vec2::new((current.x + x) as f32, (current.y + y) as f32),
                }
            };
            for point in curve.flattening_iter(FLATTENING_TOLERANCE) {
                current_line.add_absolute(CoordinatePair::new(f64::from(point.x), f64::from(point.y)));
            }
        },
        &PathSegment::ClosePath { .. } => {
            trace!("parse_path_segment: ClosePath");
            current_line.close().map_err(|e| format!("Invalid state: {}", e))?;
        },
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
        assert_eq!(line.close(), Err("Lines with less than 2 coordinate pairs cannot be closed.".into()));
        line.add_absolute((1.0, 2.0).into());
        assert_eq!(line.close(), Err("Lines with less than 2 coordinate pairs cannot be closed.".into()));
        line.add_absolute((2.0, 3.0).into());
        assert_eq!(line.close(), Ok(()));
        let finished = line.finish();
        assert_eq!(finished.len(), 3);
        assert_eq!(finished[0], (1.0, 2.0).into());
        assert_eq!(finished[2], (1.0, 2.0).into());
    }

    #[test]
    /// Parse segment data with a single MoveTo and three coordinates
    fn test_parse_segment_data() {
        let mut current_line = CurrentLine::new();
        let mut lines = Vec::new();
        parse_path_segment(&PathSegment::MoveTo {
            abs: true,
            x: 1.0,
            y: 2.0,
        }, &mut current_line, &mut lines).unwrap();
        parse_path_segment(&PathSegment::LineTo {
            abs: true,
            x: 2.0,
            y: 3.0,
        }, &mut current_line, &mut lines).unwrap();
        parse_path_segment(&PathSegment::LineTo {
            abs: true,
            x: 3.0,
            y: 2.0,
        }, &mut current_line, &mut lines).unwrap();
        assert_eq!(lines.len(), 0);
        let finished = current_line.finish();
        assert_eq!(lines.len(), 0);
        assert_eq!(finished.len(), 3);
        assert_eq!(finished[0], (1.0, 2.0).into());
        assert_eq!(finished[1], (2.0, 3.0).into());
        assert_eq!(finished[2], (3.0, 2.0).into());
    }

    #[test]
    /// Parse segment data with HorizontalLineTo / VerticalLineTo entries
    fn test_parse_segment_data_horizontal_vertical() {
        let mut current_line = CurrentLine::new();
        let mut lines = Vec::new();
        parse_path_segment(&PathSegment::MoveTo {
            abs: true,
            x: 1.0,
            y: 2.0,
        }, &mut current_line, &mut lines).unwrap();
        parse_path_segment(&PathSegment::HorizontalLineTo {
            abs: true,
            x: 3.0,
        }, &mut current_line, &mut lines).unwrap();
        parse_path_segment(&PathSegment::VerticalLineTo {
            abs: true,
            y: -1.0,
        }, &mut current_line, &mut lines).unwrap();
        assert_eq!(lines.len(), 0);
        let finished = current_line.finish();
        assert_eq!(lines.len(), 0);
        assert_eq!(finished.len(), 3);
        assert_eq!(finished[0], (1.0, 2.0).into());
        assert_eq!(finished[1], (3.0, 2.0).into());
        assert_eq!(finished[2], (3.0, -1.0).into());
    }

    #[test]
    /// Parse segment data with HorizontalLineTo / VerticalLineTo entries
    fn test_parse_segment_data_unsupported() {
        let mut current_line = CurrentLine::new();
        let mut lines = Vec::new();
        parse_path_segment(&PathSegment::MoveTo {
            abs: true,
            x: 1.0,
            y: 2.0,
        }, &mut current_line, &mut lines).unwrap();
        let result = parse_path_segment(&PathSegment::SmoothQuadratic {
            abs: true,
            x: 3.0,
            y: 4.0,
        }, &mut current_line, &mut lines);
        assert!(result.is_err());
        assert_eq!(lines.len(), 0);
        let finished = current_line.finish();
        assert_eq!(finished.len(), 1);
        assert_eq!(finished[0], (1.0, 2.0).into());
    }

    #[test]
    /// Parse segment data with multiple MoveTo commands
    fn test_parse_segment_data_multiple() {
        let mut current_line = CurrentLine::new();
        let mut lines = Vec::new();
        parse_path_segment(&PathSegment::MoveTo { abs: true, x: 1.0, y: 2.0, }, &mut current_line, &mut lines).unwrap();
        parse_path_segment(&PathSegment::LineTo { abs: true, x: 2.0, y: 3.0, }, &mut current_line, &mut lines).unwrap();
        parse_path_segment(&PathSegment::MoveTo { abs: true, x: 1.0, y: 3.0, }, &mut current_line, &mut lines).unwrap();
        parse_path_segment(&PathSegment::LineTo { abs: true, x: 2.0, y: 4.0, }, &mut current_line, &mut lines).unwrap();
        parse_path_segment(&PathSegment::MoveTo { abs: true, x: 1.0, y: 4.0, }, &mut current_line, &mut lines).unwrap();
        parse_path_segment(&PathSegment::LineTo { abs: true, x: 2.0, y: 5.0, }, &mut current_line, &mut lines).unwrap();
        parse_path_segment(&PathSegment::MoveTo { abs: true, x: 1.0, y: 5.0, }, &mut current_line, &mut lines).unwrap();
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

    #[cfg(feature="use_serde")]
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
    fn test_parse_xml_single() {
        let _ = env_logger::try_init();
        let input = r#"
            <?xml version="1.0" encoding="UTF-8" standalone="no"?>
            <svg xmlns="http://www.w3.org/2000/svg" version="1.1">
                <path d="M 10,100 40,70 h 10 m -20,40 10,-20" />
            </svg>
        "#;
        let result = parse_xml(&input).unwrap();
        assert_eq!(result, vec!["M 10,100 40,70 h 10 m -20,40 10,-20".to_string()]);
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
        assert_eq!(result, vec![
            "M 10,100 40,70 h 10 m -20,40 10,-20".to_string(),
            "M 20,30".to_string(),
        ]);
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
}
