extern crate drag_controller;
extern crate env_logger;
extern crate piston_window;
extern crate svg2polylines;

use std::env;
use std::fs;
use std::io::Read;
use std::process::exit;

use drag_controller::{DragController, Drag};
use piston_window::{PistonWindow, WindowSettings, OpenGL, Transformed, clear, line};
use piston_window::math::Matrix2d;
use svg2polylines::Polyline;

fn main() {
    // Logging
    env_logger::init().expect("Could not initialize env logger");

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

    // Create window
    let opengl = OpenGL::V3_2;
    let scale = 2;
    let fscale = 2.0;
    let window_size = [716 * scale, 214 * scale];
    let mut window: PistonWindow = WindowSettings::new("Preview (press ESC to exit)", window_size)
        .exit_on_esc(true)
        .opengl(opengl)
        .build()
        .unwrap();

    // Show window
    let black = [0.0, 0.0, 0.0, 1.0];
    let radius = 1.0;
    let mut drag = DragController::new();
    let mut translate: Matrix2d = [[1.0, 0.0, 0.0],
                                   [0.0, 1.0, 0.0]];
    let mut translate_tmp: Matrix2d = translate.clone();
    let mut translate_start = None;
    while let Some(e) = window.next() {
        drag.event(&e, |action| {
            match action {
                Drag::Start(x, y) => {
                    translate_start = Some((x, y));
                    true
                },
                Drag::Move(x, y) => {
                    let start_x = translate_start.unwrap().0;
                    let start_y = translate_start.unwrap().1;
                    translate_tmp = translate.trans(x - start_x, y - start_y);
                    true
                },
                Drag::End(..) => {
                    translate_start = None;
                    translate = translate_tmp;
                    false
                },
                // Continue dragging when receiving focus.
                Drag::Interrupt => true,
            }
        });
        window.draw_2d(&e, |c, g| {
            clear([1.0; 4], g);
            for polyline in &polylines {
                for pair in polyline.windows(2) {
                    line(black,
                         radius,
                         [pair[0].x, pair[0].y, pair[1].x, pair[1].y],
                         c.transform.append_transform(translate_tmp).scale(fscale, fscale),
                         g);
                }
            }
        });
    }
}
