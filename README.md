# svg2polylines

[![CircleCI][circle-ci-badge]][circle-ci]
[![Crates.io][crates-io-badge]][crates-io]

Convert SVG data to a list of polylines (aka polygonal chains or polygonal
paths).

This can be used e.g. for simple drawing robot that just support drawing
straight lines and liftoff / drop pen commands.

Flattening of Bézier curves is done using the
[Lyon](https://github.com/nical/lyon) library.

**Note: Currently the path style is completely ignored. Only the path itself is
returned.**

This repository contains two creates:

- `svg2polylines` contains all the functionality and can be used like a regular
  Rust library.
- `svg2polylines-ffi` contains a C interface so that the library can be used
  from other programming languages like C or Python.


## Preview

There is a small preview tool to view the generated polylines. It's simple and
hacky, but helps to debug stuff.

```shell
cd svg2polylines
cargo run --release --example preview path/to/file.svg
```

The `--release` parameter is important, otherwise it's going to be very slow.

Use the mouse to drag the image and the `Esc` key to close the window.


## Usage: Rust

Signature:

```rust
fn svg2polylines::parse(&str) -> Result<Vec<Polyline>, String>;
```

See [`svg2polylines/examples/basic.rs`][example-src] for a full usage example.


## FFI

A shared library can be built in the `svg2polylines-ffi` directory with `cargo
build`. You will then find a `libsvg2polylines.so` file in the `target`
directory.

The C interface for `svg2polylines` looks like this

```c
typedef struct CoordinatePair {
    double x;
    double y;
} CoordinatePair;

typedef struct Polyline {
    CoordinatePair* ptr;
    size_t len;
} Polyline;

uint8_t svg_str_to_polylines(char* svg, Polyline** polylines, size_t* polylines_len);
void free_polylines(Polyline* polylines, size_t polylines_len);
```

You should call the `svg_str_to_polylines` function with the following arguments:

- A pointer to the SVG contents (must be valid UTF8).
- A pointer to a `Polyline` pointer. It can be initialized to `NULL` and will be
  updated by the Rust library code.
- A pointer to a `size_t` variable. The variable will be updated by the Rust
  library code.

The return value indicates errors during processing. You **must** check it
before accessing the `polylines` and `polylines_len` pointers. If it equals
`0`, then processing was successful.

Make sure to free the memory again with `free_polylines` once you're done.


## Usage: C

A C usage example can be found at [`svg2polylines-ffi/example.c`][example-c].

Compile it like this:

    $ clang example.c -o example -L target/debug/ -lsvg2polylines -Wall -Wextra -g

Then run the resulting binary like this:

    $ LD_LIBRARY_PATH=target/debug/ ./example

Example output:

    Found 2 polylines!
    Out vec address: 0x55c90045b180
    Polyline 1:
      Address: 0x55c90045b180
      Length: 4
      Points to: 0x55c90045b010
      Data:
        (0.000000, 0.000000)
        (-40.443453, 44.601188)
        (65.767856, 4.913690)
        (70.303571, 34.306548)
    Polyline 2:
      Address: 0x55c90045b190
      Length: 4
      Points to: 0x55c90045b0f0
      Data:
        (0.000000, 35.818452)
        (40.443450, 35.818452)
        (-39.687500, 49.514881)
        (40.065480, 49.514881)


## Usage: Python

A Python usage example (with [CFFI](https://cffi.readthedocs.io/)) can be found
at [`svg2polylines-ffi/example.py`][example-python].

Simply run the script:

    $ python example.py

Example output:

    Found 2 polylines!
    Polyline 1:
      Length: 4
      Points to: <cdata 'CoordinatePair *' 0x2305350>
      Data:
        (0.000000, 34.306548)
        (-40.443453, 44.601188)
        (65.767856, 4.913690)
        (70.303571, 34.306548)
    Polyline 2:
      Length: 4
      Points to: <cdata 'CoordinatePair *' 0x2263370>
      Data:
        (0.000000, 35.818452)
        (40.443450, 35.818452)
        (-39.687500, 49.514881)
        (40.065480, 49.514881)


## License

Licensed under either of

 * Apache License, Version 2.0 (LICENSE-APACHE or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license (LICENSE-MIT or
   http://opensource.org/licenses/MIT) at your option.


### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.


[circle-ci]: https://circleci.com/gh/dbrgn/svg2polylines/tree/master
[circle-ci-badge]: https://circleci.com/gh/dbrgn/svg2polylines/tree/master.svg?style=shield
[crates-io]: https://crates.io/crates/svg2polylines
[crates-io-badge]: https://img.shields.io/crates/v/svg2polylines.svg
[example-src]: https://github.com/dbrgn/svg2polylines/blob/master/svg2polylines/examples/basic.rs
[example-c]: https://github.com/dbrgn/svg2polylines/blob/master/svg2polylines-ffi/example.c
[example-python]: https://github.com/dbrgn/svg2polylines/blob/master/svg2polylines-ffi/example.py
