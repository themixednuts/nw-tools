local first, second = ...
local data
if first then
    local value = load_first(first)
    data = value
elseif second then
    local value = load_second(second)
    data = value
else
    report("missing")
    return
end
return #data
