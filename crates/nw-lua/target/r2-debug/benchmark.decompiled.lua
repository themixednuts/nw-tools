local PopupWrapper = RequireScript("LyShineUI.Popup.PopupRequestWrapper")
local Benchmark = {}
Benchmark.Properties = {
	Frame = { default = (EntityId()) },
	ResultsFrame = { default = (EntityId()) },
	MainBgTexture1 = { default = (EntityId()) },
	MainBgTexture2 = { default = (EntityId()) },
	ScreenScrim = { default = (EntityId()) },
	ClickThroughCover = { default = (EntityId()) },
	VisualScreen = { default = (EntityId()) },
	MinFPSValueText = { default = (EntityId()) },
	MaxFPSValueText = { default = (EntityId()) },
	AVGFPSValueText = { default = (EntityId()) },
	continueButton = { default = (EntityId()) },
	retryButton = { default = (EntityId()) },
}
Benchmark.INPUT_TYPE_DROPDOWN = "Dropdown"
Benchmark.INPUT_TYPE_TEXT = "Text"
Benchmark.INPUT_TYPE_EXTERNAL_LINK = "ExternalLink"
Benchmark.DISPLAY_MODE_FULLSCREEN = 0
Benchmark.DISPLAY_MODE_WINDOWED = 1
Benchmark.DISPLAY_MODE_WINDOWED_FULLSCREEN = 2
Benchmark.mResolutionDropdown = nil
Benchmark.mLastResolution = nil
Benchmark.mLastResolutionData = nil
Benchmark.mIsPopupEnabled = nil
Benchmark.mPopupTimeDisplayed = 0
Benchmark.mPopupRevertTime = 10
Benchmark.mPopupEventId = "Resolution_Confirm_Popup"
Benchmark.DATA_LAYER_OPTIONS = "Hud.LocalPlayer.Options"
Benchmark.mOptionsDataNode = nil
Benchmark.mVisualScreenListItems = {}
local BaseScreen = RequireScript("LyShineUI._Common.BaseScreen")
BaseScreen:CreateNewScreen(Benchmark)
local Spawner = RequireScript("LyShineUI._Common.Spawner")
Spawner:AttachSpawner(Benchmark)
local ResolutionManager = RequireScript("LyShineUI.Options.ResolutionManager")
function Benchmark:OnInit()
	BaseScreen.OnInit(self)
	local v1_2 = {
		{ text = "@ui_fullscreen", value = self.DISPLAY_MODE_FULLSCREEN },
		{ text = "@ui_windowed", value = self.DISPLAY_MODE_WINDOWED },
		{ text = "@ui_windowed_fullscreen", value = self.DISPLAY_MODE_WINDOWED_FULLSCREEN },
	}
	self.WindowModeData = v1_2
	local v1_3 = {
		{
			text = "@ui_window_mode",
			desc = "@ui_window_mode_desc",
			dataNode = "Video.WindowMode",
			inputType = self.INPUT_TYPE_DROPDOWN,
			dropdownData = self.WindowModeData,
			callback = "SetWindowMode",
		},
		{
			text = "@ui_resolution",
			desc = "@ui_resolution_desc",
			dataNode = "Video.Resolution",
			inputType = self.INPUT_TYPE_DROPDOWN,
			callback = "SetResolution",
		},
		{
			text = "@ui_graphics_settings",
			desc = "@ui_graphics_settings_desc",
			dataNode = "Video.GraphicsQuality",
			inputType = self.INPUT_TYPE_DROPDOWN,
			callback = "SetGraphicsQuality",
			dropdownData = {
				{ text = "@ui_options_graphics_low", data = 1 },
				{ text = "@ui_options_graphics_medium", data = 2 },
				{ text = "@ui_options_graphics_high", data = 3 },
				{ text = "@ui_options_graphics_vhigh", data = 4 },
			},
		},
	}
	self.VisualListItemData = v1_3
	OptionsDataBus.Broadcast.InitializeSerializedOptions()
	self.mOptionsDataNode = self.dataLayer:GetDataNode(self.DATA_LAYER_OPTIONS)
	self.dataLayer:RegisterOpenEvent("Benchmark", self.canvasId)
	self.Frame:SetFrameTextureVisible(false)
	self.Frame:SetFillAlpha(0.2)
	self.Frame:SetLineColor(self.UIStyle.COLOR_TAN)
	self.ResultsFrame:SetFrameTextureVisible(false)
	self.ResultsFrame:SetFillAlpha(0.2)
	self.ResultsFrame:SetLineColor(self.UIStyle.COLOR_TAN)
	self:BusConnect(UiSpawnerNotificationBus, self.VisualScreen)
	for i = 1, #self.VisualListItemData do
		self:SpawnSettingsSlice(self.VisualListItemData[i], i, self.VisualScreen)
	end
	self:SetScreenVisible(false)
	self:BusConnect(BenchmarkControllerComponentNotificationsBus, self.canvasId)
	self.continueButton:SetCallback(self.Continue, self)
	self.retryButton:SetCallback(self.Retry, self)
end
function Benchmark:SpawnSettingsSlice(data, index, screen)
	data.itemIndex = index
	data.itemScreen = screen
	self:SpawnSlice(screen, "LyShineUI\\Options\\OptionsListItem", self.OnListItemSpawned, data)
end
function Benchmark:OnListItemSpawned(entity, data)
	entity:SetText(data.text)
	entity:SetTextDescription(data.desc)
	entity:SetInputType(data.inputType)
	local buttonInputHolder = entity:GetInputHolder()
	self:BusConnect(UiSpawnerNotificationBus, buttonInputHolder)
	if data.inputType == self.INPUT_TYPE_DROPDOWN then
		self:SpawnSlice(buttonInputHolder, "LyShineUI\\Slices\\Dropdown", self.OnListItemInputSpawned, data)
	end
	self.mVisualScreenListItems[data.itemIndex] = entity
end
function Benchmark:OnListItemInputSpawned(entity, data)
	local dataNode
	if data.dataNode then
		dataNode = self.mOptionsDataNode[data.dataNode]
	end
	if data.inputType == self.INPUT_TYPE_DROPDOWN then
		local listItemData, dropdownText
		if data.dataNode == "Video.Resolution" then
			local currentWidth = dataNode.Width:GetData()
			local currentHeight = dataNode.Height:GetData()
			dropdownText = currentWidth .. " x " .. currentHeight
			local isWindowedMode = self.mOptionsDataNode.Video.WindowMode:GetData() ~= 0 or false
			listItemData = ResolutionManager:GetResolutions(self.dataLayer, isWindowedMode)
			self.mResolutionDropdown = entity
		elseif data.dropdownData then
			local itemIndex = dataNode:GetData()
			if data.dataNode == "Video.WindowMode" then
				itemIndex = itemIndex + 1
			end
			dropdownText = data.dropdownData[itemIndex].text
			listItemData = data.dropdownData
		end
		entity:SetDropdownScreenCanvasId(self.entityId)
		entity:SetListData(listItemData)
		entity:SetCallback(data.callback, self)
		entity:SetText(dropdownText)
		local defaultRows = 5
		if #listItemData < defaultRows then
			defaultRows = #listItemData
		end
		entity:SetDropdownListHeightByRows(defaultRows)
	end
	listItemData = UiElementBus
	local inputHolder = listItemData.Event.GetParent(entity.entityId)
	local inputHolderWidth = UiTransform2dBus.Event.GetLocalWidth(inputHolder)
	local inputHolderHeight = UiTransform2dBus.Event.GetLocalHeight(inputHolder)
	local entityWidth = entity:GetWidth()
	local entityHeight = entity:GetHeight()
	local offsetPosX = inputHolderWidth - entityWidth
	local offsetPosY = (inputHolderHeight - entityHeight) / 2
	UiTransformBus.Event.SetLocalPosition(entity.entityId, Vector2(offsetPosX, offsetPosY))
end
function Benchmark:SetScreenVisible(isVisible)
	local animDuration = 0.8
	if isVisible == true then
		local v7 = {}
		v7.opacity = 0
		self.ScriptedEntityTweener:Play(self.Properties.ScreenScrim, 0.5, v7, { opacity = 0.8, ease = "QuadOut" })
		self.ScriptedEntityTweener:Set(self.Properties.MainBgTexture1, { scaleX = 1, scaleY = 1, opacity = 0 })
		self.timeline1 = self.ScriptedEntityTweener:TimelineCreate()
		self.timeline1:Add(self.MainBgTexture1, 6, { scaleX = 1.1, scaleY = 1.1, opacity = 0.85 })
		self.timeline1:Add(self.MainBgTexture1, 6, { scaleX = 1.2, scaleY = 1.2, opacity = 0 })
		self.timeline1:Add(self.MainBgTexture1, 0.1, {
			scaleX = 1,
			scaleY = 1,
			opacity = 0,
			onComplete = function()
				self.timeline1:Play()
			end,
		})
		self.timeline1:Play()
		self.ScriptedEntityTweener:Set(self.Properties.MainBgTexture2, { scaleX = 1, scaleY = 1, opacity = 0 })
		self.timeline2 = self.ScriptedEntityTweener:TimelineCreate()
		self.timeline2:Add(self.MainBgTexture2, 6, { scaleX = 1, scaleY = 1, opacity = 0 })
		self.timeline2:AddLabel("Loop", 6)
		self.timeline2:Add(self.MainBgTexture2, 6, { scaleX = 1.1, scaleY = 1.1, opacity = 0.85 })
		self.timeline2:Add(self.MainBgTexture2, 6, { scaleX = 1.2, scaleY = 1.2, opacity = 0 })
		self.timeline2:Add(self.MainBgTexture2, 0.1, {
			scaleX = 1,
			scaleY = 1,
			opacity = 0,
			onComplete = function()
				self.timeline2:Play("Loop")
			end,
		})
		self.timeline2:Play()
		self.Frame:SetLineVisible(true, 1.5)
		self.ResultsFrame:SetLineVisible(true, 1.5)
	elseif self.timeline1 ~= nil then
		self.timeline1:Stop()
		self.timeline2:Stop()
		self.Frame:SetLineVisible(false)
	end
end
function Benchmark:SetSelectedScreenVisible(entity)
	UiElementBus.Event.SetIsEnabled(self.Properties.VisualScreen, true)
	self:SetVisualScreenVisible()
end
function Benchmark:SetOptionScreenVisible(screenListItems, duration, delay)
	local animDuration = duration or 0.6
	local animDelay = delay or 0.03
	for i = 1, #screenListItems do
		local currentItem = screenListItems[i]
		local v15 = {}
		v15.opacity = 0
		self.ScriptedEntityTweener:Play(
			currentItem.entityId,
			animDuration,
			v15,
			{ opacity = 1, ease = "QuadOut", delay = animDelay * i }
		)
	end
end
function Benchmark:SetVisualScreenVisible()
	self:SetOptionScreenVisible(self.mVisualScreenListItems)
end
function Benchmark:SetWindowMode(entity, data)
	local selectedMode = data.value
	if selectedMode == self.DISPLAY_MODE_FULLSCREEN then
		OptionsDataBus.Broadcast.GoFullscreen(false)
		self:RefreshResolutionDropdown(false)
	elseif selectedMode == self.DISPLAY_MODE_WINDOWED then
		OptionsDataBus.Broadcast.GoWindowed()
		self:RefreshResolutionDropdown(true)
	elseif selectedMode == self.DISPLAY_MODE_WINDOWED_FULLSCREEN then
		OptionsDataBus.Broadcast.GoFullscreen(true)
		self:RefreshResolutionDropdown(true)
	end
end
function Benchmark:RefreshResolutionDropdown(isWindowMode)
	local listItemData = ResolutionManager:GetResolutions(self.dataLayer, isWindowMode)
	self.mResolutionDropdown:SetListData(listItemData)
end
function Benchmark:SetResolution(entity, data)
	local resolutionNode = self.mOptionsDataNode.Video.Resolution
	local v5 = resolutionNode.Width:GetData()
	self.mLastResolution = { v5, resolutionNode.Height:GetData() }
	self.mLastResolutionData = self.mResolutionDropdown:GetPreviousSelectedItemData()
	local width = data.width
	local height = data.height
	OptionsDataBus.Broadcast.SetResolution(width, height)
	self.mIsPopupEnabled = true
	self.mPopupTimeDisplayed = 0
	PopupWrapper:RequestPopup(
		ePopupButtons_YesNo,
		"@ui_resolution_popup_title",
		"@ui_resolution_popup_title_message",
		self.mPopupEventId,
		self,
		self.OnPopupResult
	)
	if self.tickBusHandler then
		self:BusDisconnect(self.tickBusHandler)
		self.tickBusHandler = nil
	end
	self.tickBusHandler = self:BusConnect(DynamicBus.UITickBus)
end
function Benchmark:OnTick(deltaTime, timePoint)
	if not self.mIsPopupEnabled then
		return
	end
	self.mPopupTimeDisplayed = self.mPopupTimeDisplayed + deltaTime
	if self.mPopupRevertTime < self.mPopupTimeDisplayed then
		PopupWrapper:KillPopup(self.mPopupEventId)
	end
end
function Benchmark:OnPopupResult(result, eventId)
	if eventId == self.mPopupEventId then
		if result ~= ePopupResult_No and result == ePopupResult_ForceClosed and self.mLastResolution then
			OptionsDataBus.Broadcast.SetResolution(self.mLastResolution[1], self.mLastResolution[2])
			self.mResolutionDropdown:SetSelectedItemData(self.mLastResolutionData)
			self.mLastResolution = nil
		end
		self.mIsPopupEnabled = false
	end
	self:BusDisconnect(self.tickBusHandler)
	self.tickBusHandler = nil
end
function Benchmark:SetGraphicsQuality(entityId, data)
	OptionsDataBus.Broadcast.SetGraphicsQuality(data.data)
end
function Benchmark:OnAction(entityId, action)
	local currentItem = self.registrar:GetEntityTable(entityId)
	if currentItem ~= nil and currentItem.GetInputType ~= nil then
		for i = 1, #self.mVisualScreenListItems do
			local listItem = self.mVisualScreenListItems[i]
			listItem:OnUnfocus()
		end
	end
	return BaseScreen.OnAction(self, entityId, action)
end
function Benchmark:OnShutdown()
	OptionsDataBus.Broadcast.SerializeOptions()
	LyShineDataLayerBus.Broadcast.SetData("Hud.LocalPlayer.Options.OnClosed", true)
	if self.mIsPopupEnabled then
		UiPopupBus.Broadcast.HidePopup(self.mPopupEventId)
	end
	if self.tickBusHandler ~= nil then
		self:BusDisconnect(self.tickBusHandler)
		self.tickBusHandler = nil
	end
	if self.timeline1 ~= nil then
		self.timeline1:Stop()
		self.timeline2:Stop()
		self.ScriptedEntityTweener:TimelineDestroy(self.timeline1)
		self.ScriptedEntityTweener:TimelineDestroy(self.timeline2)
	end
	BaseScreen.OnShutdown(self)
end
function Benchmark:OnTransitionIn(stateName, levelName)
	self:SetScreenVisible(true)
	self:BusConnect(CryActionNotificationsBus, "toggleMenuComponent")
end
function Benchmark:OnTransitionOut(stateName, levelName)
	self:SetScreenVisible(false)
	self:BusDisconnect(self.escapeKeyHandler)
	self.escapeKeyHandler = nil
	LyShineManagerBus.Broadcast.TransitionOutComplete()
end
function Benchmark:Continue()
	local entityId = self.dataLayer:GetDataFromNode("Hud.LocalPlayer.Intro.BenchmarkControllerEntityID")
	if entityId then
		BenchmarkControllerComponentRequestBus.Event.OnContinue(entityId)
	end
end
function Benchmark:Retry()
	local entityId = self.dataLayer:GetDataFromNode("Hud.LocalPlayer.Intro.BenchmarkControllerEntityID")
	if entityId then
		BenchmarkControllerComponentRequestBus.Event.OnRetry(entityId)
	end
end
function Benchmark:ShowFPSValues(minFPS, maxFPS, avgFPS)
	UiTextBus.Event.SetText(self.Properties.MinFPSValueText, minFPS)
	UiTextBus.Event.SetText(self.Properties.MaxFPSValueText, maxFPS)
	UiTextBus.Event.SetText(self.Properties.AVGFPSValueText, avgFPS)
end
return Benchmark
