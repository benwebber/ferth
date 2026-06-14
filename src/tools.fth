variable (dump-start)
variable (dump-width)
variable (dump-end)
8 constant (dump-group)

: (green) $1b emit ." [32m" ;
: (blue) $1b emit ." [36m" ;
: (dim) $1b emit ." [2m" ;
: (sgr0) $1b emit ." [0m" ;

: (dump-nibble) ( n -- char ) $f and dup 9 > if 87 + else 48 + then emit ;
: (dump-byte) ( n -- ) dup 4 rshift (dump-nibble) (dump-nibble) ;

: (emit-byte)
  dup 0= if
    (dim) (dump-byte) (sgr0)
  else
    dup $20 $7f within if
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
  dup $20 $7f within if (green) emit (sgr0) exit then
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

: (body-len) ( xt -- u ) 3 cells - @ ;

: (>name) ( xt -- c-addr u )
  2 cells - dup c@      ( info-addr len )
  swap 1 cells - over - ( len name-addr )
  swap                  ( name-addr len )
;

: (dump-header) ( xt -- )
  dup (>name) drop 1-         ( xt nfa )
  1 cells 1- invert and       ( xt nfa' )
  swap 1 cells +              ( nfa' body-start )
  over -                      ( nfa' header-size )
  dump
;

: (dump-body) ( xt -- )
  dup 1 cells +         ( xt body-start )
  swap (body-len)       ( body-start body-len )
  dump
;

: (dump-word) ( xt -- )
  dup (>name) drop 1-         ( xt nfa )
  1 cells 1- invert and       ( xt nfa' )
  swap                        ( nfa' xt )
  dup 1 cells +               ( nfa' xt body-start )
  swap (body-len) +           ( nfa' body-end )
  over -                      ( nfa' size )
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

: ? ( a-addr -- ) @ 0 <# #s #> type ;

: (has-inline-cells?) ( xt -- n )
  dup ['] (lit)   = if drop 1 exit then
  dup ['] (jmp)   = if drop 1 exit then
  dup ['] (jmpz)  = if drop 1 exit then
  dup ['] (+loop) = if drop 1 exit then
  dup ['] (?do)   = if drop 1 exit then
  drop 0
;

: see ( "<spaces>name" )
  bl parse (find)                         ( c-addr u -- 0 | xt -1 | xt 1 )
  0<> if                                  ( xt )
    cr
    dup @ ['] (docol) @ = if              ( xt )
      \ This is a colon definition.
      dup (>name)                         ( xt name-addr len )
      [char] : emit space type cr         ( xt )
      2 spaces
      \ Iterate from the start of the body to the cell before the last (exit).
      dup cell+ dup rot (body-len) 1 cells - + swap ( body' body )
      do
        i @                               ( xt )
        dup ['] (s") = if
          \ Skip variable length string.
          drop
          s" (s" type [char] " emit [char] ) emit space \ (s")
          i cell+ @ aligned 2 cells +
        else
          dup (has-inline-cells?) if
            \ This word has a body parameter.
            drop i cell+ @ .
            2 cells
          else
            (>name) type space 1 cells
          then
        then
      +loop
      cr
      [char] ; emit
    else
      s" builtin " type (>name) type cr
    then
  else                              ( )
    s" undefined word" type cr
    abort
  then
;
