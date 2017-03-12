extern crate svgparser;

use std::str;

use svgparser::{svg, path, Stream};

fn parse_path(data: Stream) -> Vec<path::Segment> {
    println!("New path:");

    let mut tokens = Vec::new();

    let mut p = path::Tokenizer::new(data);
    loop {
        match p.parse_next() {
            Ok(segment_token) => {
                match segment_token {
                    path::SegmentToken::Segment(segment) => {
                        tokens.push(segment);
                        println!("  {:?}", segment);
                    },
                    path::SegmentToken::EndOfStream => break,
                }
            },
            Err(e) => {
                println!("Warning: {:?}", e);
                break;
            },
        }
    }

    tokens
}

pub fn parse(svg: &str) {
    let bytes = svg.as_bytes();

    let mut paths = Vec::new();

    let mut p = svg::Tokenizer::new(&bytes);
    loop {
        match p.parse_next() {
            Ok(t) => {
                match t {
                    svg::Token::Attribute(name, value) => {
                        // Process only 'd' attributes
                        if name == b"d" {
                            let segments = parse_path(value);
                            paths.push(segments);
                        }
                    },
                    svg::Token::EndOfStream => break,
                    _ => {},
                }
            },
            Err(e) => {
                println!("Error: {:?}", e);
                return;
            }
        }
    }

    //println!("{:?}", paths);
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}
