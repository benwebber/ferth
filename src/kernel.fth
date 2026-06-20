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
\ The kernel has 8 stages:
\
\   1. Define defining words.
\   2. Define exception handler words.
\   3. Define parse-name
\   4. Define words that depend on parse-name
\   5. Define postpone
\   6. Define words that depend on postpone
\   7. Define diagnostic checks
\   8. Define (interpret)
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

: and (nand) invert ;

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

: if ['] (jmpz) , here 0 , ; immediate
: then here swap ! ; immediate
: else ['] (jmp) , here 0 , swap here swap ! ; immediate

: ?dup dup if dup then ;

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

\ 3. PARSE-NAME
\ =============

\ < does not need to be overflow safe here
: < - 0< ; (bootstrap)

: begin here ; immediate
: while ['] (jmpz) , here 0 , swap ; immediate
: repeat ['] (jmp) , , here swap ! ; immediate

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

\ 4. PARSE-NAME DEFINITIONS
\ =========================

: ' parse-name (find) 0= if (diagnostic!) -13 throw then ;

\ Hide and redefine bootstrap words. Notice that to redefine :, we must first
\ save the XT of :, because otherwise the interpreter would throw undefined word
\ (-13) on :.
' : dup dup >r  ( xt xt ) ( R: xt )
(hide)          ( xt )
execute :
  parse-name (header)
  (latest) @ (flags-addr) dup c@ (hidden-flag) or swap c!
  ['] (docol) @ ,
  -1 state !
;
r> (hide)       ( R: )

' create (hide)
: create
  parse-name (header) (set-create) ['] (docreate) @ , 0 ,
;

\ 5. POSTPONE
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
    ['] (lit) , ,         \ Compile `'(lit) xt`
    ['] compile, compile, \ Compile `'compile,` a call to `compile,`
  else
    \ The word is immediate.
    compile,
  then
; immediate

\ 6. POSTPONE DEFINITIONS
\ =======================

: constant >r : r> postpone literal postpone ; ;
: variable align here 0 , constant ;

' ['] (hide)
: ['] parse-name (find) drop postpone literal ; immediate

\ 7. DIAGNOSTIC CHECKS
\ ====================

: over >r dup r> swap ;

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

\ 8. (INTERPRET)
\ ==============

: rot >r swap r> swap ;
: 2dup over over ;
: 2drop drop drop ;
: 2swap rot >r rot r> ;

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
