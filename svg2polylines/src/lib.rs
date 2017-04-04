//! Convert an SVG file to a list of polylines (aka polygonal chains or polygonal
//! paths).
//! 
//! This can be used e.g. for simple drawing robot that just support drawing
//! straight lines and liftoff / drop pen commands.
//! 
//! **Note: Currently the library only supports straight lines, no curves!** Also,
//! the path style is completely ignored. Only the path itself is returned.
//! 
//! Minimal required Rust version: 1.13.
//! 
//! FFI bindings for this crate can be found [on
//! Github](https://github.com/dbrgn/svg2polylines).
//! 
//! You can optionally get serde support by enabling the `use_serde` feature.
#[macro_use] extern crate log;
extern crate svgparser;
extern crate lyon_bezier;

#[cfg(feature="use_serde")]
extern crate serde;
#[cfg(feature="use_serde")]
#[macro_use] extern crate serde_derive;

use std::convert;
use std::mem;
use std::str;

use svgparser::{svg, path, Stream};
use svgparser::path::is_absolute;
use svgparser::path::SegmentData::{
    self, MoveTo, LineTo, HorizontalLineTo, VerticalLineTo, ClosePath, CurveTo,
    Quadratic,
};
use lyon_bezier::{CubicBezierSegment, Vec2};


/// A CoordinatePair consists of an x and y coordinate.
#[derive(Debug, PartialEq, Copy, Clone)]
#[cfg_attr(feature = "use_serde", derive(Serialize, Deserialize))]
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
    line: Polyline,
}

/// Simple data structure that acts as a Polyline buffer.
impl CurrentLine {
    fn new() -> Self {
        CurrentLine { line: Polyline::new() }
    }

    /// Add a CoordinatePair to the internal polyline.
    fn add_absolute(&mut self, pair: CoordinatePair) {
        self.line.push(pair);
    }

    /// Add a relative CoordinatePair to the internal polyline.
    fn add_relative(&mut self, pair: CoordinatePair) {
        match self.line.last().cloned() {
            Some(last) => self.add_absolute(CoordinatePair::new(last.x + pair.x, last.y + pair.y)),
            None => self.add_absolute(pair),
        };
    }

    /// Add a CoordinatePair to the internal polyline.
    fn add(&mut self, relativity: Relativity, pair: CoordinatePair) {
        match relativity {
            Relativity::Absolute => self.add_absolute(pair),
            Relativity::Relative => self.add_relative(pair),
        };
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

#[derive(Debug, Copy, Clone)]
enum Relativity {
    Relative,
    Absolute,
}
use Relativity::*;

impl convert::From<u8> for Relativity {
    fn from(val: u8) -> Relativity {
        if is_absolute(val) {
            Relativity::Absolute
        } else {
            Relativity::Relative
        }
    }
}

fn parse_segment_data(data: &SegmentData,
                      relativity: Relativity,
                      current_line: &mut CurrentLine,
                      lines: &mut Vec<Polyline>) -> Result<(), String> {
    match data {
        &MoveTo { x, y } => {
            if current_line.is_valid() {
                lines.push(current_line.finish());
            }
            current_line.add(relativity, CoordinatePair::new(x, y));
        },
        &LineTo { x, y } => {
            current_line.add(relativity, CoordinatePair::new(x, y));
        },
        &HorizontalLineTo { x } => {
            match (current_line.last_y(), relativity) {
                (Some(y), Absolute) => current_line.add_absolute(CoordinatePair::new(x, y)),
                (Some(_), Relative) => current_line.add_relative(CoordinatePair::new(x, 0.0)),
                (None, _) => return Err("Invalid state: HorizontalLineTo on emtpy CurrentLine".into()),
            }
        },
        &VerticalLineTo { y } => {
            match (current_line.last_x(), relativity) {
                (Some(x), Absolute) => current_line.add_absolute(CoordinatePair::new(x, y)),
                (Some(_), Relative) => current_line.add_relative(CoordinatePair::new(0.0, y)),
                (None, _) => return Err("Invalid state: VerticalLineTo on emtpy CurrentLine".into()),
            }
        },
        &CurveTo { x1, y1, x2, y2, x, y } => {
            let current = current_line.last_pair()
                .ok_or("Invalid state: CurveTo on empty CurrentLine")?;
            let curve = match relativity {
                Absolute => CubicBezierSegment {
                    from: Vec2::new(current.x as f32, current.y as f32),
                    ctrl1: Vec2::new(x1 as f32, y1 as f32),
                    ctrl2: Vec2::new(x2 as f32, y2 as f32),
                    to: Vec2::new(x as f32, y as f32),
                },
                Relative => CubicBezierSegment {
                    from: Vec2::new(current.x as f32, current.y as f32),
                    ctrl1: Vec2::new((current.x + x1) as f32, (current.y + y1) as f32),
                    ctrl2: Vec2::new((current.x + x2) as f32, (current.y + y2) as f32),
                    to: Vec2::new((current.x + x) as f32, (current.y + y) as f32),
                },
            };
            for point in curve.flattening_iter(0.15) {
                current_line.add_absolute(CoordinatePair::new(point.x as f64, point.y as f64));
            }
        },
        &ClosePath => {
            current_line.close().map_err(|e| format!("Invalid state: {}", e))?;
        },
        d @ _ => {
            return Err(format!("Unsupported segment data: {:?}", d));
        }
    }
    Ok(())
}

fn parse_path(data: Stream) -> Vec<Polyline> {
    debug!("New path");

    let mut lines = Vec::new();

    let mut p = path::Tokenizer::new(data);
    let mut line = CurrentLine::new();
    loop {
        match p.parse_next() {
            Ok(segment_token) => {
                match segment_token {
                    path::SegmentToken::Segment(segment) => {
                        debug!("  Segment data: {:?}", segment.data);
                        parse_segment_data(
                            &segment.data,
                            Relativity::from(segment.cmd),
                            &mut line,
                            &mut lines
                        ).unwrap();
                    },
                    path::SegmentToken::EndOfStream => break,
                }
            },
            Err(e) => {
                warn!("Invalid path segment: {:?}", e);
                break;
            },
        }
    }

    // Path parsing is done, add previously parsing line if valid
    if line.is_valid() {
        lines.push(line.finish());
    }

    lines
}

/// Parse an SVG string into a vector of polylines.
pub fn parse(svg: &str) -> Result<Vec<Polyline>, String> {
    let bytes = svg.as_bytes();

    let mut polylines = Vec::new();
    let mut tokenizer = svg::Tokenizer::new(&bytes);
    loop {
        match tokenizer.parse_next() {
            Ok(t) => {
                match t {
                    svg::Token::Attribute(name, value) => {
                        // Process only 'd' attributes
                        if name == b"d" {
                            polylines.extend(parse_path(value));
                        }
                    },
                    svg::Token::EndOfStream => break,
                    _ => {},
                }
            },
            Err(e) => {
                println!("Error: {:?}", e);
                return Err(e.to_string());
            }
        }
    }

    Ok(polylines)
}

#[cfg(test)]
mod tests {
    extern crate svgparser;
    #[cfg(feature="use_serde")]
    extern crate serde_json;

    use svgparser::path::SegmentData;

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
        parse_segment_data(&SegmentData::MoveTo {
            x: 1.0,
            y: 2.0,
        }, &mut current_line, &mut lines).unwrap();
        parse_segment_data(&SegmentData::LineTo {
            x: 2.0,
            y: 3.0,
        }, &mut current_line, &mut lines).unwrap();
        parse_segment_data(&SegmentData::LineTo {
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
        parse_segment_data(&SegmentData::MoveTo {
            x: 1.0,
            y: 2.0,
        }, &mut current_line, &mut lines).unwrap();
        parse_segment_data(&SegmentData::HorizontalLineTo {
            x: 3.0,
        }, &mut current_line, &mut lines).unwrap();
        parse_segment_data(&SegmentData::VerticalLineTo {
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
        parse_segment_data(&SegmentData::MoveTo {
            x: 1.0,
            y: 2.0,
        }, &mut current_line, &mut lines).unwrap();
        let result = parse_segment_data(&SegmentData::SmoothQuadratic {
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
        parse_segment_data(&SegmentData::MoveTo { x: 1.0, y: 2.0, }, &mut current_line, &mut lines).unwrap();
        parse_segment_data(&SegmentData::LineTo { x: 2.0, y: 3.0, }, &mut current_line, &mut lines).unwrap();
        parse_segment_data(&SegmentData::MoveTo { x: 1.0, y: 3.0, }, &mut current_line, &mut lines).unwrap();
        parse_segment_data(&SegmentData::LineTo { x: 2.0, y: 4.0, }, &mut current_line, &mut lines).unwrap();
        parse_segment_data(&SegmentData::MoveTo { x: 1.0, y: 4.0, }, &mut current_line, &mut lines).unwrap();
        parse_segment_data(&SegmentData::LineTo { x: 2.0, y: 5.0, }, &mut current_line, &mut lines).unwrap();
        parse_segment_data(&SegmentData::MoveTo { x: 1.0, y: 5.0, }, &mut current_line, &mut lines).unwrap();
        assert_eq!(lines.len(), 3);
        assert_eq!(current_line.is_valid(), false);
        let finished = current_line.finish();
        assert_eq!(finished.len(), 1);
    }

    #[test]
    fn test_parse_simple_nonclosed() {
        let input = r#"
            <?xml version="1.0" encoding="UTF-8" standalone="no"?>
            <svg xmlns="http://www.w3.org/2000/svg" version="1.1">
                <path d="m 113,35 h 40 l -39,49 h 40" />
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
    fn test_parse_simple_closed() {
        let input = r#"
            <?xml version="1.0" encoding="UTF-8" standalone="no"?>
            <svg xmlns="http://www.w3.org/2000/svg" version="1.1">
                <path d="m 10,10 20,15 10,20 z" />
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

}
