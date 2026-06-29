: .( ( "ccc<paren>" -- ) [char] ) parse type ; immediate

: erase 0 fill ;

: within ( n lo hi -- flag ) over - >r - r> u< ;

: pick cells (sp@) swap + 2 cells + @ ;
: nip swap drop ;

: unused ( -- u )
  \ Address of data stack scratch cell (last cell in memory).
  (sp0) @ 1 cells +
  s" STACK-CELLS" environment? drop cells -
  s" RETURN-STACK-CELLS" environment? drop cells -
  here -
;
