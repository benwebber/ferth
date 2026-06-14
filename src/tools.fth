variable (dump-start)
variable (dump-width)
variable (dump-end)
8 constant (dump-group)

: (green) 27 emit ." [32m" ;
: (blue) 27 emit ." [36m" ;
: (dim) 27 emit ." [2m" ;
: (sgr0) 27 emit ." [0m" ;

: (dump-byte) 0 <# # # #> type ;

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

: (dump-printable) ( char -- char' ) dup $20 $7f within 0= if drop [char] . then ;
: (dump-addr) ( addr -- ) 0 <# (dump-width) @ 0 ?do # loop #> type ;

: (dump?) ( addr -- addr flag ) dup (dump-end) @ u< ;

: (dump-row) ( addr -- )
  cr
  dup (dump-addr) 2 spaces  ( addr )
  pad (/hold) + swap                      ( ascii addr )
  dup 16 + swap                           ( ascii addr' addr )
  ?do
    i (dump-start) @ -                    ( ascii pos )
    dup (dump-group) mod 0= swap 16 mod 0<> and if space then
    i (dump-end) @ u< if
      i c@                                ( ascii char )
      dup (emit-byte) space               ( ascii char )
      (dump-printable) over c!            ( ascii )
    else
      3 spaces
      bl over c!                          ( ascii )
    then
    1+
  loop
  drop
  space
  pad (/hold) + 16 type
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
