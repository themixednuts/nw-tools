local items = ...
local first, last
for _, item in pairs(items) do
    if not first then
        first = item
    end
    last = item
end
return first, last
