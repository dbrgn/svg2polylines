#![crate_type = "dylib"]

extern crate libc;
extern crate svg2polylines;

use std::ffi::CStr;
use std::mem;

use libc::{c_char};
use svg2polylines::{CoordinatePair, parse};

#[no_mangle]
pub extern fn svg_str_to_polylines(
    svg: *const c_char,
    out_vec: *const *const CoordinatePair,
    mut out_vec_len: *const usize,
) -> u8 {

    // Convert C string to Rust string
    let c_str = unsafe {
        assert!(!svg.is_null());
        CStr::from_ptr(svg)
    };
    let r_str = c_str.to_str().unwrap();

    // Process
    match parse(r_str) {
        Ok(mut vec) => {
            println!("Done!");
            vec.shrink_to_fit();
//            out_vec = vec.map(|v| v.as_ptr()).as_ptr();
//            out_vec_len = vec.len() as *const usize;
            out_vec_len = vec.len() as *const usize;
            mem::forget(vec);
            0
        },
        Err(e) => {
            1
        }
    }
}
