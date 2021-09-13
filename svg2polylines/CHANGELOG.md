# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).


## [Unreleased]


## [0.6.0] - 2021-09-13

Besides bugfixes, this release allows controlling the flattening tolerance.
It's the 2nd parameter, which needs to be passed to the `parse` function.

To get the same behavior as before, simply pass in the value `0.15`.

### Added

- Add `tol` parameter to remove flattening tolerance constant (#16)

### Fixed

- Fix incorrectly generated smooth curves (#17)

### Changed

- This library no longer guarantees a fixed MSRV
- FFI: Mark both functions as unsafe
- Upgrade all dependencies


## [0.5.2] - 2019-08-11

### Added

- CoordinatePair: Make new public


## [0.5.1] - 2019-01-30

### Added

- Add support for shorthand cubic lines (#14)

### Changed

- Upgrade all dependencies


## [0.5.0] - 2018-12-28

### Changed

- Upgrade all dependencies
- This crate now requires Rust 1.31+ (Rust 2018).

### Fixed

- Fix relative moves inside expression (#10)


## [0.4.0] - 2017-06-26

### Added

- Example script with preview feature

### Changed

- svg2polylines now requires Rust 1.16+. In theory Rust 1.15 should work too,
  but the newly added example script depends on the `image` crate which does
  not build on `1.15`.

### Fixed

- Fix move after close (#5)


## [0.3.0] - 2017-06-23

### Changed

- Update `svgparser` and `lyon_bezier` libraries


[Unreleased]: https://github.com/dbrgn/svg2polylines/compare/svg2polylines-0.5.0...HEAD
[0.5.2]: https://github.com/dbrgn/svg2polylines/compare/svg2polylines-0.5.1...svg2polylines-0.5.2
[0.5.1]: https://github.com/dbrgn/svg2polylines/compare/svg2polylines-0.5.0...svg2polylines-0.5.1
[0.5.0]: https://github.com/dbrgn/svg2polylines/compare/svg2polylines-0.4.0...svg2polylines-0.5.0
[0.4.0]: https://github.com/dbrgn/svg2polylines/compare/svg2polylines-0.3.0...svg2polylines-0.4.0
[0.3.0]: https://github.com/dbrgn/svg2polylines/compare/svg2polylines-0.2.0...svg2polylines-0.3.0
