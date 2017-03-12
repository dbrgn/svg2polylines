extern crate svg2polylines;

use std::env;
use std::fs;
use std::io::Read;
use std::process::exit;

fn main() {
    let args: Vec<_> = env::args().collect();
    match args.len() {
        2 => {},
        _ => {
            println!("Usage: {} <path/to/file.svg>", args[0]);
            exit(1);
        },
    };

    let mut file = fs::File::open(&args[1]).unwrap();
    let mut s = String::new();
    file.read_to_string(&mut s).unwrap();

    svg2coordinates::parse(&s);
}
