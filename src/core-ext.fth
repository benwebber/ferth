: .( ( "ccc<paren>" -- ) [char] ) parse type ; immediate

: within ( n lo hi -- flag ) over - >r - r> u< ;

: nip swap drop ;
