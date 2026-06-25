: source (source-addr) @ (source-len) @ ;
: invert dup (nand) ;
: - invert 1 + + ;
: (flags-addr) 2 cells - 1 + ;
: or invert swap invert (nand) ;
: (immediate-flag) %001 ;
: immediate (latest) @ (flags-addr) dup c@ (immediate-flag) or swap c! ;
: \ source >in ! drop ; immediate

\ KERNEL
\ ======
\
\ Before executing this file, the bootloader has defined primitive words
\ (opcodes and builtins) as well as hand-compiled versions of the Forth compiler
\ words.
\
\ The kernel has multiple stages:
\
\   1. Define defining words
\   2. Define exception handler words
\   3. Implement tail-call optimization
\   4. Define parse-name
\   5. Define words that depend on parse-name
\   6. Define postpone
\   7. Define words that depend on postpone
\   8. Define diagnostic checks
\   9. Define (interpret)
\
\ Once (interpret) exists, Forth takes over execution.
\
\ In the future we will minimize it even further: inline definitions (e.g. for
\ bitwise operations) and strip comments before execution.
\
\ At this stage, input is trusted, so we can use shortcuts like` parse` instead
\ of the more correct `parse-name`. We tag words that need to be replaced with
\ correct definitions with `(bootstrap)`. A final diagnostic check ensures every
\ word tagged so has been hidden and replaced.

\ 1. DEFINING WORDS
\ =================

: ( $29 parse drop drop ; immediate

\ TODO: Figure out consistent vocabulary for compiler directives.
: (hidden-flag) %010 ;
: (hide) (flags-addr) dup c@ (hidden-flag) or swap c! ;
: (bootstrap) (latest) @ (flags-addr) dup c@ %100 or swap c! ;
: (set-create) (latest) @ (flags-addr) dup c@ %01000000 or swap c! ;

: bl $20 ;
: here (here) @ ;
: aligned ( addr -- a-addr ) 1 cells -1 + + 1 cells -1 + invert and ;
: align ( -- ) here aligned here - allot ;
: create
  bl parse (header) (set-create) ['] (docreate) @ , 0 ,
; (bootstrap) \ Replace with parse-name.

\ (rp@) points to the next cell, and (docol) pushes a call frame onto the stack.
: r@ (rp@) 2 cells - @ ;

: if ['] (jmpz) compile, here 0 , ; immediate
: then here swap ! ; immediate
: else ['] (jmp) compile, here 0 , swap here swap ! ; immediate
: exit ['] (exit) compile, ; immediate

\ 2. EXCEPTIONS
\ =============
\
\ catch/throw use a linked list of handler frames threaded through the return
\ stack. handler holds the return-stack pointer of the innermost frame.
\ variable is not defined yet
create handler 0 ,

: catch ( xt -- 0 | n )
  \ Save data stack depth.
  (sp@) >r
  \ Save enclosing handler.
  handler @ >r
  \ Make this frame the current handler.
  (rp@) handler !
  \ Execute XT.
  execute
  \ Success: restore enclosing handler.
  r> handler !
  \ Drop saved SP and exit with success (0).
  r> drop 0
;

: throw ( n -- )
  ?dup if
    \ Unwind the return stack to the saved handler frame.
    handler @ (rp!)
    \ Restore the enclosing handler.
    r> handler !
    \ Pop saved SP from the return stack, push n to the return stack.
    r> swap >r
    \ Restore data stack depth.
    (sp!)
    \ Discard saved XT. Leave n on top.
    drop r>
  then
;

: (diagnostic!) (diagnostic-len) ! (diagnostic-addr) ! ;

\ 3. TAIL-CALL OPTIMIZATION
\ =========================
\
\ In this token-threaded system, all words share the same structure in memory.
\ Specifically, the word's XT points to its `code` field, which is always a
\ literal opcode. Some opcodes, such as `Lit` and `Call` take operands, stored
\ consecutively in memory.
\
\ There is no DOCOL instruction to enter the word. Instead, `Call` immediately
\ jumps to the target word's `code` field and executes it until it hits `Exit`.
\
\ A primitive looks like:
\
\         name      bodylen        code
\         v         v              v
\     ...[3]["dup"][2][info][link][Dup][Exit]
\
\ A colon definition composed of primitives looks like:
\
\     : over >r dup r> swap ;
\
\         name       bodylen        code
\         v          v              v
\     ...[4]["over"][4][info][link][ToR][Dup][RFrom][Swap][Exit]
\
\ A word that calls other colon definitions looks like:
\
\     : ?dup dup if dup then ;
\
\         name       bodylen        code
\         v          v              v
\     ...[4]["?dup"][4][info][link][Dup][Call]['if][Dup][Call]['then][Exit]
\
\ Where `'if` and `'then` are those words' XTs.
\
\ This permits simple tail-call optimization. If the last instruction in a
\ definition is a `Call`, we would normally push a stack frame to enter that
\ word. Not only does this incur extra work, but deep recursion will overflow
\ the return stack.  Instead, we can easily patch the final `Call` to a `Jmp`.
\
\         name       bodylen        code                 patch
\         v          v              v                    v
\     ...[4]["?dup"][4][info][link][Dup][Call]['if][Dup][Jmp]['then][Exit]
\
\ This is safe for all words, and particularly useful for recursive words. Note
\ that this does leave a spurious, yet harmless, `Exit` at the end of the
\ definition.
\
\ Because this is safe for all words, we can redefine `;` to apply it to all new
\ definitions. See below.

: over >r dup r> swap ;

: xor over over and invert >r or r> and ;

\ Overflow-safe comparison operators.
\ TODO: Figure out how to store the current XT without ' and hide it later.
: <  ( n1 n2 -- flag ) 2dup xor 0< if      drop 0< else - 0< then ;
: u< ( u1 u2 -- flag ) 2dup xor 0< if swap drop 0< else - 0< then ;

: begin here ; immediate
: while ['] (jmpz) compile, here 0 , swap ; immediate
: repeat ['] (jmp) compile, , here swap ! ; immediate

\ Returns the length of the body in cells.
: (body-len) ( xt -- u ) 3 cells - @ ;

\ Calculate the size of the instruction at addr.
: (instr-size) ( addr -- n )
  \ All of these instructions take one operand, for two cells.
  \ TODO: Expose (call)?
  dup @ $25           - 0= if drop 2 cells exit then
  dup @ ['] (lit)   @ - 0= if drop 2 cells exit then
  dup @ ['] (jmp)   @ - 0= if drop 2 cells exit then
  dup @ ['] (jmpz)  @ - 0= if drop 2 cells exit then
  dup @ ['] (+loop) @ - 0= if drop 2 cells exit then
  dup @ ['] (?do)   @ - 0= if drop 2 cells exit then
  \ `Str` takes a variable length operand: `[Str][len][data...]`, for a total of
  \ `2 * SIZE + len(data)` bytes, aligned up.
  dup @ ['] (s")    @ - 0= if 1 cells + @ aligned 2 cells + exit then
  drop 1 cells
;

\ Return the address of a word's last instruction.
: (tail) ( xt -- addr )
  dup dup (body-len) + 1 cells - >r   ( xt ) ( R: exit-addr )
  dup                                 ( prev addr )
  \ Walk up the word's instructions until hitting `exit-addr`.
  \ The final address on the stack will be the address of the last instruction
  \ before `Exit`.
  begin
    dup r@ u<                         ( prev addr flag )
  while
    swap drop dup                     ( addr addr )
    dup (instr-size) +                ( prev addr )
  repeat
  drop r> drop                        ( prev ) ( R: )
;

: (tail-optimize) ( xt -- )
  \ Skip if body is less than three cells (probably a primitive).
  dup (body-len) 3 cells u< if drop exit then
  (tail) dup @ $25 - 0= if
    \ Replace `Call` with `Jmp`. Dead `Exit` remains.
    ['] (jmp) @ swap !
  else
    drop
  then
;

\ 4. PARSE-NAME
\ =============

: parse-name ( "<spaces>name<space>" -- c-addr u )
  \ Skip leading whitespace characters.
  begin
    >in @ source  ( >in source-addr source-len )
    swap drop     ( >in source-len )
    < if
      source drop ( source-addr )
      >in @       ( source-addr >in )
      + c@        ( char )
      \ Skip all ASCII control and whitespace characters (up to and including
      \ BL/0x20/SPACE.)
      bl 1 + <     ( flag )
    else
      0
    then
  while
    1 >in +!
  repeat
  bl parse
;

\ 5. PARSE-NAME DEFINITIONS
\ =========================

: ' parse-name (find) 0= if (diagnostic!) -13 throw then ;

\ Hide and redefine bootstrap words.
' and (hide)
: and (nand) invert ;

' ?dup (hide)
: ?dup dup if dup then ;

\ Notice that to redefine :, we must first save the XT of :, because otherwise
\ the interpreter would throw undefined word (-13) on :.
' : dup dup     ( xt xt xt )
(hide)          ( xt xt )
execute :
  parse-name (header)
  (latest) @ (flags-addr) dup c@ (hidden-flag) or swap c!
  -1 state !
;
(hide)          ( xt -- )

' create (hide)
: create
  parse-name (header) (set-create) ['] (docreate) @ , 0 ,
;

\ Redefine `;` to call `(tail-optimize)` after executing. Compile the current XT
\ of `;` into the definition.
' ; ( xt )
: ; literal execute (latest) @ (tail-optimize) ; immediate ( )

\ Micro-optimization. The following defined words *can* use TCO.
' and (tail-optimize)
' aligned (tail-optimize)
' 2dup (tail-optimize)
' xor (tail-optimize)
' begin (tail-optimize)
' ' (tail-optimize)

\ 6. POSTPONE
\ ===========

\ postpone's definition is difficult to grasp because it fuses two different
\ times:
\
\   * T1: When postpone itself is compiled.
\   * T2: When `postpone foo` executes, i.e., when a word W is being compiled.
\
\ Immediate words execute at T1. Non-immediate words execute at T2. ['] moves
\ its operand from T1 to T2.
\
\ The end result is that `postpone foo` compiles ['(lit)][xt]['compile,] into W.
\ `(lit)` pushes `xt` and `compile,` compiles it.
: postpone ( "<spaces>name" -- )
  parse-name (find)                       ( c-addr u 0 | xt flag )
  dup 0= if (diagnostic!) -13 throw then  ( xt flag )
  0< if
    \ The word is not immediate.
    ['] (lit) compile, ,  \ Compile `'(lit) xt`
    ['] compile, compile, \ Compile `'compile,` a call to `compile,`
  else
    \ The word is immediate.
    compile,
  then
; immediate

\ 7. POSTPONE DEFINITIONS
\ =======================

: constant >r : r> postpone literal postpone ; ;
: variable align here 0 , constant ;

' ['] (hide)
: ['] parse-name (find) drop postpone literal ; immediate

\ 7. DIAGNOSTIC CHECKS
\ ====================

\ Check that we have redefined all hand-compiled words.
: (check-bootstrap)
  (latest) @                          ( latest )
  begin
    dup 0= invert                     ( latest flag )
  while                               ( latest )
    dup (flags-addr) c@ %100 and      ( latest bootstrap )
    over (flags-addr) c@ %010 and 0=  ( latest bootstrap !hidden )
    and if
      drop                            ( )
      -256 throw
    then
    1 cells - @                       ( link )
  repeat
  drop
;

\ 9. (INTERPRET)
\ ==============

: rot >r swap r> swap ;

' 2dup (hide)
: 2dup over over ;

' 2drop (hide)
: 2drop drop drop ;

' 2swap (hide)
: 2swap rot >r rot r> ;

' (interpret) ( xt )
: (interpret)
  begin
    parse-name                  ( c-addr u )
  dup while                     ( c-addr u )
    2dup (find) ?dup if         ( c-addr u xt flag )
      2swap 2drop               ( xt flag )
      0< state @ and if compile, else execute then
    else                        ( c-addr u )
      (number?) if              ( n )
        state @ if postpone literal then \ Left on stack if in interpretation mode.
      else
        (diagnostic!) -13 throw
      then
    then
  repeat
  2drop
;
(hide)        ( xt -- )
