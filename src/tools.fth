variable (dump-start)
variable (dump-width)
variable (dump-end)
8 constant (dump-group)

: (green) $1b emit ." [32m" ;
: (blue) $1b emit ." [36m" ;
: (dim) $1b emit ." [2m" ;
: (sgr0) $1b emit ." [0m" ;

: (is-ascii-graphic?) $20 $7f within ;
: (dump-nibble) ( n -- char ) $f and dup 9 > if 87 + else 48 + then emit ;
: (dump-byte) ( n -- ) dup 4 rshift (dump-nibble) (dump-nibble) ;

: (emit-byte)
  dup 0= if
    (dim) (dump-byte) (sgr0)
  else
    dup (is-ascii-graphic?) if
      (green) (dump-byte) (sgr0)
    else
      dup $ff = if
        (blue) (dump-byte) (sgr0)
      else
        (dump-byte)
      then
    then
  then
;

: (dump-addr) ( addr -- ) 0 <# (dump-width) @ 0 ?do # loop #> type ;

: (dump?) ( addr -- addr flag ) dup (dump-end) @ u< ;

: (emit-ascii) ( char -- )
  dup 0= if drop (dim) [char] . emit (sgr0) exit then
  dup (is-ascii-graphic?) if (green) emit (sgr0) exit then
  dup $ff = if drop (blue) [char] . emit (sgr0) exit then
  drop [char] . emit ;

: (dump-row) ( addr -- )
  cr
  dup (dump-addr) 2 spaces
  dup >r
  dup 16 + swap
  ?do
    i (dump-start) @ -
    dup (dump-group) mod 0= swap 16 mod 0<> and if space then
    i (dump-end) @ u< if
      i c@ (emit-byte) space
    else
      3 spaces
    then
  loop
  space
  r>
  dup 16 + swap
  ?do
    i (dump-end) @ u< if i c@ (emit-ascii) else space then
  loop
;

\ Count digits used to represent number in the current base.
: (digits) ( u -- n ) 0 <# #s #> nip ;

: dump ( addr u )
  base @ >r
  hex
  over (dump-start) ! ( addr u )
  over +              ( addr end )
  dup (dump-end) !    ( addr end )
  \ Store minimum number of digits to render hex addresses (minimum: 4).
  dup 1- (digits) 4 max (dump-width) !
  swap                ( end addr )
  ?do i (dump-row) 16 +loop
  r> base !
  cr
;

: (>name) ( xt -- c-addr u )
  2 cells - dup c@      ( info-addr len )
  swap 1 cells - over - ( len name-addr )
  swap                  ( name-addr len )
;

: (dump-header) ( xt -- )
  dup (>name) drop 1-         ( xt name-addr )
  1 cells 1- invert and       ( xt name-addr' )
  swap 1 cells +              ( name-addr' body-start )
  over -                      ( name-addr' header-size )
  dump
;

: (dump-body) ( xt -- )
  dup 1 cells +         ( xt body-start )
  swap (body-len)       ( body-start body-len )
  dump
;

: (dump-word) ( xt -- )
  dup (>name) drop 1-         ( xt name-addr )
  1 cells 1- invert and       ( xt name-addr' )
  swap                        ( name-addr' xt )
  dup 1 cells +               ( name-addr' xt body-start )
  swap (body-len) +           ( name-addr' body-end )
  over -                      ( name-addr' size )
  dump
;

: words
  cr
  (latest) @
  begin dup 0<> while
    dup (flags-addr) c@ (hidden-flag) and 0= if
      dup (>name) type space
    then
    1 cells - @
  repeat
;

: ? ( a-addr -- ) @ 0 <# #s #> type space ;

: (opcode) ( xt -- op ) @ $ff and ;

: (.xt) ( xt -- ) (>name) type space ;

: (see-instr) ( ip -- next)
  dup @ swap (decode)   ( x op operand next )
  >r                    ( x op operand ) ( R: next )
  \ Literal. Display value.
  over ['] (lit)   (opcode) = if nip nip . r> exit then
  \ Call. Display name of target.
  over ['] (call)  (opcode) = if nip nip (.xt) r> exit then
  \ Yield. Extract packed XT and display name of source definition.
  over ['] (yield) (opcode) = if 2drop 16 rshift (.xt) r> exit then
  \ Str. Display literal string as it would be typed by the user.
  over ['] (s")    (opcode) = if
    nip nip             ( len ) ( R: next )
    r@ over aligned -   ( len addr )
    [char] s emit [char] " emit space
    swap type [char] " emit space
    r> exit
  then
  \ All other instructions fall though. Primitive and builtin instructions
  \ encode their opcode in the lowest byte and then pack their defining XT into
  \ the same cell.
  \ The `Jmp` patched into `;` for tail-call optimization *does not* pack a
  \ source XT into the cell.
  rot 8 rshift          ( op operand xt )
  ?dup if               ( op operand xt )
    \ This is a normal packed instruction.
    (.xt) 2drop
  else                  ( op operand )
    \ A TCO `Jmp`. The packed `xt == 0`. The operand is an XT.
    (.xt) drop
  then
  r>                    ( next )
;

: (see-colon) ( xt -- )
  dup (>name) [char] : emit space type cr 2 spaces  ( xt )
  \ Get address of last cell (Exit).
  dup (body-len) 1 cells - over +                   ( xt last )
  swap                                              ( last ip )
  begin 2dup swap u< while                          ( last ip )
    \ ip < last
    (see-instr)                                     ( last next )
  repeat
  2drop cr [char] ; emit space
;

: see ( "<spaces>name" )
  parse-name (find)                         ( c-addr u -- 0 | xt -1 | xt 1 )
  0<> if                                    ( xt )
    dup (flags-addr) c@ %1011000 and 0= if  ( xt )
      (see-colon)
    else
      s" builtin " type (>name) type cr
    then
  else
    s" undefined word" type cr abort
  then
;
