: (dump-byte) 0 <# # # #> type ;
: (dump-printable) ( char -- char' ) dup $20 $7f within 0= if drop [char] . then ;
: (dump-addr) ( addr -- )
  (address-unit-bits) cells
  dup 64 = if drop 0 <# # # # # # # # # # # # # # # # # #> type exit then
  dup 32 = if drop 0 <# # # # # # # # # #> type exit then
  0 <# # # # # #> type
;

variable (dump-end)

: (dump?) ( addr -- addr flag ) dup (dump-end) @ u< ;

: (dump-row) ( addr -- )
  cr
  dup (dump-addr) [char] : emit space   ( addr )
  pad (/hold) + swap                    ( ascii addr )
  dup 16 + swap                         ( ascii addr' addr )
  ?do
    i (dump-end) @ u< if
      i c@                              ( ascii char )
      dup (dump-byte) space             ( ascii char )
      (dump-printable) over c!          ( ascii )
    else
      3 spaces
      bl over c!                        ( ascii )
    then
    1+
  loop
  drop
  [char] | emit
  pad (/hold) + 16 type
  [char] | emit
;

: dump ( addr u )
  base @ >r
  hex
  over +              ( addr end )
  dup (dump-end) !    ( addr end )
  swap                ( end addr )
  ?do i (dump-row) 16 +loop
  0 (dump-end) !
  r> base !
  cr
;
