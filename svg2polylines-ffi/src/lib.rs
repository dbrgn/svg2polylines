#![crate_type = "dylib"]
#![feature(alloc_system)]
extern crate alloc_system;

extern crate libc;
extern crate svg2polylines;

use std::mem;
use std::ffi::CStr;

use libc::{c_char, size_t};
use svg2polylines::{CoordinatePair, parse};

/// Structure that contains a pointer to the coordinate pairs as well as the
/// number of coordinate pairs. It is only used for C interop.
#[derive(Debug)]
#[repr(C)]
pub struct Polyline {
    ptr: *mut CoordinatePair,
    len: size_t,
}

#[no_mangle]
pub extern fn svg_str_to_polylines(
    svg: *const c_char,
    polylines: *mut *mut Polyline,
    polylines_len: *mut size_t,
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
            // Convert `Vec<Vec<CoordinatePair>>` to `Vec<Polyline>`
            let mut tmp_vec: Vec<Polyline> = vec.iter_mut().map(|v| {
                v.shrink_to_fit();
                Polyline {
                    ptr: v.as_mut_ptr(),
                    len: v.len(),
                }
            }).collect();
            tmp_vec.shrink_to_fit();
            assert!(tmp_vec.len() == tmp_vec.capacity());

            // Return number of polylines
            unsafe { *polylines_len = tmp_vec.len() as size_t; }

            // Return pointer to data
            unsafe { *polylines = tmp_vec.as_mut_ptr(); }

            // Prevent memory from being deallocated
            mem::forget(vec);
            mem::forget(tmp_vec);

            0
        },
        Err(_) => 1
    }
}

#[no_mangle]
pub extern fn free_polylines(polylines: *mut Polyline, polylines_len: size_t) {
    unsafe {
        for p in Vec::from_raw_parts(polylines, polylines_len as usize, polylines_len as usize) {
            Vec::from_raw_parts(p.ptr, p.len as usize, p.len as usize);
        }
    }
}
