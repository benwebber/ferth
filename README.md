# ferth
[![CI](https://github.com/benwebber/ferth/actions/workflows/ci.yml/badge.svg)](https://github.com/benwebber/ferth/actions/workflows/ci.yml)

A safe, native-sized Forth.

## Highlights

* Zero-dependency, `no_std` core
* Implements the Forth-2012 core wordlist
* Native cell width (4 bytes on 32-bit, 8 on 64-bit)
* Memory safety at the host level (Forth programs still have full access to the Forth data space)
* Configurable memory and stack sizes
* Tail-call optimization

## Usage

```
fe [-m MEMORY] [-s STACK_CELLS] [-r RETURN_STACK_CELLS] [FILE]
```

## Architecture

### Token threading

Most Forths use indirect-threaded code (ITC).
In ITC, colon definitions are a sequence of addresses pointing to native code routines.

In safe Rust, it is possible to store function pointers, but not to call functions by pointer address.
Therefore this Forth uses **token threading** instead.
Every word body consists of a sequence of bytecode instructions.
The inner interpreter works on this sequence of instructions directly.

The compiler inlines primitive operations like `dup` and `+`.
There is no `DOCOL` instruction to execute colon definitions.
Instead of following an address and *discovering* the target definition is a colon definition, the `Call` instruction immediately nests, jumps, and begins executing the target word body.

## Licence

MIT
