local count, index, left, right = ...
local item, is_right
if index < count then
    item = left[index]
    is_right = false
else
    item = right[index]
    is_right = true
end
return lookup[item], is_right
