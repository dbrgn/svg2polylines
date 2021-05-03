#![crate_type = "dylib"]

use std::ffi::CStr;
use std::mem;

use libc::{c_char, c_double, size_t};
use svg2polylines::{parse, CoordinatePair};

/// Structure that contains a pointer to the coordinate pairs as well as the
/// number of coordinate pairs. It is only used for C interop.
#[derive(Debug)]
#[repr(C)]
pub struct Polyline {
    ptr: *mut CoordinatePair,
    len: size_t,
}

/// Convert the specified SVG string to an array of polylines.
///
/// The two pointers `polylines` and `polylines_len` are out parameters.
/// Initialize them like this:
///
/// ```c
/// Polyline* polylines = NULL;
/// size_t polylines_len = 0;
/// ```
///
/// Pass them in like this:
///
/// ```c
/// uint8_t err = svg_str_to_polylines(input, &polylines, &polylines_len);
/// ```
///
/// # Safety
///
/// The `svg` pointer must point to a valid C-style 0-terminated string.
#[no_mangle]
pub unsafe extern "C" fn svg_str_to_polylines(
    svg: *const c_char,
    tol: c_double,
    polylines: *mut *mut Polyline,
    polylines_len: *mut size_t,
) -> u8 {
    // Convert C string to Rust string
    let c_str = {
        assert!(!svg.is_null());
        CStr::from_ptr(svg)
    };
    let r_str = c_str.to_str().unwrap();

    // Process
    match parse(r_str, tol) {
        Ok(vec) => {
            // Convert `Vec<Vec<CoordinatePair>>` to `Vec<Polyline>`
            let mut tmp_vec: Vec<Polyline> = vec
                .into_iter()
                .map(|mut v| {
                    v.shrink_to_fit();
                    let p = Polyline {
                        ptr: v.as_mut_ptr(),
                        len: v.len(),
                    };
                    mem::forget(v);
                    p
                })
                .collect();
            tmp_vec.shrink_to_fit();
            assert!(tmp_vec.len() == tmp_vec.capacity());

            // Return number of polylines
            *polylines_len = tmp_vec.len() as size_t;

            // Return pointer to data
            *polylines = tmp_vec.as_mut_ptr();

            // Prevent memory from being deallocated
            mem::forget(tmp_vec);

            0
        }
        Err(_) => 1,
    }
}

/// Free the specified `polyline_len` polylines.
///
/// # Safety
///
/// The user must be sure that the raw pointer is still valid and points to a
/// polyline array previously allocated by `svg_str_to_polylines`.
#[no_mangle]
pub unsafe extern "C" fn free_polylines(polylines: *mut Polyline, polylines_len: size_t) {
    for p in Vec::from_raw_parts(polylines, polylines_len as usize, polylines_len as usize) {
        Vec::from_raw_parts(p.ptr, p.len as usize, p.len as usize);
    }
}
