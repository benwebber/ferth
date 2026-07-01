: .( ( "ccc<paren>" -- ) [char] ) parse type ; immediate

: erase ( addr u ) 0 fill ;

: within ( u1 u2 u3 -- flag ) over - >r - r> u< ;

: pick ( xu...x1 x0 u -- xu...x1 x0 xu ) cells (sp@) swap + 2 cells + @ ;
: roll ( xu xu-1 ... x0 u -- xu-1 ... x0 xu )
  dup if swap >r 1- recurse r> swap exit then drop
;
: nip ( x1 x2 -- x2 ) swap drop ;
: tuck ( x1 x2 -- x2 x1 x2 ) swap over ;

: :noname ( C: -- colon-sys ) ( -- xt )
  here 0 (header)
  (latest) @
  -1 state !
;

: unused ( -- u )
  \ Address of data stack scratch cell (last cell in memory).
  (sp0) @ 1 cells +
  s" STACK-CELLS" environment? drop cells -
  s" RETURN-STACK-CELLS" environment? drop cells -
  here -
;

: u> ( u1 u2 -- flag ) swap u< ;

: buffer: ( u "<spaces>name" -- ) create allot ;

: 2r@ ( -- x1 x2 ) ( R: x1 x2 -- x1 x2 ) (rp@) 3 cells + @ (rp@) 2 cells + @ ;

: .r ( n1 n2 -- )
  swap dup abs 0 <# #s rot sign #>
  rot over - spaces type
;
: u.r ( u1 u2 -- )
  swap 0 <# #s #>
  rot over - spaces type
;
: holds ( addr u -- ) begin dup while 1- 2dup + c@ hold repeat 2drop ;

\ Compile an inline counted string. At runtime, push its address.
\
\ The result looks like this in memory:
\
\                         addr             dest
\                         v                v
\   [Lit][addr][Jmp][dest][11][foo bar baz][...]
\
\
\ `Lit` pushes `addr` and then `Jmp` jumps past the string to `dest`.
: c" ( C: "ccc<quote>" -- ) ( -- c-addr )
  \ Reserve the `Lit` and `Jmp` placeholders.
  ['] (lit) compile, here 0 ,   ( addr-slot )
  ['] (jmp) compile, here 0 ,   ( addr-slot dest-slot )
  here                          ( addr-slot dest-slot addr )
  [char] " parse                ( ... src len )
  \ Store the length byte.
  dup c,
  \ Copy the characters after the count byte.
  here swap dup allot move      ( addr-slot dest-slot addr )
  align
  \ Patch `Lit`.
  rot !                         ( dest-slot )
  \ Patch `Jmp`.
  here swap !                   ( )
; immediate (compile-only)

\ case
\
\ `case` structures expand to a nested `if ... then`. `case` pushes an
\ accumulator onto the stack. Each `of` increments the accumulator. `endcase`
\ compiles a `then` for each `of`.
\
\ Lifted from
\ <https://forth-standard.org/standard/rationale#paragraph.A.3.2.3.2>.
0 constant case immediate (compile-only)

: of
  1+
  >r
  postpone over postpone =
  postpone if
  postpone drop
  r>
; immediate (compile-only)

: endof
  >r
  postpone else
  r>
; immediate (compile-only)

: endcase
  postpone drop
  0 ?do
    postpone then
  loop
; immediate (compile-only)

: source-id ( -- 0|-1 ) (source-id) @ ;
: save-input ( -- xn...x1 n ) >in @ source source-id 4 ;
: restore-input ( xn...x1 n -- flag )
  depth > if -4 throw then
  (source-id) !
  (source-len) !
  (source-addr) !
  >in !
  false
;

: (marker) ( addr addr ) (here) ! (latest) ! ;

: marker ( "<spaces>name" -- )
  here (latest) @     ( here latest )
  parse-name (header)
  postpone literal    ( here )
  postpone literal    ( )
  postpone (marker)
  ['] (exit) compile,
;

: value ( x "<spaces>name" -- ) create , does> @ ;
: to ( x "<spaces>name" -- )
  ' >body
  state @ if postpone literal postpone ! else ! then
; immediate

: defer ( "<spaces>name" -- ) create ['] abort compile, does> @ execute ;
: defer! ( xt2 xt1 -- ) >body ! ;
: defer@ ( xt1 -- xt2 ) >body @ ;
: is ( xt "<spaces>name" -- )
  state @ if postpone ['] postpone defer! else ' defer! then
; immediate
: action-of ( "<spaces>name" -- xt )
  state @ if postpone ['] postpone defer@ else ' defer@ then
; immediate

\ Similar to s", but calls `(parse\")` to parse escape sequences.
: s\" ( "ccc<quote>" -- )
  state @ if
    ['] (s") compile,
    source >in @ here cell+   ( src srclen pos dest )
    (parse\")                 ( dest u pos' )
    >in !                     ( dest u )
    nip                       ( u )
    dup , allot align
  else
    source >in @ pad (parse\") >in !
  then
; immediate
