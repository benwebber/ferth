# Optimization

This document describes optimization opportunities and prior work.

First, some background on the execution model.
In a classic indirect-threaded Forth, a word's code field always contains the address of the machine code routine, i.e., a function pointer.
It is not possible to directly execute function pointers in safe Rust.
Therefore this Forth uses a hybrid execution model:

* The code field always contains an opcode value.
* Cells in the word body always contain execution tokens (XTs).
  In this system, XTs are always code field addresses (CFAs).
* Colon definitions *nest*.
  The `DoCol` opcode pushes a call frame onto the return stack, and the inner interpreter executes the word body exactly as it would any other stream of words.

The majority of opcodes correspond to simple words that either lack a body (e.g., `+`) or read a specific number of cells from the body (e.g., `(lit)` and `(jmp)`).

Furthermore, some primitives are implemented in the outer interpreter for convenience or out of necessity.
These include parsing words and I/O words.
The inner interpreter yields control to the outer interpreter to execute these words.
Aside from the yield mechanism, these are similar to the byte code instructions; they compile to native code.

Nesting is comparatively expensive and compounds.
Consider a definition like:

```forth
: square dup * ;
```

If `dup` and `*` are both opcodes, executing `square` only requires nesting once (into `square`).
However, if `dup` and `*` are colon definitions themselves, executing them requires multiple levels of nesting (into `square`, then into `dup`, etc.).

In [057c124](https://github.com/benwebber/ferth/commit/057c124b31c1812129f6cad90d9a74d5c77055c0), with Claude, I generated a set of benchmarks and their Lua equivalents.
The benchmarks indicate that nesting is the most significant optimization target.

I use [cargo-show-asm](https://github.com/pacak/cargo-show-asm) to review the generated assembly.

## Unchecked stack access

The inner interpreter stores the stacks in main memory and stores pointers to the tops of the stacks in struct fields.
Reading stack values safely requires validating the address and copying the word bytes from memory to a temporary buffer.

The outer interpreter&mdash;and therefore program code&mdash;cannot directly manipulate these registers except through prescribed methods and opcodes.
Therefore we can trust that the stack pointers always point to valid stack cells.

---

See [6582330](https://github.com/benwebber/ferth/commit/658233063d96e1f94c0f59a5672a07e6f223c9cf).

I introduced an `unsafe` feature flag to cast stack cell ranges directly to and from `usize` values.
This improves all stack operations and yielded a 5 to 10% improvement on all benchmarks.

## Dense opcode values

For byte code dispatch like this, LLVM will compile dispatch to a single jump table if the range of discriminant values is dense.
During development, I added and removed opcodes, and never renumbered them sequentially, leaving lots of gaps.
The maximum opcode value was `0x33` (51) even though there were only 34 instructions.
I was curious if renumbering them sequentially would generate a more efficient table.

---

See [3992865](https://github.com/benwebber/ferth/commit/3992865aa6eed56691436d8c2f895460291b108c).

Renumbering them sequentially had no impact on execution.
LLVM already generated a jump table for dispatch.
Renumbering them saved tens of bytes from the binary size because LLVM did generate a *smaller* table.
Previously, each gap in the range fell through to the invalid opcode branch.

## Unchecked IP access

Similar to stack access, only the inner interpreter has access to the IP register.
It is safe to cast this value directly to a `usize`.

In contrast, the outer interpreter *can* manipulate the W register through `Vm::call`, so an unsafe read would be truly unsafe.

---

See [6a2fc4f](https://github.com/benwebber/ferth/commit/6a2fc4f5a7d7e88e460c70675fa90b50351a73f7).

Similar to the unchecked stack access optimization, this yielded an improvement of 5 to 15% across all benchmarks except `countdown`.
The `deepchain` benchmark showed an improvement of 35%.

## Introduce primitive for `create`&hellip;`does>` words

Previously, `create` compiled an execution token directly to the code field that `does>` would later replace.

In this system, XTs are cell addresses.
This incurred an extra branch in the hot inner loop (`Vm::dispatch`) to check that the value fell outside the range of the inner interpreter stacks.
This also indirectly checked that the value *was* an address and not an opcode, because the opcode values all fall within the stack address range.

---

See [5d31e99](https://github.com/benwebber/ferth/commit/5d31e995e870799a50a882fe6d6799b741e5a7f9).

I implemented a dedicated `DoCreate` opcode to remove this branch.
Although no benchmark exercises `create`&hellip;`does>` words directly, this change modestly improved all `Vm::dispatch` calls.
The `deepchain` benchmark showed a 20% improvement.

## Tail call optimization

Many Forth words end in calls to other Forth words.
Consider `/mod` in this system:

```forth
: /mod ( n1 n2 -- n3 n4 ) >r s>d r> sm/rem ;
```

Because `sm/rem` is a colon definition, this pushes another call frame on the return stack.
Once again, nesting incurs a cost.

We can eliminate this nesting cost if we jump directly to the code field address of `sm/rem` instead of executing it indirectly.

## Inlining

Consider the definition of `xor`:

```forth
: xor over over and invert >r or r> and ;
```

This is a lot of operations for a simple bitwise operation.
Fundamental words like this would benefit from inlining: instead of compiling calls to `over`, `and`, etc., we can expand those words' code and copy it directly into the body of `xor`.

We would implement this with another word, like `immediate`, that transforms the latest defined word:

```forth
: xor over over and invert >r or r> and ; (inline)
```

Complications arise quickly.

First, the compiler needs to know how *many* cells to inline.
The word header will need to include a length value indicating the length of its body field.
Second, it would be difficult (perhaps impossible) to recurse into nested definitions without blowing up memory.
We would likely need to restrict inlining to one level below the current word.
Finally, we would need to adjust internal jump addresses because modifying the body will affect their relative positions.

## Token threading

Currently, in a definition composed of primitives, the body always contains a sequence of XTs.
Consider `square` again:

```
[XT of dup] [XT of *] [XT of exit]
```

Executing native code primitives always requires two reads: one to fetch the XT and one to fetch the opcode.

Token threading would compile opcode values directly into the body:

```
[dup] [*] [exit]
```

This eliminates a read for every single primitive.
Colon definitions would continue to use indirect execution:

```
[docol] [target]
```

Furthermore, it would also be possible to encode the opcode values as single bytes instead of full cells.
This could improve cache locality.

## Cache stack limits as struct fields

The inner interpreter needs to check if addresses fall within or without the stack ranges often.
This requires basic arithmetic on the stack sizes.
Caching these bounds as fields would simplify the checks.

---

See [a0a2351](https://github.com/benwebber/ferth/commit/a0a23517f75baa2515906d343b794dfc140d45e6).

This removed arithmetic from every stack mutation, so all benchmarks showed a modest improvement in wall clock time of about 5%.
`countdown` and `deepchain` exercise the return stack more extensively.
They showed an improvement of around 35%.

## Store top of data stack in register

Most stack and arithmetic operations would benefit if the inner interpreter stored the value at the top of the data stack in a register.
For example, `+` would only need to read the second value from memory, and it would store the result to the register directly, saving two memory accesses.

---

This change (up to [9886cb1](https://github.com/benwebber/ferth/commit/9886cb1ea50e14fcabfef654514b07ff4d6ce3d6)) initially had mixed results.

* `countdown` and `deepchain` showed no change.
* `crc32` and `fib` regressed by about 2%.
* `rangesum` and `sieve` improved by about 2%.

Words that perform a lot of binary stack arithmetic, where the next word can read directly from the register, saw an improvement.
Words that perform a lot of stack operations without binary arithmetic saw a regression.
This was primarily because `Vm::push` and `Vm::pop` now required bounds checks to spill and reload the TOS register, respectively.

Next I reserved a scratch cell below the data stack to absorb writes, eliminating the bounds checks ([132e3b8](https://github.com/benwebber/ferth/commit/132e3b84e4deabcc5def45816bd18b4b844e8365)).
This improved wall clock time by 3 to 15% on all benchmarks except `deepchain`.
This makes sense because `deepchain` exercises the return stack and call path more than the data stack.

## Implement more primitives in Rust

During development, I strived to minimize the number of opcodes I implemented.
This includes some words as fundamental as `dup`, which I originally defined using the stack pointer words.
I did this partially to internalize the Forth execution model, but also because I wanted to see the effect of optimizing these common words in native code.
