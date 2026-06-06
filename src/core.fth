: immediate (latest) @ (flags-addr) dup c@ (immediate-flag) or swap c! ;
: source (source-addr) @ (source-len) @ ;
: \ source >in ! drop ; immediate

\ core word set
\ <https://forth-standard.org/standard/core>
\
\ This word set must only contain words belonging to the core set, not core-ext.
\ Exceptions: 2r>, 2>r, needed to implement 2over

: ( $29 parse drop drop ; immediate

: bl $20 ;
: true -1 ;
: false 0 ;

: decimal #10 base ! ;
: hex $10 base ! ;

: over >r dup r> swap ;
: rot >r swap r> swap ;

\ : invert dup (nand) ;
\ : and (nand) invert ;
\ : or invert swap invert (nand) ;
: xor over over and invert >r or r> and ;
: negate invert 1 + ;

: * ( n1 n2 -- n3 ) um* drop ;

: 1+ 1 + ;
: 1- -1 + ;
\ : - invert 1+ + ;
: 2* 2 * ;

: = xor 0= ;
: <> = 0= ;
: 0<> 0= invert ;

: +! dup >r @ + r> ! ;

: here (here) @ ;
\ : allot (here) +! ;
: cell+ 1 cells + ;
\ : aligned 1 cells 1- + 1 cells 1- invert and ;
\ : align here aligned here - allot ;
\ : , align here ! 1 cells allot ;

\ : [ false state ! ; immediate
\ : ] true state ! ;
: ['] ' postpone literal ; immediate
: c,  here c! 1 allot ;
: >body 1 cells + ;

: exit ['] (exit) , ; immediate

: if ['] (jmpz) , here 0 , ; immediate
: then here swap ! ; immediate
: else ['] (jmp) , here 0 , swap here swap ! ; immediate

: begin here ; immediate
: until ['] (jmpz) , , ; immediate
: again ['] (jmp) , , ; immediate
: while ['] (jmpz) , here 0 , ; immediate
: repeat ['] (jmp) , swap , here swap ! ; immediate

: constant >r : r> postpone literal postpone ; ;
: variable align here 0 , constant ;

: 2dup over over ;
: 2drop drop drop ;
: 2swap rot >r rot r> ;

\ Overflow-safe comparison operators.
\ TODO: explain
: <  ( n1 n2 -- flag ) 2dup xor 0< if      drop 0< else - 0< then ;
: u< ( u1 u2 -- flag ) 2dup xor 0< if swap drop 0< else - 0< then ;
: > swap < ;

: ?dup ( x -- 0 | x x ) dup if dup then ;
: abs dup 0< if negate then ;
: min ( n1 n2 -- n3 ) 2dup < if drop else swap drop then ;
: max ( n1 n2 -- n3 ) 2dup > if drop else swap drop then ;

\ TODO: Compare bitwise alternative.
: dnegate ( d1 -- d2 )
                                      ( dlo dhi )
  swap negate swap                    ( dlo' dhi )
  \ If dlo' is 0, negating the double is the same as negating the high cell.
  \ Otherwise invert the high cell.
  over 0= if negate else invert then  ( dlo' dhi' )
;
: dabs ( d1 -- d2 ) dup 0< if dnegate then ;

: s>d dup 0< if -1 else 0 then ;

\ um/mod
\   \_ sm/rem
\        \_ fm/mod
\        \_ / /mod */ */mod
: sm/rem ( d1 n1 -- n2 n3 )
  \ Save dividend sign (high cell).
  over >r               ( dlo dhi ) ( R: dhi )
  \ Save quotient sign (hi XOR n1 ).
  2dup xor >r           ( dlo dhi n ) ( R: dhi q' )
  \ Calculate magnitudes.
  abs >r dabs r>        ( dlo_u dhi_u u )
  um/mod                ( r_u q_u )
  \ Apply quotient sign.
  r> 0< if negate then  ( r_u q_n ) ( R: dhi )
  swap                  ( q_n r_u )
  \ Apply remainder sign.
  r> 0< if negate then  ( q_n r_n ) ( R: )
  swap                  ( r_n q_n )
;

: 2>r ( x1 x2 -- ) ( R: -- x1 x2 )
  ['] swap , ['] >r , ['] >r ,
; immediate
: 2r> ( -- x1 x2 ) ( R: x1 x2 -- )
  ['] r> , ['] r> , ['] swap ,
; immediate
: 2over ( x1 x2 x3 x4 -- x1 x2 x3 x4 x1 x2 )
  2>r 2dup 2r> 2swap
;

: fm/mod ( d1 n1 -- n2 n3 )
  \ Save divisor.
  dup >r              ( dlo dhi n ) ( R: n )
  \ Save sign of quotient (hi XOR n).
  over r@ xor >r      ( dlo dhi n ) ( R: dhi q' )
  sm/rem              ( r_s q_s )
  over 0<> r> 0< and if
    \ Floor if signs differ and remainder is non-zero.
    1- swap r> + swap ( r+n q-1 )
  else
    \ Discard divisor.
    r> drop
  then
;

: /mod ( n1 n2 -- n2 n3 ) >r s>d r> sm/rem ;
: / /mod swap drop ;
: mod /mod drop ;
: m* ( n1 n2 -- d )
  \ Save negative flag.
  2dup xor 0< >r      ( R: flag )
  \ Multiply absolute magnitudes.
  abs swap abs        ( u2 u1 )
  um*                 ( ud )
  \ Adust sign.
  r> if dnegate then
;
: */mod ( n1 n2 n3 -- n4 n5 ) >r m* r> sm/rem ;
: */ ( n1 n2 n3 -- n4 ) */mod swap drop ;

: 2/ dup 1 rshift swap 0 invert 1 rshift invert and or ;
: depth (sp@) 1 cells / ;
: 2@ dup cell+ @ swap @ ;
: 2! swap over ! cell+ ! ;
: 2@ dup cell+ @ swap @ ;

: move ( addr1 addr2 u -- )
  begin
    dup       ( addr1 addr2 u u )
  while       ( addr1 addr2 u )
    >r        ( addr1 addr2 ) ( R: u )
    over c@   ( addr1 addr2 char )
    over c!   ( addr1 addr2 )
    swap 1+   ( addr2 addr1' )
    swap 1+   ( addr1+1 addr2+1 )
    r> 1-     ( addr1+1 addr2+1 u-1 )
  repeat
  ( addr1 addr2 0 )
  drop drop drop
;

: word ( char "<chars>ccc<char>" -- c-addr )
  \ Skip leading delimiters.
  >r ( R: delim )
  begin
    \ Current offset in source.
    >in @         ( pos )
    source        ( pos source-addr source-len )
    swap drop     ( pos source-len )
    \ Is there still input?
    < if          ( )
      source drop ( source-addr )
      >in @ +     ( pos' )
      \ Is the current character the delimiter?
      c@ r@ =     ( flag) ( R: delim )
    else
      \ No input left.
      0           ( flag )
    then
  \ Read until >in reaches a non-delimiter, or end of input.
  while
    1 >in +!
  repeat
  r> parse        ( c-addr u ) ( R: )
  \ Limit to 255 characters.
  dup 255 > if drop 255 then
  \ Store length byte.
  dup here c!     ( c-addr u )
  \ Copy string.
  >r here 1+ r>
  move
  here
;

: char+ 1 + ;
: char ( "<spaces>name>" -- char ) bl word char+ c@ ;
: [char] char postpone literal ; immediate

\ output
: cr ( -- ) $0a emit ;
: space ( -- ) bl emit ;
: spaces ( n -- ) 0 max begin dup while space 1- repeat drop ;
: count ( c-addr1 -- c-addr2 u ) dup 1+ swap c@ ;

: find ( c-addr -- c-addr 0 | xt 1 | xt -1 )
  dup count             ( caddr1 caddr2 u )
  (find)                ( caddr1 0 | caddr1 xt 1 | caddr1 xt -1 )
  dup if rot drop then  ( caddr1 0 | xt 1 | xt - 1 )
;

: type ( c-addr u -- ) begin dup while over c@ emit swap 1+ swap 1- repeat 2drop ;

: s"
  ['] (s") ,
  [char] " parse  ( src len )
  dup ,           \ compile len
  dup allot       \ reserve space for string bytes
  here over -     ( src len string_start )
  swap            ( src string_start len )
  move
  align
; immediate

\ ==============================================================================
\ LOOPS
\ ==============================================================================

variable (leave-chain)

: do
  ['] (do) ,
  (leave-chain) @
  0 (leave-chain) !
; immediate

: leave
  ['] r> , ['] drop ,
  ['] r> , ['] drop ,
  ['] r> , ['] drop ,
  ['] (jmp) ,
  (leave-chain) @ ,
  here 1 cells -
  (leave-chain) !
; immediate

: loop
  ['] (loop) ,
  (leave-chain) @ begin ?dup while
    dup @
    swap here swap !
  repeat
  (leave-chain) !
; immediate
