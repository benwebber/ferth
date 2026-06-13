# Benchmarks

Benchmarks run against both Forth (`ferth`) and Lua (`mlua`) for comparison.

## `fib(20)`

Calculates the 20<sup>th</sup> Fibonacci number with the naive recursive algorithm.

Exercises tree recursion and branching.
Neither recursive call is a tail call.

## `rangesum(1m)`

Sums integers 0 to 999 999 using a counted loop.

Measures loop overhead (`do`/`loop`/`i`) independently of recursion.

## `sieve(100)`

Count prime numbers up to and including 100 using the [sieve of Eratosthenes](https://en.wikipedia.org/wiki/Sieve_of_Eratosthenes).

Exercises memory access and many different control flow words.

## `deepchain`

A deeply nested call chain.

Measures call and return cost in isolation.
The entry point expands to a call chain five levels deep.

## `countdown(60)`

Count down from 60 recursively.

A simple recursive algorithm that benefits from tail call optimization.

## `crc32(4k)`

[CRC-32/ISO-HDLC](https://rosettacode.org/wiki/CRC-32) over a 4 KiB buffer.

Exercises memory access, bitwise operations, and nested loops.
The inner loop runs exactly 8 times per byte, so branch behaviour is uniform.
