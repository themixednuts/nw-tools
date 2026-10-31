local page, limit, table_ = ...
local data = (page or 1) <= limit and load_page(table_[page or 1]) or nil
if not data then
    return
end
show(data)
