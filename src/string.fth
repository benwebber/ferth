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
