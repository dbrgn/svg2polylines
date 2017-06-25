# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](http://keepachangelog.com/en/1.0.0/)
and this project adheres to [Semantic Versioning](http://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/dbrgn/svg2polylines/compare/svg2polylines-0.4.0...HEAD
[0.3.0]: https://github.com/dbrgn/svg2polylines/compare/svg2polylines-0.3.0...svg2polylines-0.4.0
[0.3.0]: https://github.com/dbrgn/svg2polylines/compare/svg2polylines-0.2.0...svg2polylines-0.3.0
