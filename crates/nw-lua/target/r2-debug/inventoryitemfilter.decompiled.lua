local ContractBrowser_SellTab_InventoryItemFilter = {}
ContractBrowser_SellTab_InventoryItemFilter.Properties =
	{ PrimaryFilterContainer = { default = (EntityId()) }, PrimaryFilterElement = { default = (EntityId()) } }
ContractBrowser_SellTab_InventoryItemFilter.primaryFilterList = {}
ContractBrowser_SellTab_InventoryItemFilter.additionalPrimaryCategories = {}
ContractBrowser_SellTab_InventoryItemFilter.CATEGORY_PRIORITY =
	{ Weapons = 1, Tools = 2, Ammos = 3, Apparel = 4, Resources = 5, Utilities = 6, Ammo = 7 }
ContractBrowser_SellTab_InventoryItemFilter.primaryCategoryIconPath = "LyShineUI\\Images\\Icons\\ItemTypes\\%s.dds"
local BaseElement = RequireScript("LyShineUI._Common.BaseElement")
BaseElement:CreateNewElement(ContractBrowser_SellTab_InventoryItemFilter)
function ContractBrowser_SellTab_InventoryItemFilter:OnInit()
	BaseElement.OnInit(self)
	self.canvasId = UiElementBus.Event.GetCanvas(self.entityId)
	self.additionalPrimaryCategories[0] = self.PrimaryFilterElement
	self.PrimaryFilterElement:SetImage("LyShineUI\\Images\\Icons\\ItemTypes\\itemType_all.dds")
	self.PrimaryFilterElement:AddToGroup(self.Properties.PrimaryFilterContainer)
	self.PrimaryFilterElement:SetCallback(self, self.SelectPrimaryCategory, nil)
	self.PrimaryFilterElement:SetVisible(true)
	self:UpdatePrimaryFilterCategories()
	self:SetFilter()
end
function ContractBrowser_SellTab_InventoryItemFilter:OnShutdown()
	local primaryElements = UiElementBus.Event.GetChildren(self.Properties.PrimaryFilterContainer)
	for i = 2, primaryElements and #primaryElements or 0 do
		UiElementBus.Event.DestroyElement(primaryElements[i])
	end
end
function ContractBrowser_SellTab_InventoryItemFilter:UpdatePrimaryFilterCategories()
	self.primaryFilterList = self:GetSortedCategoryList(self:GetFilterList())
	for i = 1, #self.primaryFilterList do
		local categoryKey = self.primaryFilterList[i]
		local categoryButtonTable = self.additionalPrimaryCategories[i]
		if categoryButtonTable == nil then
			categoryButtonTable = CloneUiElement(
				self.canvasId,
				self.registrar,
				self.Properties.PrimaryFilterElement,
				self.Properties.PrimaryFilterContainer,
				true
			)
			self.additionalPrimaryCategories[i] = categoryButtonTable
		end
		categoryButtonTable:SetImage(string.format(self.primaryCategoryIconPath, "itemType_" .. categoryKey))
		categoryButtonTable:AddToGroup(self.Properties.PrimaryFilterContainer)
		categoryButtonTable:SetCallback(self, self.SelectPrimaryCategory, categoryKey)
		categoryButtonTable:SetVisible(true)
	end
	for i = #self.primaryFilterList + 1, #self.additionalPrimaryCategories do
		self.additionalPrimaryCategories[i]:SetVisible(false)
	end
end
function ContractBrowser_SellTab_InventoryItemFilter:GetSortedCategoryList(originalList)
	local sortedFilterList = {}
	for i = 1, #originalList do
		local categoryKey = originalList[i]
		local sortIndex = self.CATEGORY_PRIORITY[categoryKey]
		sortIndex = sortIndex or GetMaxNum()
		table.insert(sortedFilterList, { sortIndex = sortIndex, originalKey = categoryKey })
	end
	table.sort(sortedFilterList, function(a, b)
		return a.sortIndex < b.sortIndex or false
	end)
	local toReturnList = {}
	for _, sortListData in ipairs(sortedFilterList) do
		table.insert(toReturnList, sortListData.originalKey)
	end
	return toReturnList
end
function ContractBrowser_SellTab_InventoryItemFilter:SetFilter(categoryKey)
	self.categoryKey = categoryKey
	self:ExecuteCallback(self.categoryKey)
end
function ContractBrowser_SellTab_InventoryItemFilter:GetFilterItem()
	local itemList =
		ItemDataManagerBus.Broadcast.GetFilteredItemList(self.categoryKey, self.familyKey, self.groupKey, self.tierKey)
	if itemList and itemList[1] then
		return itemList[1]
	end
end
function ContractBrowser_SellTab_InventoryItemFilter:SelectPrimaryCategory(categoryKey)
	self:SetFilter(categoryKey)
end
function ContractBrowser_SellTab_InventoryItemFilter:SetCallback(callerSelf, callerFn)
	self.callerSelf = callerSelf
	self.callerFn = callerFn
end
function ContractBrowser_SellTab_InventoryItemFilter:ExecuteCallback(filterValue)
	if not self.callerFn then
		return
	end
	self.callerFn(self.callerSelf, filterValue)
end
function ContractBrowser_SellTab_InventoryItemFilter:GetFilterList(categoryKey, familyKey, groupKey, tierKey)
	return ItemDataManagerBus.Broadcast.GetFilteredCategoryList(categoryKey, familyKey, groupKey, tierKey)
end
function ContractBrowser_SellTab_InventoryItemFilter:SelectAllCategory()
	UiRadioButtonGroupCommunicationBus.Event.RequestRadioButtonStateChange(
		self.Properties.PrimaryFilterContainer,
		self.Properties.PrimaryFilterElement,
		true
	)
end
function ContractBrowser_SellTab_InventoryItemFilter:SelectCategory(key)
	for i = 0, #self.additionalPrimaryCategories do
		if self.additionalPrimaryCategories[i].key == key then
			UiRadioButtonGroupCommunicationBus.Event.RequestRadioButtonStateChange(
				self.Properties.PrimaryFilterContainer,
				self.additionalPrimaryCategories[i].entityId,
				true
			)
			break
		end
	end
end
return ContractBrowser_SellTab_InventoryItemFilter
