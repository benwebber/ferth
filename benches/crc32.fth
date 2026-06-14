hex
: crc-bit ( crc -- crc )
    dup 1 and
    if 1 rshift EDB88320 xor
    else 1 rshift
    then ;
: crc-byte ( crc byte -- crc )
    xor 8 0 do crc-bit loop ;
: crc32 ( c-addr u -- crc )
    FFFFFFFF swap
    0 do over i + c@ crc-byte loop
    nip FFFFFFFF xor ;
decimal
