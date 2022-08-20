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

This repository contains the following crate:

- `svg2polylines` contains all the functionality and can be used like a regular
  Rust library.


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

This crate used to contain FFI bindings. These have been dropped as of version
0.8.0. If you need them, open an issue on GitHub and I might bring them back.


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


[circle-ci]: https://circleci.com/gh/dbrgn/svg2polylines/tree/main
[circle-ci-badge]: https://circleci.com/gh/dbrgn/svg2polylines/tree/main.svg?style=shield
[crates-io]: https://crates.io/crates/svg2polylines
[crates-io-badge]: https://img.shields.io/crates/v/svg2polylines.svg
[example-src]: https://github.com/dbrgn/svg2polylines/blob/main/svg2polylines/examples/basic.rs
