use std::env;
use std::fs;
use std::io::Read;
use std::process::exit;

use svg2polylines::{self, Polyline};

use csv::Writer;

fn main() {
    // Logging
    env_logger::init();

    // Argument parsing
    let args: Vec<_> = env::args().collect();
    match args.len() {
        2 => {}
        _ => {
            println!("Usage: {} <path/to/file.svg>", args[0]);
            exit(1);
        }
    };

    // Load file
    let mut file = fs::File::open(&args[1]).unwrap();
    let mut s = String::new();
    file.read_to_string(&mut s).unwrap();

    // Parse data
    let polylines: Vec<(Option<String>, Polyline)> = svg2polylines::parse(&s, 0.15, true).unwrap_or_else(|e| {
        println!("Error: {}", e);
        exit(2);
    });

    // Print data
    println!("Found {} polylines.", polylines.len());
    for (num, (id, line)) in polylines.iter().enumerate() {
        let filename = if let Some(id_str) = id {
            format!("{}_{}.csv", id_str, num)
        } else {
            format!("unk_{}.csv", num)
        };

        let mut wtr = Writer::from_path(filename).unwrap();
        for row in line {
            wtr.serialize(row).unwrap();
        }
        wtr.flush().unwrap();
    }
}
