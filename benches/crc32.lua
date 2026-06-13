local function crc_bit(crc)
    if crc & 1 ~= 0 then
        return (crc >> 1) ~ 0xEDB88320
    else
        return crc >> 1
    end
end

local function crc_byte(crc, byte)
    crc = crc ~ byte
    for _ = 1, 8 do
        crc = crc_bit(crc)
    end
    return crc
end

function crc32(data)
    local crc = 0xFFFFFFFF
    for i = 1, #data do
        crc = crc_byte(crc, data:byte(i))
    end
    return crc ~ 0xFFFFFFFF
end
