: source (source-addr) @ (source-len) @ ;
: invert dup (nand) ;
: or invert swap invert (nand) ;
: immediate (latest) @ (flags-addr) dup c@ (immediate-flag) or swap c! ;
: \ source >in ! drop ; immediate

\ KERNEL
\ ======
\
\ Before executing this file, the outer interpreter defines the primitive words
\ (opcodes and builtins) and hand-compiles basic versions of the Forth compiler
\ words.
\
\ The kernel has two functions:
\
\   1. Patch incomplete versions of compiler words. The basic version of :, for
\      example, does not set the hidden flag. The kernel replaces it immediately
\      with a version that does.
\   2. Bootstrap the interpreter. The first part of the boot defines (interpret)
\      and its dependencies.
\
\ Defines the minimum set of words required to define (interpret).
\ In the future we will minimize it even further: inline definitions (e.g. for
\ bitwise operations) and strip comments before execution.
\
\ Dependencies
\ ============
\
\ Opcodes
\ -------
\ 0< @ (jmpz) (jmp) swap ! drop dup + c@ execute >r r> (nand) c!
\
\ Builtins
\ --------
\ ' postpone parse (find) (number?)
\
\ Variables
\ ---------
\ (latest) (flags-addr) (here) (source-addr) (source-len) >in state
\
\ Hand-compiled
\ -------------
\ : ; , (cells, !+, allot) literal (hidden-flag) (immediate-flag)

\ 1. PATCH
\ ========

: ['] ' postpone literal ; immediate
: :
  $20 parse (header)
  (latest) @ (flags-addr) dup c@ (hidden-flag) or swap c!
  ['] (docol) @ ,
  -1 state !
;

\ 2. DEFINING WORDS
\ ================
\
\ Provided by the kernel so that every layer above (core, core-ext, tools, and
\ anything Fe loads) has the defining words and the exception mechanism.

: ( $29 parse drop drop ; immediate

: and (nand) invert ;
: - invert 1 + + ;

: bl $20 ;
: here (here) @ ;
: aligned ( addr -- a-addr ) 1 cells -1 + + 1 cells -1 + invert and ;
: align ( -- ) here aligned here - allot ;
: create bl parse (header) ['] (docreate) @ , 0 , ;
: constant >r : r> postpone literal postpone ; ;
: variable align here 0 , constant ;

: if ['] (jmpz) , here 0 , ; immediate
: then here swap ! ; immediate
: else ['] (jmp) , here 0 , swap here swap ! ; immediate

: ?dup dup if dup then ;

\ 3. EXCEPTIONS
\ =============
\
\ catch/throw use a linked list of handler frames threaded through the return
\ stack. handler holds the return-stack pointer of the innermost frame.

variable handler

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

\ 4. (INTERPRET)
\ ==============

\ < does not need to be overflow safe here
: < - 0< ;

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

: over >r dup r> swap ;
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
      0< state @ and if , else execute then
    else                        ( c-addr u )
      (number?) if              ( n )
        state @ if postpone literal then \ Left on stack if in interpretation mode.
      else
        (diagnostic-len) ! (diagnostic-addr) !
        -13 throw
      then
    then
  repeat
  2drop
;
