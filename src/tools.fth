: (dump-byte) 0 <# # # #> type ;
: (dump-printable) ( x -- )
  dup $20 $7f within if
    emit
  else
    drop
    [char] . emit
  then
;
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
  dup (dump-addr) [char] : emit space
  dup 16 over + swap ?do
    i (dump?) if c@ (dump-byte) space else 3 spaces then
  loop
  [char] | emit
  16 over + swap ?do
    i (dump?) if c@ (dump-printable) else drop space then
  loop
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
