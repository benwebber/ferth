# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-07-09

### Added

* Implemented the remaining core-ext words (e.g. `case`, `marker`, `value`/`to`, `defer`/`is`, `s\"`, `[compile]`, `:noname`, `unused`).
* Added `time&date` and `ms` from the facility word set.

### Changed

* Renamed the system type and CLI from `Fe`/`fe` to `Ferth`/`ferth`.
* Builtins now declare the host facilities they need (e.g., `Io + Clock`) as trait bounds.
* Compile-only words now throw `-13` (interpreting a compile-only word) in interpretation state, instead of corrupting the input.
* `abort` and `abort"` now use `throw`.
* CLI flags now match Gforth (`-d`, `-e`).
* Booting with too little memory now returns a `KernelError` instead of panicking.
* Invalid memory accesses now raise Forth exceptions instead of panicking.

### Fixed

* Fixed calling builtin primitives in interpretation state.
* Fixed incorrect sign position in numeric literals (e.g., `$-1`).
* `char` now throws `-16` for a zero-length name.
* `s"` now copies its string to a transient buffer in interpretation state.
* Fixed a bug where certain `c"` strings could corrupt the surrounding word under tail-call optimization.
* `hold` no longer corrupts `pad`.
* All `refill` implementations now strip line endings consistently.
* `lshift`/`rshift` no longer panic in debug builds when the shift count exceeds the cell width.

## [0.1.0] - 2026-06-28

Initial release.

[Unreleased]: https://github.com/benwebber/ferth/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/benwebber/ferth/releases/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/benwebber/ferth/releases/tag/v0.1.0
