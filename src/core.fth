\ core word set
\ <https://forth-standard.org/standard/core>
\
\ This word set must only contain words belonging to the core set, not core-ext.
\ Exceptions: 2r>, 2>r, needed to implement 2over

: true -1 ;
: false 0 ;

: decimal #10 base ! ;
: hex $10 base ! ;

: negate invert 1 + ;

: * ( n1 n2 -- n3 ) um* drop ;

: 1+ 1 + ;
: 1- -1 + ;
: 2* 2 * ;

: = xor 0= ;
: <> = 0= ;
: 0<> 0= invert ;

: cell+ 1 cells + ;
: c,  here c! 1 allot ;
: >body 2 cells + ;
: does> r> (latest) @ cell+ ! ;
: chars ( -- ) ;

: until ['] (jmpz) compile, , ; immediate (compile-only)
: again ['] (jmp) compile, , ; immediate (compile-only)

: [ false state ! ; immediate (compile-only)
: ] true state ! ;

: > swap < ;
: 0> 0 > ;

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
  ['] swap compile, ['] >r compile, ['] >r compile,
; immediate (compile-only)
: 2r> ( -- x1 x2 ) ( R: x1 x2 -- )
  ['] r> compile, ['] r> compile, ['] swap compile,
; immediate (compile-only)
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

: /mod ( n1 n2 -- n3 n4 ) >r s>d r> sm/rem ;
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
: depth (sp@) (sp0) @ swap - 1 cells / ;
: 2@ dup cell+ @ swap @ ;
: 2! swap over ! cell+ ! ;

\ Copy u consecutive bytes from addr1 to addr2.
\
\ Works like memmove(3).
: move ( addr1 addr2 u -- )
  >r 2dup u< r> swap if
    \ addr2 > addr1. Copy from top to bottom to avoid overwriting the source.
    dup >r            ( addr1 addr2 u ) ( R: u )
    + swap r@ + swap  ( addr1+u addr2+u ) ( R: u )
    r>                ( addr1+u addr2+u u )
    begin
      dup             ( a1 a2 u u )
    while
      >r              ( a1 a2 ) ( R: u )
      1- swap 1- swap ( a1-1 a2-1 )
      over c@         ( a1-1 a2-1 char )
      over c!         ( a1-1 a2-1 )
      r> 1-           ( a1-1 a2-1 u-1 ) ( R: )
    repeat
  else
    \ addr1 > addr2. Copy from bottom to top.
    begin
      dup       ( addr1 addr2 u u )
    while       ( addr1 addr2 u )
      >r        ( addr1 addr2 ) ( R: u )
      over c@   ( addr1 addr2 char )
      over c!   ( addr1 addr2 )
      swap 1+   ( addr2 addr1+1 )
      swap 1+   ( addr1+1 addr2+1 )
      r> 1-     ( addr1+1 addr2+1 u-1 )
    repeat
  then
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
: char ( "<spaces>name>" -- char )
  bl word
  dup c@ 0= if -16 throw then
  char+ c@
;
: [char] char postpone literal ; immediate (compile-only)

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

\ Free-field number display
\   https://forth-standard.org/standard/usage#subsubsection.3.2.1.3
\   https://www.jimbrooks.org/programming/forth/forthPicturedNumericOutput.php
variable hld
: pad ( -- c-addr ) here (/hold) + ;
: hold ( char -- )
  hld @ 1-   ( char hld-1 )
  dup hld !  ( char hld-1 )
  c!         ( )
;
: (digit) ( n -- char ) dup 9 > if 55 + else 48 + then ; \ ( 55 = 'A' - 10, 48 = '0')
: # ( ud1 -- ud2 )
  ( lo hi )
  0 base @      ( lo hi 0 base )
  um/mod >r     ( lo r1 ) ( R: qhi )
  base @        ( lo r1 base )
  um/mod        ( r0 qlo )
  swap          ( qlo r0 )
  (digit) hold  ( qlo )
  r>            ( qlo qhi ) ( R: )
;
: <# ( -- ) pad hld ! ;
: #s ( ud1 -- ud2 )
  begin
    #           ( ud2 )
    2dup or 0=  ( ud2 flag )
  until
;
: #> ( xd -- c-addr u )
  2drop      ( )
  hld @      ( c-addr )
  pad        ( c-addr bufend )
  over -     ( c-addr u )
;
: sign ( n -- ) 0< if [char] - hold then ;

\ Toggles between 0 and 1.
variable (buf-addr-id)
\ Return an available transient buffer address.
: (buf-addr) ( -- c-addr )
  (buf-addr-id) @               ( id )
  dup (buf-size) * (buf-base) + ( id buf-addr )
  swap 1 xor (buf-addr-id) !    ( buf-addr )
;
: s"
  state @ if
    ['] (s") compile,
    [char] " parse  ( src len )
    dup ,           \ compile len
    dup allot       \ reserve space for string bytes
    here over -     ( src len string_start )
    swap            ( src string_start len )
    move
    align
  else
    [char] " parse    ( src len )
    dup >r            ( R: len )
    (buf-addr) dup >r ( src len buf-addr ) ( R: len buf-addr )
    swap move         ( )
    r> r>             ( buf-addr len ) ( R: )
  then
; immediate

\ dot commands
: u. ( u -- ) 0 <# #s #> type space ;
: . ( n -- ) dup abs 0 <# #s rot sign #> type space ;
: ." ( C: "ccc<quote>" -- ) ( -- ) postpone s" postpone type ; immediate (compile-only)

: recurse (latest) @ compile, ; immediate (compile-only)

\ ==============================================================================
\ LOOPS
\ ==============================================================================

\ TODO: Document.
variable (leave-list)

: do ( n1 n2 -- ) ( R: -- loop-sys )
  ['] (do) compile,
  (leave-list) @
  0 (leave-list) !
  here
; immediate (compile-only)

: +loop
  postpone (+loop)
  ,
  (leave-list) @ begin ?dup while
    dup @
    swap here swap !
  repeat
  (leave-list) !
; immediate (compile-only)

: loop 1 postpone literal postpone +loop ; immediate (compile-only)

: leave
  postpone unloop
  postpone (jmp)
  (leave-list) @ ,
  here 1 cells -
  (leave-list) !
; immediate (compile-only)

: ?do ( n1|u1 n2|u2 -- ) ( R: -- loop-sys )
  ['] (?do) compile,
  (leave-list) @
  0 (leave-list) !
  (leave-list) @ ,
  here 1 cells -
  (leave-list) !
  here
; immediate (compile-only)

: fill ( c-addr u char -- ) rot rot 0 ?do 2dup c! char+ loop 2drop ;

: accept ( c-addr +n1 -- +n2 )
  over swap         ( start ptr )
  0 ?do
    key             ( start ptr c )
    dup $0a = if    \ if c == '\n'
      drop leave      ( start ptr )
    then
    over c! char+   ( start ptr+1 )
  loop              ( start ptr )
  swap -            ( n2 )
;

: .s ( -- )
  depth
  \ Print <depth>.
  [char] < emit dup 0 <# #s #> type [char] > emit space
  0 ?do (sp0) @ i cells - @ . loop
;

: evaluate ( i*x c-addr u -- j*x )
  \ Save input source specification.
  (source-addr) @ >r
  (source-len) @ >r
  >in @ >r
  \ Set input source to string.
  (source-len) !
  (source-addr) !
  0 >in !
  (interpret)
  \ Restore input source specification.
  r> >in !
  r> (source-len) !
  r> (source-addr) !
;

: (diagnostic)
  (diagnostic-len) @ ?dup if
    (diagnostic-addr) @ swap type
  then
;

: compare ( c-addr1 u1 c-addr2 u2 -- n )
  \ Save the length comparision value.
  rot                     ( addr1 addr2 len2 len1 )
  2dup > if -1 else
  2dup < if 1 else
  0
  then then >r            ( addr1 addr2 len2 len1 ) ( R: n )
  \ Compare byte-by-byte until minimum length.
  min                     ( addr1 addr2 min )
  begin
    dup                   ( addr1 addr2 min min )
  while                   ( addr1 addr2 min )
    >r                    ( addr1 addr2 ) ( R: n min )
    over c@               ( addr1 addr2 c1 )
    over c@               ( addr1 addr2 c1 c2 )
    2dup <> if
      < if -1 else 1 then ( addr1 addr2 n )
      >r 2drop r>         ( n )
      r> drop r> drop     ( n ) ( R: )
      exit
    then
    2drop                 ( addr1 addr2 )
    swap 1+ swap 1+       ( addr1' addr2' )
    r> 1-                 ( addr1' addr2' min' ) ( R: n )
  repeat
  2drop drop r>           ( n ) ( R: )
;

: environment? ( c-addr u -- false | i*x true )
  2dup s" /COUNTED-STRING"    compare 0= if 2drop (/counted-string)    true exit then
  2dup s" /HOLD"              compare 0= if 2drop (/hold)              true exit then
  2dup s" /PAD"               compare 0= if 2drop (/pad)               true exit then
  2dup s" ADDRESS-UNIT-BITS"  compare 0= if 2drop (address-unit-bits)  true exit then
  2dup s" FLOORED"            compare 0= if 2drop (floored)            true exit then
  2dup s" MAX-CHAR"           compare 0= if 2drop (max-char)           true exit then
  2dup s" MAX-D"              compare 0= if 2drop (max-d)              true exit then
  2dup s" MAX-N"              compare 0= if 2drop (max-n)              true exit then
  2dup s" MAX-U"              compare 0= if 2drop (max-u)              true exit then
  2dup s" MAX-UD"             compare 0= if 2drop (max-ud)             true exit then
  2dup s" RETURN-STACK-CELLS" compare 0= if 2drop (return-stack-cells) true exit then
  2dup s" STACK-CELLS"        compare 0= if 2drop (stack-cells)        true exit then
  2drop false
;

: quit
  (rp0) @ (rp!)
  0 (source-id) !
  postpone [
  begin
    refill
  while
    ['] (interpret) catch
    ?dup if
      \ -1 (ABORT) and -56 (QUIT) silently return to the prompt.
      dup -1 = over -56 = or 0= if
        dup -2 = if drop (diagnostic) else
        dup -3 = if drop ." stack overflow " else
        dup -4 = if drop ." stack underflow " else
        dup -5 = if drop ." return stack overflow " else
        dup -6 = if drop ." return stack underflow " else
        dup -9 = if drop ." invalid memory address " else
        dup -10 = if drop ." division by zero " else
        dup -13 = if drop ." undefined word: " (diagnostic) else
        dup -14 = if drop ." interpreting a compile-only word" else
        dup -16 = if drop ." attempt to use zero-length string as a name" else
        dup -19 = if drop ." definition name too long: " (diagnostic) else
        dup -20 = if drop ." parsed string overflow " else
        dup -21 = if drop ." unsupported operation " else
        ." error: " . then then then then then then then then then then then then then cr
      else drop then
      \ Reset compilation state and clear stack.
      postpone [
      (sp0) @ (sp!)
    else
      state @ 0= if ." ok" cr then
    then
  repeat
;

: abort ( i*x -- ) -1 throw ;

: (abort") ( flag c-addr u -- )
  rot if (diagnostic!) -2 throw else 2drop then
;

: abort" postpone s" postpone (abort") ; immediate (compile-only)

\ TODO: Move to block set later.
: (load) begin refill while (interpret) repeat ;
