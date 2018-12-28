extern crate env_logger;
extern crate svg2polylines;

use std::env;
use std::fs;
use std::io::Read;
use std::process::exit;

use svg2polylines::Polyline;

fn main() {
    // Logging
    env_logger::init();

    // Argument parsing
    let args: Vec<_> = env::args().collect();
    match args.len() {
        2 => {},
        _ => {
            println!("Usage: {} <path/to/file.svg>", args[0]);
            exit(1);
        },
    };

    // Load file
    let mut file = fs::File::open(&args[1]).unwrap();
    let mut s = String::new();
    file.read_to_string(&mut s).unwrap();

    // Parse data
    let polylines: Vec<Polyline> = svg2polylines::parse(&s).unwrap_or_else(|e| {
        println!("Error: {}", e);
        exit(2);
    });

    // Print data
    println!("Found {} polylines.", polylines.len());
    for line in polylines {
        println!("- {:?}", line);
    }
}
