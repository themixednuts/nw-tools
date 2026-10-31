local items, wanted = ...
local found = false
for index = 1, #items do
    if items[index] == wanted then
        found = true
        break
    end
end
return found
