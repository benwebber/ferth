# System

This document specifies the system behaviour according to the Forth-2012 [documentation requirements](https://forth-standard.org/standard/doc).

## Implementation-defined options

<dl>
<dt>aligned address requirements (<a href="https://forth-standard.org/standard/usage#usage:addr">3.1.3.3 Addresses</a>)
<dd>Aligned to the size of <a href="https://doc.rust-lang.org/std/primitive.usize.html"><code>usize</code></a> (4 bytes on 32-bit, 8 bytes on 64-bit).

<dt>behaviour of <a href="https://forth-standard.org/standard/core/EMIT"><code>EMIT</code></a> for non-graphic characters
<dd>Passed through to the host <code>emit</code> implementation as a raw byte.

<dt>character editing of <a href="https://forth-standard.org/standard/core/ACCEPT"><code>ACCEPT</code></a>
<dd>None. <code>accept</code> reads characters one at a time via <code>key</code> until it reads <code>LF</code> (<code>&bsol;n</code>, <code>0x0a</code>).

<dt>character set (<a href="https://forth-standard.org/standard/usage#usage:char">3.1.2 Character types</a>, <a href="https://forth-standard.org/standard/core/EMIT"><code>EMIT</code></a>, <a href="https://forth-standard.org/standard/core/KEY"><code>KEY</code></a>)
<dd>Characters are raw bytes (<code>u8</code>). The system assumes ASCII for parsing and name matching. <code>emit</code> and <code>key</code> pass bytes to and from the host directly.

<dt>character-aligned address requirements (<a href="https://forth-standard.org/standard/usage#usage:addr">3.1.3.3 Addresses</a>)
<dd>Aligned to byte (<a href="https://doc.rust-lang.org/std/primitive.u8.html"><code>u8</code></a>).

<dt id="charset-match">character-set-extensions matching characteristics (<a href="https://forth-standard.org/standard/usage#usage:find">3.4.2 Finding definition names</a>)
<dd>Case-insensitive match for characters in ASCII range, exact match for others.

<dt>conditions under which control characters match a space delimiter (<a href="https://forth-standard.org/standard/usage#usage:delim">3.4.1.1 Delimiters</a>)
<dd>If the delimiter is <code>bl</code> (<code>SPACE</code>), <code>parse</code> treats ASCII whitespace characters as delimiters. <code>parse-name</code> always skips ASCII control and whitespace characters.

<dt>conversion of digits larger than thirty-five (<a href="https://forth-standard.org/standard/usage#usage:digits">3.2.1.2 Digit conversion</a>)
<dd>Base 36 is the largest possible base.

<dt>display after input terminates in <a href="https://forth-standard.org/standard/core/ACCEPT"><code>ACCEPT</code></a>
<dd>None. <code>accept</code> consumes but does not display a terminating <code>LF</code>.

<dt>exception abort sequence (as in <a href="https://forth-standard.org/standard/core/ABORTq"><code>ABORT"</code></a>)
<dd>Throw <code>-2</code>, caught in `quit`. `quit` clears the stacks and prints the error message.

<dt>format of the control-flow stack (<a href="https://forth-standard.org/standard/usage#usage:controlstack">3.2.3.2 Control-flow stack</a>)
<dd>The data stack. There is no separate control-flow stack.

<dt>input line terminator (<a href="https://forth-standard.org/standard/usage#usage:input">3.2.4.1 User input device</a>)
<dd>Line feed (<code>0x0a</code>). Consumed but not written to the input buffer.

<dt>maximum size of a counted string, in characters (<a href="https://forth-standard.org/standard/usage#usage:cstring">3.1.3.4 Counted strings</a>, <a href="https://forth-standard.org/standard/core/WORD"><code>WORD</code></a>)
<dd>255

<dt>maximum size of a definition name, in characters (<a href="https://forth-standard.org/standard/usage#usage:names">3.3.1.2 Definition names</a>)
<dd>31. Names longer than this throw exception <code>-19</code> (<em>definition name too long</em>).

<dt>maximum size of a parsed string (<a href="https://forth-standard.org/standard/usage#usage:parsing">3.4.1 Parsing</a>)
<dd>256 characters (the input buffer size). Strings longer than this throw exception <code>-20</code> (<em>parsed string overflow</em>).

<dt>maximum string length for <a href="https://forth-standard.org/standard/core/ENVIRONMENTq"><code>ENVIRONMENT?</code></a>, in characters
<dd>No limit.

<dt>method of selecting <a href="https://forth-standard.org/standard/usage#usage:input">3.2.4.1 User input device</a>
<dd>Configured at compilation time. There is no runtime selection mechanism.

<dt>method of selecting <a href="https://forth-standard.org/standard/usage#usage:output">3.2.4.2 User output device</a>
<dd>Configured at compilation time. There is no runtime selection mechanism.

<dt>methods of dictionary compilation (<a href="https://forth-standard.org/standard/usage#usage:dict">3.3 The Forth dictionary</a>)
<dd>Sequential. The dictionary is a singly-linked list. Each header contains a <code>link</code> field pointing to the previous entry.

<dt>number of bits in one address unit (<a href="https://forth-standard.org/standard/usage#usage:addr">3.1.3.3 Addresses</a>)
<dd>8

<dt>number representation and arithmetic (<a href="https://forth-standard.org/standard/usage#usage:number">3.2.1.1 Internal number representation</a>)
<dd>Two's complement, native word width (<code>usize</code>/<code>isize</code>). Arithmetic overflow wraps.

<dt>ranges for <em>n</em>, <em>+n</em>, <em>u</em>, <em>d</em>, <em>+d</em>, and <em>ud</em> (<a href="https://forth-standard.org/standard/usage#usage:cell">3.1.3 Single-cell types</a>, <a href="https://forth-standard.org/standard/usage#usage:2cell">3.1.4 Cell-pair types</a>)``
<dd>Depends on the target platform. Query <code>MAX-N</code>, <code>MAX-U</code>, <code>MAX-D</code>, <code>MAX-UD</code>.

<dt>read-only data-space regions (<a href="https://forth-standard.org/standard/usage#usage:dataspace">3.3.3 Data space</a>)
<dd>None. All data-space addresses below the the stack region are writable.

<dt>size of buffer at <a href="https://forth-standard.org/standard/core/WORD"><code>WORD</code></a> (<a href="https://forth-standard.org/standard/usage#usage:transient">3.3.3.6 Other transient regions</a>)
<dd>No specific buffer. <code>word</code> writes its result at <code>here</code>.

<dt>size of one cell in address units (<a href="https://forth-standard.org/standard/usage#usage:cell">3.1.3 Single-cell types</a>)
<dd>4 on 32-bit, 8 on 64-bit

<dt>size of one character in address units (<a href="https://forth-standard.org/standard/usage#usage:char">3.1.2 Character types</a>)
<dd>1

<dt>size of the keyboard terminal input buffer (<a href="https://forth-standard.org/standard/usage#usage:inbuf">3.3.3.5 Input buffers</a>)
<dd>256 bytes

<dt>size of the pictured numeric output string buffer (<a href="https://forth-standard.org/standard/usage#usage:transient">3.3.3.6 Other transient regions</a>)
<dd>64 bytes by default. Configured at compilation time.

<dt>size of the scratch area whose address is returned by <a href="https://forth-standard.org/standard/core/PAD"><code>PAD</code></a> (<a href="https://forth-standard.org/standard/usage#usage:transient">3.3.3.6 Other transient regions</a>)
<dd>84 bytes by default. Configured at compilation time.

<dt>system case-sensitivity characteristics (<a href="https://forth-standard.org/standard/usage#usage:find">3.4.2 Finding definition names</a>)
<dd>See <a href="#charset-match">character-set-extensions matching characteristics</a>.

<dt>system prompt (<a href="https://forth-standard.org/standard/usage#usage:dict">3.3 The Forth dictionary</a>, <a href="https://forth-standard.org/standard/core/QUIT"><code>QUIT</code></a>)
<dd><code>ok</code> in interpretation state. No prompt in compilation state.

<dt>type of division rounding (<a href="https://forth-standard.org/standard/usage#usage:div">3.2.2.1 Integer division</a>)
<dd>Symmetric (<code>sm/rem</code>) by default.

<dt>values of <a href="https://forth-standard.org/standard/core/STATE"><code>STATE</code></a> when true
<dd>&minus;1

<dt>values returned after arithmetic overflow (<a href="https://forth-standard.org/standard/usage#usage:intops">3.2.2.2 Other integer operations</a>)
<dd>Arithmetic operations wrap. Division by zero throws <code>-10</code> (<em>divison by zero</em>).

<dt>whether the current definition can be found after <a href="https://forth-standard.org/standard/core/DOES"><code>DOES></code></a> (<a href="https://forth-standard.org/standard/core/Colon"><code>:</code></a>)
<dd>Yes. <code>does></code> only patches the <code>DoCreate</code> operand of the latest word. It does unlink or hide it.
</dl>
