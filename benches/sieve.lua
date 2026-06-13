function sieve(limit)
    local composite = {}
    for i = 0, limit do composite[i] = false end
    composite[0] = true
    composite[1] = true
    local i = 2
    while i * i < limit do
        if not composite[i] then
            local j = i * i
            while j <= limit do
                composite[j] = true
                j = j + i
            end
        end
        i = i + 1
    end
    local count = 0
    for i = 0, limit do
        if not composite[i] then count = count + 1 end
    end
    return count
end
