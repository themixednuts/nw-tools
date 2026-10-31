local first, second = ...
if first then
elseif second then
else
    report("missing")
    return
end
report("done")
