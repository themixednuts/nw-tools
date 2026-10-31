local function pick(key, areas, use)
  if key == 1 then
    use(1)
  else
    local area = areas[key]
    if area.bank ~= "" then
      use(area.bank)
    elseif area.group ~= "" then
      use(area.group)
    end
  end
end
return pick
