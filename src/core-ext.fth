: .( ( "ccc<paren>" -- ) [char] ) parse type ; immediate

: erase 0 fill ;

: within ( n lo hi -- flag ) over - >r - r> u< ;

: pick cells (sp@) swap + 2 cells + @ ;
: nip swap drop ;
