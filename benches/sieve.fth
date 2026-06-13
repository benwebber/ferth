: nip swap drop ;
: erase 0 fill ;
: pick cells (sp@) swap - 2 cells - @ ;

variable sieve-end

: sieve ( limit -- n )
    dup sieve-end !
    here over 1+ erase
    1 here c! 1 here 1+ c!
    2
    begin dup dup * 2 pick < while
        here over + c@ 0= if
            dup dup *
            begin
                here over + 1 swap c!
                over +
                dup sieve-end @ >
            until
            drop
        then
        1+
    repeat drop
    0 swap 1+ 0 do i here + c@ 0= if 1+ then loop ;
