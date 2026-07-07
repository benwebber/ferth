# ferth
[![CI](https://github.com/benwebber/ferth/actions/workflows/ci.yml/badge.svg)](https://github.com/benwebber/ferth/actions/workflows/ci.yml)

A safe, native-sized Forth.

Ferth is a [Forth-2012 standard system](https://forth-standard.org/standard/label).
This project provides both the `ferth` crate and the `fe` interpreter.

## Highlights

* Zero-dependency, `no_std` core
* Safe by default
* Native cell width (32- or 64-bit integers)
* Configurable memory and stack sizes
* Tail-call optimization

## Install

Download a pre-built binary of the [latest release](https://github.com/benwebber/ferth/releases/latest) for your system.

Or install with Cargo:

```
cargo install --git https://github.com/benwebber/ferth.git --features repl
```

## Usage

```
fe [-m MEMORY] [-d STACK_CELLS] [-r RETURN_STACK_CELLS] [FILE]
```

## Licence

MIT
