local function find(list)
  local first, last
  for _, item in pairs(list) do
    if not first then
      first = item
    end
    last = item
  end
  return first, last
end
return find
