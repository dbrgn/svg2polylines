#[macro_use] extern crate log;
extern crate svgparser;

use std::str;

use svgparser::{svg, path, Stream};
use svgparser::path::SegmentData::{MoveTo, LineTo, HorizontalLineTo, VerticalLineTo};

pub type CoordinatePair = (f64, f64);
pub type Polyline = Vec<CoordinatePair>;

fn parse_path(data: Stream) -> Vec<Polyline> {
    debug!("New path");

    let mut lines = Vec::new();

    let mut p = path::Tokenizer::new(data);
    let mut line = Polyline::new();
    loop {
        match p.parse_next() {
            Ok(segment_token) => {
                match segment_token {
                    path::SegmentToken::Segment(segment) => {
                        debug!("  Segment data: {:?}", segment.data);
                        match segment.data {
                            MoveTo { x: x, y: y } => {
                                if line.len() > 1 {
                                    lines.push(line);
                                }
                                line = Polyline::new();
                                line.push((x, y));
                            },
                            LineTo { x: x, y: y } => {
                                line.push((x, y));
                            },
                            d @ _ => {
                                println!("Unsupported segment data: {:?}", d);
                            }
                        }
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

    lines
}

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
    #[test]
    fn it_works() {
    }
}
