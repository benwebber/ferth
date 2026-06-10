\ The ttester harness uses `environment?` and the conditional compilation words
\ `[if]`/`[else]`/`[then]` to conditionally enable floating point tests.
\
\ This system does not support floating point yet. This file provides shims to
\ load the file and skip those tests.

\ [IF]/[ELSE]/[THEN] reference implementations. Case-sensitive.
\
\   https://forth-standard.org/standard/tools/BracketELSE
\   https://forth-standard.org/standard/tools/BracketIF
\   https://forth-standard.org/standard/tools/BracketELSE
: [ELSE] ( -- )
  1 begin                               ( level )
     begin bl word count dup while      ( level addr len )
       2dup s" [IF]" compare 0= if      ( level addr len )
           2drop 1+                     ( level' )
        else                            ( level addr len )
          2dup s" [ELSE]" compare 0= if ( level addr len )
              2drop 1- dup if 1+ then   ( level' )
          else                          ( level addr len )
              s" [THEN]" compare 0= if  ( level )
                 1-                     ( level' )
             then
           then
        then ?dup 0= if exit then       ( level' )
     repeat 2drop                       ( level )
  refill 0= until                       ( level )
  drop
; immediate
: [IF] ( flag -- ) 0= if postpone [ELSE] then ; immediate
: [THEN] ( -- ) ; immediate
