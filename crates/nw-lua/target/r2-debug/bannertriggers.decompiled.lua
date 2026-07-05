local BannerTriggers = {}
BannerTriggers.WAR_BANNER_DISPLAY_DURATION = 9
BannerTriggers.WAR_BANNER_DRAW_ORDER = 25
BannerTriggers.BANNER_DRAW_ORDER_TOP = 100
BannerTriggers.TERRITORY_CLAIMED_BANNER_DRAW_ORDER = 25
BannerTriggers.POINT_FORCED_TIME = 20
BannerTriggers.POINT_BANNER_DISPLAY_DURATION = 6
BannerTriggers.TOWN_CHECKIN_THRESHOLD = 20
BannerTriggers.mLastDamagedClaim = ""
BannerTriggers.mLastDamagedClaimHealth = 100
BannerTriggers.mDamageBannerId = nil
BannerTriggers.queuedTradeskillBanners = {}
BannerTriggers.timeSincePointCheck = 0
BannerTriggers.attributePoints = 0
BannerTriggers.masteryPoints = 0
BannerTriggers.firstLoadingScreenDismissed = false
BannerTriggers.isInCombat = false
BannerTriggers.raidId = RaidId()
BannerTriggers.DEBUG_OBJECTIVE_COMPLETED = false
BannerTriggers.TRADESKILL_ICON_PATH = "LyShineUI\\Images\\Tradeskills\\tradeskill_%s.dds"
BannerTriggers.DUNGEON_LIMIT_WARNING_THRESHOLD = 2
BannerTriggers.showEndGameBannerLevel = 60
BannerTriggers.QUEST_CELEBRATION_DISPLAY_DURATION = 4
local layouts = RequireScript("LyShineUI.Banner.Layouts")
local timeHelpers = RequireScript("LyShineUI._Common.TimeHelperFunctions")
local SlashCommands = RequireScript("LyShineUI.SlashCommands")
local ObjectiveTypeData = RequireScript("LyShineUI.Objectives.ObjectiveTypeData")
local ObjectiveDataHelper = RequireScript("LyShineUI.Objectives.ObjectiveDataHelper")
local ObjectivesDataHandler = RequireScript("LyShineUI._Common.ObjectivesDataHandler")
local WeaponMasteryData = RequireScript("LyShineUI.Skills.WeaponMastery.WeaponMasteryData")
local SocialDataHandler = RequireScript("LyShineUI._Common.SocialDataHandler")
local TerritoryDataHandler = RequireScript("LyShineUI._Common.TerritoryDataHandler")
local EncounterDataHandler = RequireScript("LyShineUI._Common.EncounterDataHandler")
local FactionCommon = RequireScript("LyShineUI._Common.FactionCommon")
local TradeSkillsCommon = RequireScript("LyShineUI._Common.TradeSkillsCommon")
local PopupWrapper = RequireScript("LyShineUI.Popup.PopupRequestWrapper")
local TimingUtils = RequireScript("LyShineUI._Common.TimingUtils")
local StaticItemDataManager = RequireScript("LyShineUI._Common.StaticItemDataManager")
local CampCommon = RequireScript("LyShineUI.Inventory.CampCommon")
local ExpeditionsCommon = RequireScript("LyShineUI._Common.ExpeditionsCommon")
local seasonsRewardsCommon = RequireScript("LyShineUI.SeasonsRewards.SeasonsRewardsCommon")
local inventoryCommon = RequireScript("LyShineUI._Common.InventoryCommon")
local UIStyle = RequireScript("LyShineUI._Common.UIStyle")
local TerritoryEnteredCardTypes = {}
TerritoryEnteredCardTypes.TerritoryType = 0
TerritoryEnteredCardTypes.SettlementType = 1
TerritoryEnteredCardTypes.FortType = 2
TerritoryEnteredCardTypes.HQType = 3
TerritoryEnteredCardTypes.OutpostType = 4
TerritoryEnteredCardTypes.OpenWorld = 5
function BannerTriggers:OnInit(banners, dataLayer, tweener, audioHelper)
	if not banners or not dataLayer or not tweener or not audioHelper then
		Log("BannerTriggers:Init(): invalid init parameters")
		return
	end
	self.suppressPointsBannersDuringCombat =
		ConfigProviderEventBus.Broadcast.GetBool("UIFeatures.in-combat-banner-suppression-points")
	self.suppressWarDeclarationBannersDuringCombat =
		ConfigProviderEventBus.Broadcast.GetBool("UIFeatures.in-combat-banner-suppression-war-declarations")
	self.suppressStationBannersDuringCombat =
		ConfigProviderEventBus.Broadcast.GetBool("UIFeatures.in-combat-banner-suppression-stations")
	self.TOWN_PROJECTS_STATE = 640726528
	self.OWMISSION_BOARD_STATE = 2609973752
	self.notificationHandlers = {}
	self.banners = banners
	self.dataLayer = dataLayer
	self.ScriptedEntityTweener = tweener
	self.audioHelper = audioHelper
	self.playerLevel = nil
	self.socialDataHandler = SocialDataHandler
	self.socialDataHandler:OnActivate()
	self.territoryTokens = {}
	self:RegisterObservers()
	self.loadScreenNotificationBus = self:BusConnect(LoadScreenNotificationBus, self.entityId)
	self:BusConnect(LandClaimNotificationBus)
	self:BusConnect(MapComponentEventBus)
	self:BusConnect(WarDataNotificationBus)
	self.pvpCurrencyId = 221228154
	local v5_14 = { { warningPercentage = 0.9, seen = false }, { warningPercentage = 0.95, seen = false } }
	self.azothSaltWarningThresholds = v5_14
	self.checkpoints = 0
	self.dataLayer:RegisterAndExecuteDataObserver(
		self,
		"Hud.LocalPlayer.HudComponent.PlayerEntityId",
		function(self, playerEntityId)
			if not playerEntityId then
				return
			end
			if self.categoricalProgressionHandler then
				self:BusDisconnect(self.categoricalProgressionHandler)
				self.categoricalProgressionHandler = nil
			end
			if self.progressionPointHandler then
				self:BusDisconnect(self.progressionPointHandler)
				self.progressionPointHandler = nil
			end
			if self.playerQuickCourseComponentBusHandler then
				self:BusDisconnect(self.playerQuickCourseComponentBusHandler)
				self.playerQuickCourseComponentBusHandler = nil
			end
			self.playerEntityId = playerEntityId
			local forceBanner = self:UpdateTerritoryTokens()
			self:TryPointsBanner(forceBanner)
			self.categoricalProgressionHandler =
				self:BusConnect(CategoricalProgressionNotificationBus, self.playerEntityId)
			self.progressionPointHandler = self:BusConnect(ProgressionPointsNotificationBus, self.playerEntityId)
			self.playerQuickCourseComponentBusHandler =
				self:BusConnect(PlayerQuickCourseComponentNotificationsBus, self.playerEntityId)
			self.currentAzothSalt =
				CategoricalProgressionRequestBus.Event.GetProgression(self.playerEntityId, self.pvpCurrencyId)
			local azothCap =
				CategoricalProgressionRequestBus.Event.GetMaxPointsForRank(self.playerEntityId, self.pvpCurrencyId, 0)
			self.azothSaltCap = azothCap > 0 and azothCap or 1
			self:TrySeasonsRewardsSeasonPassBanner()
		end
	)
	local v5_16 = {
		layouts.LAYOUT_TEXT_CARD,
		layouts.LAYOUT_LEVEL_UP_BANNER,
		layouts.ROW_TERRITORY_LEVEL_UP_BANNER,
		layouts.LAYOUT_WAR_CARD,
		layouts.LAYOUT_TERRITORY_CLAIMED,
		layouts.LAYOUT_ACHIEVEMENT,
		layouts.LAYOUT_TOWN_STRUCTURE_CHANGED,
		layouts.LAYOUT_TOWN_PROJECT_STARTED,
		layouts.LAYOUT_TERRITORY_ENTERED,
		layouts.LAYOUT_TERRITORY_LEVEL_UP_BANNER,
	}
	self.layoutsWithCustomAnimateIn = v5_16
	local v5_17 = {
		layouts.LAYOUT_TEXT_CARD,
		layouts.LAYOUT_LEVEL_UP_BANNER,
		layouts.ROW_TERRITORY_LEVEL_UP_BANNER,
		layouts.LAYOUT_WAR_CARD,
		layouts.LAYOUT_TERRITORY_CLAIMED,
		layouts.LAYOUT_ACHIEVEMENT,
		layouts.LAYOUT_TOWN_STRUCTURE_CHANGED,
		layouts.LAYOUT_TOWN_PROJECT_STARTED,
		layouts.LAYOUT_TERRITORY_ENTERED,
		layouts.LAYOUT_TERRITORY_LEVEL_UP_BANNER,
	}
	self.layoutsWithCustomAnimateOut = v5_17
	self.layoutsWithCustomAnimateOutCallback = { [layouts.LAYOUT_LEVEL_UP_BANNER] = true }
	self.dataLayer:RegisterAndExecuteDataObserver(self, "Hud.LocalPlayer.Faction", function(self, factionType)
		self.localPlayerFaction = factionType
		if self.notifyFactionsConflictsOnFactionSet then
			self.notifyFactionsConflictsOnFactionSet = false
			self:NotifyInitialFactionConflicts()
		end
	end)
	self.dataLayer:RegisterAndExecuteDataObserver(
		self,
		"Hud.LocalPlayer.IsLandClaimManagerAvailable",
		function(self, isAvailable)
			self.landClaimAvailable = isAvailable
			if isAvailable == true then
				local rawClaimKeys = LandClaimRequestBus.Broadcast.GetClaimKeys()
				for i = 1, #rawClaimKeys do
					local claimKey = rawClaimKeys[i]
					local conflictFaction = LandClaimRequestBus.Broadcast.GetTerritoryConflictFaction(claimKey)
					self:OnTerritoryConflictFactionChanged(claimKey, conflictFaction)
				end
				if self.localPlayerFaction then
					self:NotifyInitialFactionConflicts()
				else
					self.notifyFactionsConflictsOnFactionSet = true
				end
				self:TryTerritoryUpkeepNotification()
			end
		end
	)
	self.dataLayer:RegisterAndExecuteDataObserver(
		self,
		"Hud.LocalPlayer.OverpopulateTeleportTime",
		function(self, teleportTime)
			if not teleportTime then
				return
			end
			local now = LocalPlayerComponentRequestBus.Broadcast.GetCurrentSyncedWallClockTime()
			local teleportTimeInSec = teleportTime:Subtract(now):ToSeconds()
			if teleportTimeInSec > 0 then
				self:OnOverpopulationPopup(teleportTimeInSec)
			end
		end
	)
	self.dataLayer:RegisterAndExecuteDataObserver(
		self,
		"Hud.LocalPlayer.FCPOverpopulateTeleportTime",
		function(self, messageTime)
			if messageTime and messageTime > 0 then
				self:OnFCPOverpopulationNotification(messageTime)
			elseif messageTime and messageTime < 0 then
				self:CancelFCPOverpopulationNotification()
			end
		end
	)
	self.usePostSkillCapProgression = ConfigProviderEventBus.Broadcast.GetBool("javelin.enable-post-cap-trade-skills")
end
local overpopPopupId = "OverpopulationPopup"
function BannerTriggers:OnOverpopulationPopup(timeRemainingSeconds)
	local v4 = {}
	v4.title = "@ui_overpopulationPopup"
	v4.message = "@ui_overpopulationPopup_desc"
	v4.eventId = overpopPopupId
	v4.callerSelf = self
	function v4:callback(result, eventId)
		if eventId ~= overpopPopupId then
			return
		end
		LocalPlayerComponentRequestBus.Broadcast.RequestImmediateOverpopulationTeleport()
	end
	v4.buttonText = "@ui_overpopulationPopup_teleport"
	v4.additionalHeight = 30
	local v5_3 = { { detailType = "RemainingTime", value = timeRemainingSeconds } }
	v4.customData = v5_3
	PopupWrapper:RequestPopupWithParams(v4)
	self.isOverpopPopupShowing = true
	if not self.loadScreenNotificationBus then
		self.loadScreenNotificationBus = self:BusConnect(LoadScreenNotificationBus, self.entityId)
	end
end
function BannerTriggers:OnFCPOverpopulationNotification(messageTime)
	local notificationData = NotificationData()
	notificationData.type = "CrowdControl"
	notificationData.priority = eNotificationPriority_High
	notificationData.title = "@ui_fcp_crowdcontrol_title"
	notificationData.icon = "lyshineui/images/seasonsrewards/icon_warning.png"
	notificationData.text =
		GetLocalizedReplacementText("@ui_fcp_crowdcontrol_banner_warning", { TeleportSeconds = messageTime })
	notificationData.maximumDuration = messageTime
	notificationData.showProgress = notificationData.maximumDuration > 0 or false
	notificationData.contextId = self.entityId
	self.fcpOverpopNotificationId = UiNotificationsBus.Broadcast.EnqueueNotification(notificationData)
	self.isFCPOverpopPopupShowing = true
	if not self.loadScreenNotificationBus then
		self.loadScreenNotificationBus = self:BusConnect(LoadScreenNotificationBus, self.entityId)
	end
end
function BannerTriggers:CancelFCPOverpopulationNotification()
	if not self.fcpOverpopNotificationId then
		return
	end
	UiNotificationsBus.Broadcast.RescindNotification(self.fcpOverpopNotificationId, true, true)
	self.fcpOverpopNotificationId = nil
end
function BannerTriggers:OnFCPOverpopulationTeleportedNotification()
	local notificationData = NotificationData()
	notificationData.type = "CrowdControl"
	notificationData.priority = eNotificationPriority_High
	notificationData.title = "@ui_fcp_crowdcontrol_title"
	notificationData.icon = "lyshineui/images/seasonsrewards/icon_warning.png"
	notificationData.text = "@ui_fcp_crowdcontrol_banner_teleported"
	notificationData.maximumDuration = 60
	notificationData.contextId = self.entityId
	self.fcpOverpopNotificationId = UiNotificationsBus.Broadcast.EnqueueNotification(notificationData)
end
function BannerTriggers:OnLoadingScreenShown()
	if self.isOverpopPopupShowing then
		UiPopupBus.Broadcast.HidePopup(overpopPopupId)
		self.isOverpopPopupShowing = false
	end
	if self.isFCPOverpopPopupShowing then
		self.isFCPOverpopPopupShowing = false
		self.showFCPOverPopCompleted = true
	end
	if self.firstLoadingScreenDismissed and self.loadScreenNotificationBus and not self.showFCPOverPopCompleted then
		self:BusDisconnect(self.loadScreenNotificationBus)
		self.loadScreenNotificationBus = nil
	end
end
function BannerTriggers:OnLoadingScreenDismissed()
	self.firstLoadingScreenDismissed = true
	if self.isOverpopPopupShowing then
		UiPopupBus.Broadcast.HidePopup(overpopPopupId)
		self.isOverpopPopupShowing = false
	end
	if self.showFCPOverPopCompleted then
		self:OnFCPOverpopulationTeleportedNotification()
		self.showFCPOverPopCompleted = false
	end
	if self.firstLoadingScreenDismissed and self.loadScreenNotificationBus then
		self:BusDisconnect(self.loadScreenNotificationBus)
		self.loadScreenNotificationBus = nil
	end
end
function BannerTriggers:NotifyInitialFactionConflicts()
	if not self:ShouldShowWarNotifications() then
		return
	end
	if self.localPlayerFaction == eFactionType_None then
		return
	end
	local numInConflict = 0
	for claimKey, factionId in pairs(self.initialConflictFactions) do
		if factionId == self.localPlayerFaction then
			numInConflict = numInConflict + 1
		end
	end
	if numInConflict > 0 then
		local notificationData = NotificationData()
		notificationData.type = "Social"
		notificationData.icon = "LyShineUI/Images/Icons/Misc/icon_warUncolored.dds"
		notificationData.title = "@owg_influence_login_notification_title"
		notificationData.text =
			GetLocalizedReplacementText("@owg_influence_login_notification_desc", { count = numInConflict })
		UiNotificationsBus.Broadcast.EnqueueNotification(notificationData)
	end
end
function BannerTriggers:OnShutdown()
	for _, handler in ipairs(self.notificationHandlers) do
		handler:Disconnect()
	end
	self.socialDataHandler:OnDeactivate()
	self.notificationHandlers = {}
	TimingUtils:StopDelay(self)
	self.pointsBannerDelay = nil
end
function BannerTriggers:BusConnect(bus, param)
	if bus == nil then
		local handler = Log
		handler("Trying to connect a bus that is nil.\n" .. debug.traceback())
		return
	end
	local handler
	if param == nil then
		handler = bus.Connect(self)
	else
		handler = bus.Connect(self, param)
	end
	table.insert(self.notificationHandlers, handler)
	return handler
end
function BannerTriggers:BusDisconnect(bushandler, param)
	if bushandler == nil then
		return
	end
	if param == nil then
		bushandler:Disconnect()
	else
		bushandler:Disconnect(param)
	end
	for index, handler in ipairs(self.notificationHandlers) do
		if handler == bushandler then
			table.remove(self.notificationHandlers, index)
			return
		end
	end
end
function BannerTriggers:GetGuildDetailedDataFailure(reason)
	if reason == eSocialRequestFailureReasonThrottled then
		Log("ERR - BannerTriggers:WarBanner: GuildData request throttled")
	elseif reason == eSocialRequestFailureReasonTimeout then
		Log("ERR - BannerTriggers:WarBanner: GuildData request timed out")
	end
end
function BannerTriggers:RegisterObservers()
	self.dataLayer:RegisterAndExecuteDataCallback(
		self,
		"Hud.LocalPlayer.Siege.SiegePhase",
		function(self, isInSiegePhase)
			self.isPlayerAtWar = isInSiegePhase
		end
	)
	LyShineDataLayerBus.Broadcast.SetData("LyShineUi.Banners.BannerScreenId", self.entityId)
	self.dataLayer:RegisterAndExecuteDataObserver(
		self,
		"UIFeatures.g_uiEnableClaimDamageBanner",
		function(self, enableClaimDamageBanner)
			self.mEnableClaimDamageBanners = enableClaimDamageBanner
		end
	)
	self.dataLayer:RegisterAndExecuteDataObserver(
		self,
		"UIFeatures.g_uiEnableClaimProtectedBanner",
		function(self, enableClaimProtectedBanner)
			self.mEnableClaimProtectedBanners = enableClaimProtectedBanner
		end
	)
	self.dataLayer:RegisterAndExecuteDataObserver(self, "UIFeatures.enable_camp_warning_in_poi", function(self, enabled)
		self.campWarningsEnabled = enabled
	end)
	self.dataLayer:RegisterDataCallback(self, "Hud.LocalPlayer.CurrentAreaTerritory.ClaimKey", function(self, claimKey)
		if not claimKey or claimKey == 0 or LoadScreenBus.Broadcast.IsLoadingScreenShown() then
			return
		end
		self:ShowTerritoryEnteredCard(claimKey, TerritoryEnteredCardTypes.TerritoryType)
	end)
	self.dataLayer:RegisterAndExecuteDataCallback(
		self,
		"Hud.LocalPlayer.HudComponent.OutpostId",
		function(self, outpostId)
			if not self.dataLayer:GetDataFromNode("UIFeatures.g_enableContracts") then
				return
			end
			if outpostId and string.len(outpostId) > 0 and not LoadScreenBus.Broadcast.IsLoadingScreenShown() then
				local claimKey = self.dataLayer:GetDataFromNode("Hud.LocalPlayer.CurrentAreaTerritory.ClaimKey")
				local additionalData = {}
				additionalData.outpostId = outpostId
				self:ShowTerritoryEnteredCard(claimKey, TerritoryEnteredCardTypes.OutpostType, additionalData)
			end
		end
	)
	self:BusConnect(UiTriggerAreaEventNotificationBus)
	self.dataLayer:RegisterAndExecuteDataObserver(
		self,
		"Hud.LocalPlayer.CombatStatus.IsInCombat",
		function(self, isInCombat)
			self.isInCombat = isInCombat or false
			if not self.isInCombat and self.suppressedPointsBanner and not self:ShouldSuppressPointsBanner() then
				self:TryPointsBanner(true)
			end
			self.suppressedPointsBanner = false
		end
	)
	self.dataLayer:RegisterAndExecuteDataObserver(self, "Hud.LocalPlayer.Raid.Id", function(self, raidId)
		if raidId and raidId:IsValid() then
			self.raidId = raidId
			if self.groupsNotificationBusHandler then
				self:BusDisconnect(self.groupsNotificationBusHandler)
				self.groupsNotificationBusHandler = nil
			end
			self.groupsNotificationBusHandler = self:BusConnect(GroupsUINotificationBus)
			local warDetails = raidId and WarDataServiceBus.Broadcast.GetWarForRaid(raidId)
			self.isInWar = warDetails and warDetails:IsValid()
		else
			self.raidId:Reset()
			self:BusDisconnect(self.groupsNotificationBusHandler)
			self.groupsNotificationBusHandler = nil
			self.isInWar = false
		end
	end)
	self.dataLayer:RegisterAndExecuteDataObserver(
		self,
		"Hud.LocalPlayer.WarDataBeenReplicated",
		function(self, replicated)
			if replicated then
				self.warDataReplicationTime = os.time()
			else
				self.warDataReplicationTime = nil
			end
		end
	)
	self.dataLayer:RegisterDataCallback(self, "Hud.LocalPlayer.Guild.LastModifiedGuildWarId", function(self, warId)
		if not warId then
			return
		end
		if not self.firstLoadingScreenDismissed then
			return
		end
		if self.warDataReplicationTime == nil then
			return
		end
		local now = os.time()
		local timeElapsedSinceInitialReplicationSeconds = now - self.warDataReplicationTime
		if timeElapsedSinceInitialReplicationSeconds < 60 then
			return
		end
		if not self:ShouldShowWarNotifications() then
			return
		end
		local warDetails = WarDataClientRequestBus.Broadcast.GetWarDetails(warId)
		local v5_3 = warDetails:GetWarPhase()
		if v5_3 ~= eWarPhase_PreWar then
			return
		end
		local guildIds = vector_GuildId()
		if not warDetails:IsInvasion() then
			guildIds:push_back(warDetails:GetAttackerGuildId())
		end
		guildIds:push_back(warDetails:GetDefenderGuildId())
		local function successCallback(self, results)
			local warDetails = WarDataClientRequestBus.Broadcast.GetWarDetails(warId)
			local attackingGuildData, defendingGuildData
			for i = 1, #results do
				if results[i].guildId == warDetails:GetAttackerGuildId() then
					attackingGuildData = results[i]
				elseif results[i].guildId == warDetails:GetDefenderGuildId() then
					defendingGuildData = results[i]
				end
			end
			local territoryName = TerritoryDataHandler:GetTerritoryNameFromTerritoryId(warDetails:GetTerritoryId())
			local warTitleText = ""
			local warDetailText = ""
			local defendingGuildCrest, attackingGuildCrest
			if warDetails:IsInvasion() then
				warTitleText = "@ui_invasion_declared"
				warDetailText =
					GetLocalizedReplacementText("@ui_invasion_declared_details", { territoryName = territoryName })
			else
				warTitleText = "@ui_war_prewar"
				warDetailText =
					GetLocalizedReplacementText("@ui_war_declared_details", { territoryName = territoryName })
				defendingGuildCrest = defendingGuildData.crestData
				attackingGuildCrest = attackingGuildData.crestData
			end
			local bannerColor = 1
			local phaseEndTime = warDetails:GetPhaseEndTime()
			local isAttacking = self.localPlayerFaction == warDetails:GetAttackerFaction() or false
			local isInvasion = warDetails:IsInvasion()
			self.WAR_BANNER_DISPLAY_DURATION = layouts.WAR_BANNER_DISPLAY_DURATION
			self.audioHelper:PlaySound(self.audioHelper.Banner_WarDeclared)
			self.audioHelper:SwitchMusicDB(
				self.audioHelper.MusicSwitch_Gameplay,
				self.audioHelper.MusicState_WarDeclaration
			)
			local bannerData = {}
			bannerData.dropDuringCombat = self.suppressWarDeclarationBannersDuringCombat
			bannerData.WarCard1 = {
				warTitleText = warTitleText,
				warDetailText = warDetailText,
				phaseEndTime = phaseEndTime,
				isAttacking = isAttacking,
				bannerColor = bannerColor,
				isInvasion = isInvasion,
				defendingGuildCrest = defendingGuildCrest,
				attackingGuildCrest = attackingGuildCrest,
			}
			local priority = 3
			self.banners:EnqueueBanner(
				layouts.LAYOUT_WAR_CARD,
				bannerData,
				self.WAR_BANNER_DISPLAY_DURATION,
				nil,
				nil,
				false,
				priority,
				self.WAR_BANNER_DRAW_ORDER
			)
		end
		local function failureCallback(reason)
			if reason == eSocialRequestFailureReasonThrottled then
				Log("ERR - BannerTriggers:RequestGetGuilds: Throttled")
			elseif reason == eSocialRequestFailureReasonTimeout then
				Log("ERR - BannerTriggers:RequestGetGuilds: Timed Out")
			end
		end
		self.socialDataHandler:RequestGetGuilds_ServerCall(self, successCallback, failureCallback, guildIds)
	end)
	self.dataLayer:RegisterDataCallback(
		self,
		"Hud.LocalPlayer.Guild.LastEnemyClaimDestroyed.EnemyGuild",
		function(self, enemyGuild)
			if not self:ShouldShowWarNotifications() then
				return
			end
			local claimName = self.dataLayer:GetDataFromNode("Hud.LocalPlayer.Guild.LastEnemyClaimDestroyed.ClaimName")
			local claimPosition =
				self.dataLayer:GetDataFromNode("Hud.LocalPlayer.Guild.LastEnemyClaimDestroyed.ClaimPosition")
			local enemyGuildId =
				self.dataLayer:GetDataFromNode("Hud.LocalPlayer.Guild.LastEnemyClaimDestroyed.EnemyGuildId")
			local playerGuildId = self.dataLayer:GetDataFromNode("Hud.LocalPlayer.Guild.Id")
			local playerGuildData = {}
			playerGuildData.guildName = self.dataLayer:GetDataFromNode("Hud.LocalPlayer.Guild.Name")
			playerGuildData.crestData = self.dataLayer:GetDataFromNode("Hud.LocalPlayer.Guild.Crest")
			self.socialDataHandler:GetGuildDetailedData_ServerCall(self, function(self, result)
				if #result <= 0 then
					Log("ERR - BannerTriggers:WarBanner: GuildData request returned with no data")
					return
				end
				local guildData = type(result[1]) == "table" and result[1].guildData or result[1]
				if guildData and guildData:IsValid() then
					local keys = vector_basic_string_char_char_traits_char()
					keys:push_back("claimName")
					local values = vector_basic_string_char_char_traits_char()
					values:push_back(claimName)
					local isAtWar = WarDataClientRequestBus.Broadcast.IsAtWarWithGuild(enemyGuildId)
					local warTitleText = LyShineScriptBindRequestBus.Broadcast.LocalizeTextWithReplacements(
						"@ui_war_claim_destroyed",
						keys,
						values
					)
					local warGuildsText = ""
					local warDetailText = isAtWar and "@ui_war_claimdestroyed_detail" or ""
					local warMessageText = isAtWar and "@ui_claimDestroyed_message"
						or "@ui_war_neutral_claim_marker_destroyed"
					local is2Steps = true
					local isSingleCrest = true
					local bannerColor = 2
					self.WAR_BANNER_DISPLAY_DURATION = layouts.INVASION_BANNER_DISPLAY_DURATION
					local attackingGuildData = playerGuildData
					local defendingGuildData = guildData
					local attackingGuildName = attackingGuildData.guildName
					local defendingGuildName = defendingGuildData.guildName
					if attackingGuildName and defendingGuildName then
						local keys = vector_basic_string_char_char_traits_char()
						keys:push_back("defendingGuildName")
						local values = vector_basic_string_char_char_traits_char()
						values:push_back(defendingGuildName)
						warGuildsText = LyShineScriptBindRequestBus.Broadcast.LocalizeTextWithReplacements(
							"@ui_claimMarkerDestroyed",
							keys,
							values
						)
					end
					keys = self.audioHelper
					keys:PlaySound(self.audioHelper.Banner_WarDeclared)
					self.audioHelper:SwitchMusicDB(
						self.audioHelper.MusicSwitch_Gameplay,
						self.audioHelper.MusicState_WarDeclaration
					)
					local attackingGuildCrest = attackingGuildData.crestData
					local defendingGuildCrest = defendingGuildData.crestData
					local bannerData = {}
					bannerData.WarCard1 = {
						warTitleText = warTitleText,
						warGuildsText = warGuildsText,
						warDurationText = "",
						warMessageText = warMessageText,
						warDetailText = warDetailText,
						phaseEndTime = nil,
						warAttackingGuildCrestData = attackingGuildCrest,
						warDefendingGuildCrestData = defendingGuildCrest,
						is2Steps = is2Steps,
						isSingleCrest = isSingleCrest,
						bannerColor = bannerColor,
					}
					local priority = 3
					self.banners:EnqueueBanner(
						layouts.LAYOUT_WAR_CARD,
						bannerData,
						self.WAR_BANNER_DISPLAY_DURATION,
						nil,
						nil,
						false,
						priority,
						self.WAR_BANNER_DRAW_ORDER
					)
				end
			end, self.GetGuildDetailedDataFailure, enemyGuildId)
		end
	)
	self.dataLayer:RegisterDataCallback(
		self,
		"Hud.LocalPlayer.Guild.LastLockedClaimTaken.ClaimingGuild",
		function(self, claimingGuild)
			if not self:ShouldShowWarNotifications() then
				return
			end
			local claimName = self.dataLayer:GetDataFromNode("Hud.LocalPlayer.Guild.LastLockedClaimTaken.ClaimName")
			local text =
				GetLocalizedReplacementText("@ui_war_claim_taken", { claimName = claimName, guildName = claimingGuild })
			local v4_3 = {}
			v4_3.Text1 = { text = text }
			bannerData = v4_3
			local priority = 3
			local duration = 10
			self.mDamageBannerId = self.banners:EnqueueBanner(
				layouts.LAYOUT_CLAIM_TAKEN_MESSAGE,
				bannerData,
				duration,
				nil,
				nil,
				false,
				priority,
				self.WAR_BANNER_DRAW_ORDER
			)
		end
	)
	self.dataLayer:RegisterAndExecuteDataObserver(self, "Hud.LocalPlayer.Progression.Level", function(self, level)
		local enableGlory = self.dataLayer:GetDataFromNode("UIFeatures.g_uiEnableGloryBar")
		if not enableGlory or not level or level < 1 or self.playerLevel == level then
			return
		end
		local firstTime = not self.playerLevel
		self.playerLevel = level
		if firstTime then
			return
		end
		local bannerData = {}
		bannerData.BannerLevelUp1 = { level = level, play = true }
		local priority = 4
		local duration = layouts.DEFAULT_DISPLAY_DURATION
		local data = DynamicBus.MilestoneWindow.Broadcast.GetDataFromLevel(level)
		if data then
			local isEndGameGuideEnabled = ConfigProviderEventBus.Broadcast.GetBool("UIFeatures.enable-endgame-guide")
			if isEndGameGuideEnabled and level == self.showEndGameBannerLevel then
				LyShineDataLayerBus.Broadcast.SetData("Hud.LocalPlayer.EndgameGuide.Viewed", false)
				bannerData.BannerLevelUp1.isEndGameBanner = true
				bannerData.BannerLevelUp1.isEndGameBannerDuration = duration * 7
				duration = duration * 8
			else
				bannerData.BannerLevelUp1.milestoneData = data
				for i = 1, #data do
					local milestoneData = data[i]
					if milestoneData.type == eMilestoneType_TerritoryRecommendation then
						duration = duration + layouts.DEFAULT_DISPLAY_DURATION
					end
				end
				duration = duration + layouts.DEFAULT_DISPLAY_DURATION
			end
		else
			local showNextMilestone = false
			local enableUpdatedRewardMapping =
				self.dataLayer:GetDataFromNode("UIFeatures.enable-updated-reward-mapping")
			if showNextMilestone and enableUpdatedRewardMapping then
				local nextMilestone = DynamicBus.MilestoneWindow.Broadcast.GetNextMilestoneForLevel(level)
				if nextMilestone > 0 then
					bannerData.BannerLevelUp1.nextMilestone = nextMilestone
					duration = duration * 2
				end
			end
		end
		showNextMilestone = self.banners
		showNextMilestone:EnqueueBanner(
			layouts.LAYOUT_LEVEL_UP_BANNER,
			bannerData,
			duration,
			nil,
			nil,
			false,
			priority,
			self.BANNER_DRAW_ORDER_TOP
		)
	end)
	self.dataLayer:RegisterDataCallback(self, "Hud.LocalPlayer.Journal.NewChapterId", function(self, loreId)
		local loreData = LoreDataManagerBus.Broadcast.GetLoreData(loreId)
		local bannerData = {}
		bannerData.AchievementCard1 = {
			title = "@ui_chapter_discovered_title",
			subject = loreData.title,
			prompt = "@ui_openjournal",
			promptAction = "toggleJournalComponent",
			icon = "lyshineui/images/icons/objectives/icon_lore.png",
			iconColor = UIStyle.COLOR_GRAY_80,
			shouldPlayGlow = true,
		}
		local priority = 4
		self.banners:EnqueueBanner(
			layouts.LAYOUT_ACHIEVEMENT,
			bannerData,
			layouts.DEFAULT_DISPLAY_DURATION,
			nil,
			nil,
			false,
			priority
		)
	end)
	self.dataLayer:RegisterDataCallback(self, "Hud.LocalPlayer.Journal.ChapterComplete", function(self, loreId)
		local loreData = LoreDataManagerBus.Broadcast.GetLoreData(loreId)
		local bannerData = {}
		bannerData.AchievementCard1 = {
			title = "@ui_chapter_complete_title",
			subject = loreData.title,
			prompt = "@ui_openjournal",
			promptAction = "toggleJournalComponent",
			icon = "lyshineui/images/icons/objectives/icon_lore.png",
			iconColor = UIStyle.COLOR_GRAY_80,
			shouldPlayGlow = true,
		}
		local priority = 4
		self.banners:EnqueueBanner(
			layouts.LAYOUT_ACHIEVEMENT,
			bannerData,
			layouts.DEFAULT_DISPLAY_DURATION,
			nil,
			nil,
			false,
			priority
		)
	end)
	self.enableObjectives = false
	if ConfigProviderEventBus.Broadcast.GetBool("javelin.enable-objectives") then
		self.enableObjectives = true
	end
	self.dataLayer:RegisterAndExecuteDataObserver(
		self,
		"Hud.LocalPlayer.HudComponent.GDERootEntityId",
		function(self, rootEntityId)
			if rootEntityId == nil then
				return
			end
			self.rootPlayerId = rootEntityId
			if self.notificationHandler then
				self:BusDisconnect(self.notificationHandler)
				self.notificationHandler = nil
			end
			self.notificationHandler = self:BusConnect(VitalsComponentNotificationBus, self.rootPlayerId)
			if self.enableObjectives then
				self:BusDisconnect(self.objectivesComponentBusHandler)
				self.objectivesComponentBusHandler = self:BusConnect(ObjectivesComponentNotificationsBus, rootEntityId)
				if self.playerArenaEventHandler then
					self:BusDisconnect(self.playerArenaEventHandler)
					self.playerArenaEventHandler = nil
				end
				self.playerArenaEventHandler = self:BusConnect(PlayerArenaEventBus, rootEntityId)
			end
			if self.cutsceneEventHandler then
				self:BusDisconnect(self.cutsceneEventHandler)
				self.cutsceneEventHandler = nil
			end
			self.cutsceneEventHandler = self:BusConnect(PlayerCutsceneComponentNotificationsBus, rootEntityId)
			self.maxLevel = (ProgressionRequestBus.Event.GetMaxLevelForPlayer(rootEntityId) or 0) + 1
		end
	)
	SlashCommands:RegisterSlashCommand("townproject", function(args)
		if #args < 2 then
			return
		end
		local progressionData = TerritoryProgressionData()
		progressionData.description = "Blacksmith Upgrade Tier 2 to Tier 3"
		progressionData.image = "LyShineUI\\Images\\items\\BlacksmithT3.png"
		if args[2] == "start" then
			progressionData.title = "@ui_town_project_started"
			self:OnTownStructureChanged(
				"Brightmark",
				progressionData,
				{},
				UIStyle.COLOR_GREEN_LIGHT,
				UIStyle.COLOR_GREEN
			)
		end
		if args[2] == "upgrade" then
			progressionData.title = "@ui_town_project_completed"
			self:OnTownStructureChanged(
				"Brightmark",
				progressionData,
				{},
				UIStyle.COLOR_YELLOW_GOLD,
				UIStyle.COLOR_YELLOW_GOLD
			)
		end
		if args[2] == "downgrade" then
			local bannerData = {}
			bannerData.TextCard1 = {
				title = (
					GetLocalizedReplacementText("@ui_territory_downgraded_banner", { structure = "Blacksmithing" })
				),
				sound = self.audioHelper.Banner_TerritoryDowngrade,
				musicSwitch = self.audioHelper.MusicSwitch_Gameplay,
				musicState = self.audioHelper.MusicState_Territory_Downgraded,
			}
			local priority = 4
			self.banners:EnqueueBanner(
				layouts.LAYOUT_TEXT_CARD,
				bannerData,
				layouts.DEFAULT_DISPLAY_DURATION,
				nil,
				nil,
				false,
				priority,
				self.BANNER_DRAW_ORDER_TOP
			)
		end
		bannerData = args[2]
		if bannerData == "taken" then
			LyShineDataLayerBus.Broadcast.SetData("Hud.LocalPlayer.Guild.LastLockedClaimTaken.ClaimingGuild", 1)
		end
	end)
	self:BusDisconnect(self.gameEventUiNotificationBusHandler)
	self.gameEventUiNotificationBusHandler = self:BusConnect(GameEventUiNotificationBus)
	self.dataLayer:RegisterAndExecuteDataObserver(self, "Hud.LocalPlayer.Guild.Id", function(self, guildId)
		self.guildId = guildId
		self:TryTerritoryUpkeepNotification()
		if self.guildId then
			self.dataLayer:UnregisterObserver(self, "Hud.LocalPlayer.Guild.Id")
		end
	end)
	self.dataLayer:RegisterAndExecuteDataObserver(
		self,
		"Hud.LocalPlayer.Attributes.UnspentPoints",
		function(self, attributePoints)
			if not attributePoints then
				return
			end
			local forceBanner = self.attributePoints < attributePoints or false
			self.attributePoints = attributePoints
			local currentScreenState = LyShineManagerBus.Broadcast.GetCurrentState()
			if currentScreenState == 3576764016 then
				return
			end
			if forceBanner then
				LyShineDataLayerBus.Broadcast.SetData("Hud.LocalPlayer.Attributes.ScreenChecked", false)
			end
			self:TryPointsBanner(forceBanner)
		end
	)
	self.dataLayer:RegisterAndExecuteDataObserver(
		self,
		"Hud.LocalPlayer.Skills.MasteryPoints",
		function(self, masteryPoints)
			if not masteryPoints then
				return
			end
			local forceBanner = self.masteryPoints < masteryPoints or false
			self.masteryPoints = masteryPoints
			if forceBanner then
				LyShineDataLayerBus.Broadcast.SetData("Hud.LocalPlayer.Skills.ScreenChecked", false)
			end
			self:TryPointsBanner(forceBanner)
		end
	)
	self.dataLayer:RegisterAndExecuteDataObserver(
		self,
		"Hud.LocalPlayer.SeasonsRewards.NumPendingRedeems",
		function(self, redeemCount)
			if not redeemCount then
				return
			end
			local currentScreenState = LyShineManagerBus.Broadcast.GetCurrentState()
			if currentScreenState == 1652736112 then
				return
			end
			if redeemCount ~= 0 and redeemCount ~= self.journeyRedeemableCount then
				self.journeyRedeemableCount = redeemCount
				self:TrySeasonsRewardsJourneyBanner()
			end
		end
	)
	self.dataLayer:RegisterAndExecuteDataObserver(
		self,
		"Hud.LocalPlayer.SeasonsRewards.ActivityCardStampableCount",
		function(self, stampableCount)
			if not stampableCount then
				return
			end
			local currentScreenState = LyShineManagerBus.Broadcast.GetCurrentState()
			if currentScreenState == 1652736112 then
				return
			end
			if stampableCount ~= 0 and stampableCount ~= self.activityCardStampableCount then
				self.activityCardStampableCount = stampableCount
				self:TrySeasonsRewardsActivityCardBanner()
			end
		end
	)
	self.dataLayer:RegisterAndExecuteDataObserver(
		self,
		"Hud.LocalPlayer.Skills.PvpAvailableCheckpoints",
		function(self, checkpoints)
			if not checkpoints then
				return
			end
			local showBanner = self.checkpoints < checkpoints or false
			self.checkpoints = checkpoints
			if showBanner then
				local bannerData = {}
				bannerData.TextCard1 = {
					title = "@ui_pvp_track",
					titleLabel = "@ui_notification_pvp_checkpoint_unlocked",
					icon = "lyshineui/images/skills/pvptrack/icon_pvptrack.dds",
					iconScale = 1.7,
					offset = 70,
					bgOffset = -40,
					showBg = true,
					showLine = true,
					keybindValue = "toggleSkillsComponent",
					hintDescription = "@ui_notification_pvp_reward_options",
				}
				local bannerDisplayTime = 5
				local priority = 3
				DynamicBus.Banner.Broadcast.EnqueueBanner(
					layouts.LAYOUT_TEXT_CARD,
					bannerData,
					bannerDisplayTime,
					nil,
					nil,
					false,
					priority,
					self.BANNER_DRAW_ORDER_TOP
				)
			end
		end
	)
	self.dataLayer:RegisterDataCallback(self, "Hud.LocalPlayer.Damage.OnDownedPlayer", function(self, playerName)
		local chatMessage = BaseGameChatMessage()
		chatMessage.type = eChatMessageType_Group
		chatMessage.isPingMsg = true
		chatMessage.body = GetLocalizedReplacementText("@ui_downed_notification", { playerName = playerName })
		ChatComponentBus.Broadcast.WriteMessageToLocalChat(chatMessage)
		self.audioHelper:PlaySound(self.audioHelper.KnockedDown_Player)
	end)
	self.dataLayer:RegisterDataCallback(self, "Hud.LocalPlayer.Damage.OnKilledPlayer", function(self, playerName)
		local chatMessage = BaseGameChatMessage()
		chatMessage.type = eChatMessageType_Group
		chatMessage.isPingMsg = true
		chatMessage.body = GetLocalizedReplacementText("@ui_killed_notification", { playerName = playerName })
		ChatComponentBus.Broadcast.WriteMessageToLocalChat(chatMessage)
		self.audioHelper:PlaySound(self.audioHelper.Killed_Player)
	end)
end
function BannerTriggers:OnCategoricalProgressionPointsChanged(progressionId, oldPoints, newPoints)
	if self.territoryTokens[progressionId] then
		local unspentTokens = ProgressionPointRequestBus.Event.GetUnspentTokens(self.playerEntityId, progressionId)
		if self.territoryTokens[progressionId] < unspentTokens then
			local forceBanner = self.territoryTokens[progressionId] < unspentTokens or false
			if forceBanner then
				LyShineDataLayerBus.Broadcast.SetData("Hud.LocalPlayer.Map.ScreenChecked", false)
			end
			self.territoryTokens[progressionId] = unspentTokens
			self:TryPointsBanner(forceBanner)
		end
	elseif progressionId == self.pvpCurrencyId then
		if not self.currentAzothSalt then
			return
		end
		if newPoints <= self.currentAzothSalt then
			for _, currentThreshold in ipairs(self.azothSaltWarningThresholds) do
				if newPoints / self.azothSaltCap < currentThreshold.warningPercentage then
					currentThreshold.seen = false
				end
			end
			self.currentAzothSalt = newPoints
			return
		end
		local notificationTitle, notificationDesc
		if newPoints == self.azothSaltCap then
			notificationTitle = "@ui_azoth_salt_max_title"
			notificationDesc = "@ui_azoth_salt_max_desc"
		else
			for _, currentThreshold in ipairs(self.azothSaltWarningThresholds) do
				if
					currentThreshold.warningPercentage <= newPoints / self.azothSaltCap and not currentThreshold.seen
				then
					currentThreshold.seen = true
					notificationTitle = "@ui_azoth_salt_warning_title"
					notificationDesc = GetLocalizedReplacementText(
						"@ui_azoth_salt_warning_desc",
						{ amount = newPoints, maxAmount = self.azothSaltCap }
					)
				end
			end
		end
		if notificationTitle then
			local notificationData = NotificationData()
			notificationData.type = "Social"
			notificationData.icon = FactionCommon.azothSaltImagePath
			notificationData.title = notificationTitle
			notificationData.text = notificationDesc
			UiNotificationsBus.Broadcast.EnqueueNotification(notificationData)
		end
		self.currentAzothSalt = newPoints
	elseif self.usePostSkillCapProgression then
		local tradeSkillData = TradeSkillsCommon:GetTradeSkillDataFromTableId(progressionId)
		if tradeSkillData and tradeSkillData.isPostSkill then
			local progressionData =
				CategoricalProgressionRequestBus.Event.GetCategoricalProgressionData(self.playerEntityId, progressionId)
			local postSkillCapData = CategoricalProgressionRequestBus.Event.GetPostSkillCapProgressionData(
				self.playerEntityId,
				progressionId
			)
			local maxPoints = postSkillCapData.maxPoints > 0 and postSkillCapData.maxPoints or 1
			if oldPoints > 0 then
				if newPoints < oldPoints then
					newPoints = newPoints + maxPoints
				end
				local prevPercent = oldPoints / maxPoints
				local nextPercent = newPoints / maxPoints
				local milestones = {}
				for i = 1, #postSkillCapData.momentRewardPercentages do
					if
						postSkillCapData.momentRewardPercentages[i] < 1
						and prevPercent < postSkillCapData.momentRewardPercentages[i]
						and postSkillCapData.momentRewardPercentages[i] <= nextPercent
					then
						local staticItemData = StaticItemDataManager:GetItem(postSkillCapData:GetItemReward(i - 1))
						local milestone = {}
						milestone.name = staticItemData.displayName
						milestone.icon =
							ItemDataManagerBus.Broadcast.GetHiresIconPath(postSkillCapData:GetItemReward(i - 1))
						milestone.type = eMilestoneType_Minor
						table.insert(milestones, milestone)
					end
				end
				if #milestones > 0 then
					self:QueueTradeskillCelebration(tradeSkillData, milestones, "", "")
				end
			end
		end
	end
end
function BannerTriggers:OnProgressionPointsChanged(pointId, oldLevel, newLevel)
	local pointData = ProgressionPointRequestBus.Event.GetStaticProgressionPointData(self.playerEntityId, pointId)
	if pointData.poolCategory == ePoolCategory_Territory then
		local forceBanner = self:UpdateTerritoryTokens()
		if forceBanner then
			LyShineDataLayerBus.Broadcast.SetData("Hud.LocalPlayer.Map.ScreenChecked", false)
		end
		self:TryPointsBanner(forceBanner)
	end
end
function BannerTriggers:UpdateTerritoryTokens()
	local forceBanner = false
	local claims = MapComponentBus.Broadcast.GetClaims()
	for index = 1, #claims do
		local territoryCrc = Math.CreateCrc32(tostring(claims[index].settlementId))
		local unspentPoints = ProgressionPointRequestBus.Event.GetUnspentTokens(self.playerEntityId, territoryCrc) or 0
		if not self.territoryTokens[territoryCrc] then
			self.territoryTokens[territoryCrc] = 0
		end
		forceBanner = forceBanner or self.territoryTokens[territoryCrc] < unspentPoints
		self.territoryTokens[territoryCrc] = unspentPoints
	end
	return forceBanner
end
function BannerTriggers:GetTotalUnspentTokens()
	local unspent = 0
	for idCrc, tokens in pairs(self.territoryTokens) do
		unspent = unspent + tokens
	end
	return unspent
end
function BannerTriggers:ExecutePointsBanner()
	if self:ShouldSuppressPointsBanner() then
		self.suppressedPointsBanner = true
		return
	end
	self.suppressedPointsBanner = false
	local hasPointsToDisplay = false
	local showAttributePoints = not self.dataLayer:GetDataFromNode("Hud.LocalPlayer.Attributes.ScreenChecked")
			and self.attributePoints > 0
		or false
	local showMasteryPoints = not self.dataLayer:GetDataFromNode("Hud.LocalPlayer.Skills.ScreenChecked")
			and self.masteryPoints > 0
		or false
	local currentScreenState = LyShineManagerBus.Broadcast.GetCurrentState()
	if currentScreenState ~= 3576764016 then
		if showAttributePoints or showMasteryPoints then
			local header1, header2, point1, point2, color1, color2
			if showAttributePoints and showMasteryPoints then
				header1 = "@ui_attribute_point"
				header2 = "@ui_mastery_point"
				point1 = self.attributePoints
				point2 = self.masteryPoints
				color1 = UIStyle.COLOR_XP
				color2 = UIStyle.COLOR_MASTERY
			elseif showAttributePoints then
				header1 = "@ui_attribute_point"
				point1 = self.attributePoints
				color1 = UIStyle.COLOR_XP
			elseif showMasteryPoints then
				header1 = "@ui_mastery_point"
				point1 = self.masteryPoints
				color1 = UIStyle.COLOR_MASTERY
			end
			if header1 then
				local bannerData = {}
				bannerData.dropDuringCombat = self.suppressPointsBannersDuringCombat
				bannerData.TextCard1 = {
					header1 = header1,
					header2 = header2,
					point1 = point1,
					point2 = point2,
					color1 = color1,
					color2 = color2,
					title = "@ui_points_available",
					keybindValue = "toggleSkillsComponent",
				}
				if self.currentSkillPointsBanner then
					self.banners:RescindBanner(self.currentSkillPointsBanner)
				end
				local priority = 3
				self.currentSkillPointsBanner = self.banners:EnqueueBanner(
					layouts.LAYOUT_TEXT_CARD,
					bannerData,
					self.POINT_BANNER_DISPLAY_DURATION,
					nil,
					nil,
					false,
					priority,
					self.BANNER_DRAW_ORDER_TOP
				)
				hasPointsToDisplay = true
			end
		end
	end
	local header1 = self:ShouldShowTerritoryNotifications()
	local showStandingPoints
	if header1 then
		showStandingPoints = not self.dataLayer:GetDataFromNode("Hud.LocalPlayer.Map.ScreenChecked")
				and self.standingTokens > 0
			or false
	end
	if currentScreenState ~= 2477632187 and showStandingPoints then
		local bannerData = {}
		bannerData.TextCard1 = {
			header1 = "@ui_standing_point",
			point1 = self.standingTokens,
			color1 = UIStyle.COLOR_STANDING,
			title = "@ui_points_available",
			keybindValue = "toggleMapComponent",
		}
		if self.curentStandingPointsBanner then
			self.banners:RescindBanner(self.curentStandingPointsBanner)
		end
		local priority = 3
		self.curentStandingPointsBanner = self.banners:EnqueueBanner(
			layouts.LAYOUT_TEXT_CARD,
			bannerData,
			self.POINT_BANNER_DISPLAY_DURATION,
			nil,
			nil,
			false,
			priority,
			self.BANNER_DRAW_ORDER_TOP
		)
		hasPointsToDisplay = true
	end
	if not hasPointsToDisplay then
		TimingUtils:StopDelay(self, self.ExecutePointsBanner)
		self.pointsBannerDelay = nil
	end
end
function BannerTriggers:TrySeasonsRewardsJourneyBanner()
	if FtueSystemRequestBus.Broadcast.IsFtue() then
		return
	end
	local playerLevel = self.dataLayer:GetDataFromNode("Hud.LocalPlayer.Progression.Level")
	local minLevel = ConfigProviderEventBus.Broadcast.GetUInt("javelin.seasons-rewards.min-level-for-journey")
	if self.isInCombat or self.isInWar or playerLevel < minLevel then
		return
	end
	local journeyClaimableCount = seasonsRewardsCommon:GetJourneyTotalClaimableCount()
	if journeyClaimableCount > 0 then
		TimingUtils:Delay(self.POINT_FORCED_TIME, self, self.ExecuteSeasonsRewardsJourneyBanner)
	end
end
function BannerTriggers:ExecuteSeasonsRewardsJourneyBanner()
	local hasItemsToDisplay = false
	local currentScreenState = LyShineManagerBus.Broadcast.GetCurrentState()
	local journeyClaimableCount = seasonsRewardsCommon:GetJourneyTotalClaimableCount()
	if currentScreenState ~= 1652736112 and journeyClaimableCount > 0 then
		local bannerData = {}
		bannerData.dropDuringCombat = true
		bannerData.TextCard1 = {
			header1 = "@seasons_rewards_banner_journey_header",
			point1 = journeyClaimableCount,
			color1 = UIStyle.COLOR_MASTERY,
			title = "@seasons_rewards_banner_journey_title",
			keybindValue = "toggleSeasonsRewardsComponent",
		}
		if self.currentJourneyClaimablesBanner then
			self.banners:RescindBanner(self.currentJourneyClaimablesBanner)
		end
		local priority = 3
		self.currentJourneyClaimablesBanner = self.banners:EnqueueBanner(
			layouts.LAYOUT_TEXT_CARD,
			bannerData,
			self.POINT_BANNER_DISPLAY_DURATION,
			nil,
			nil,
			false,
			priority,
			self.BANNER_DRAW_ORDER_TOP
		)
		hasItemsToDisplay = true
	end
	if not hasItemsToDisplay then
		TimingUtils:StopDelay(self, self.ExecuteSeasonsRewardsJourneyBanner)
	end
end
function BannerTriggers:TrySeasonsRewardsSeasonPassBanner()
	if FtueSystemRequestBus.Broadcast.IsFtue() then
		return
	end
	local playerLevel = self.dataLayer:GetDataFromNode("Hud.LocalPlayer.Progression.Level")
	local minLevel = ConfigProviderEventBus.Broadcast.GetUInt("javelin.seasons-rewards.min-level-for-seasonpass")
	if self.isInCombat or self.isInWar or playerLevel < minLevel then
		return
	end
	local seasonPassClaimableCount = seasonsRewardsCommon:GetSeasonPassClaimableCount()
	if seasonPassClaimableCount > 0 then
		TimingUtils:Delay(self.POINT_FORCED_TIME, self, self.ExecuteSeasonsRewardsSeasonPassBanner)
	end
end
function BannerTriggers:ExecuteSeasonsRewardsSeasonPassBanner()
	local hasItemsToDisplay = false
	local currentScreenState = LyShineManagerBus.Broadcast.GetCurrentState()
	local seasonPassClaimableCount = seasonsRewardsCommon:GetSeasonPassClaimableCount()
	if currentScreenState ~= 1652736112 and seasonPassClaimableCount > 0 then
		local bannerData = {}
		bannerData.dropDuringCombat = true
		bannerData.TextCard1 = {
			header1 = "@seasons_rewards_banner_seasonpass_header",
			point1 = seasonPassClaimableCount,
			color1 = UIStyle.COLOR_MASTERY,
			title = "@seasons_rewards_banner_seasonpass_title",
			keybindValue = "toggleSeasonsRewardsComponent",
		}
		if self.currentSeasonPassClaimablesBanner then
			self.banners:RescindBanner(self.currentSeasonPassClaimablesBanner)
		end
		local priority = 3
		self.currentSeasonPassClaimablesBanner = self.banners:EnqueueBanner(
			layouts.LAYOUT_TEXT_CARD,
			bannerData,
			self.POINT_BANNER_DISPLAY_DURATION,
			nil,
			nil,
			false,
			priority,
			self.BANNER_DRAW_ORDER_TOP
		)
		hasItemsToDisplay = true
	end
	if not hasItemsToDisplay then
		TimingUtils:StopDelay(self, self.ExecuteSeasonsRewardsSeasonPassBanner)
	end
end
function BannerTriggers:TrySeasonsRewardsActivityCardBanner()
	if FtueSystemRequestBus.Broadcast.IsFtue() then
		return
	end
	local playerLevel = self.dataLayer:GetDataFromNode("Hud.LocalPlayer.Progression.Level")
	local minLevel = ConfigProviderEventBus.Broadcast.GetUInt("javelin.seasons-rewards.min-level-for-seasonpass")
	if self.isInCombat or self.isInWar or playerLevel < minLevel then
		return
	end
	local stampableCount = seasonsRewardsCommon:GetActivityCardStampableCount(self.playerEntityId)
	if stampableCount > 0 then
		TimingUtils:Delay(self.POINT_FORCED_TIME, self, self.ExecuteSeasonsRewardsActivityCardBanner)
	end
end
function BannerTriggers:ExecuteSeasonsRewardsActivityCardBanner()
	local hasItemsToDisplay = false
	local currentScreenState = LyShineManagerBus.Broadcast.GetCurrentState()
	local activityCardStampableCount = seasonsRewardsCommon:GetActivityCardStampableCount(self.playerEntityId)
	if currentScreenState ~= 1652736112 and activityCardStampableCount > 0 then
		local bannerData = {}
		bannerData.dropDuringCombat = true
		bannerData.TextCard1 = {
			header1 = "@seasons_rewards_banner_activitycard_header",
			point1 = activityCardStampableCount,
			color1 = UIStyle.COLOR_MASTERY,
			title = "@seasons_rewards_banner_activitycard_title",
			keybindValue = "toggleSeasonsRewardsComponent",
		}
		if self.currentSeasonPassActivityCardBanner then
			self.banners:RescindBanner(self.currentSeasonPassActivityCardBanner)
		end
		local priority = 3
		self.currentSeasonPassActivityCardBanner = self.banners:EnqueueBanner(
			layouts.LAYOUT_TEXT_CARD,
			bannerData,
			self.POINT_BANNER_DISPLAY_DURATION,
			nil,
			nil,
			false,
			priority,
			self.BANNER_DRAW_ORDER_TOP
		)
		hasItemsToDisplay = true
	end
	if not hasItemsToDisplay then
		TimingUtils:StopDelay(self, self.ExecuteSeasonsRewardsActivityCardBanner)
	end
end
function BannerTriggers:TryPointsBanner(forceBanner)
	if self:ShouldSuppressPointsBanner() then
		self.suppressedPointsBanner = true
		return
	end
	self.standingTokens = self:GetTotalUnspentTokens()
	if not self.attributePoints or not self.masteryPoints or not self.standingTokens then
		return
	end
	if self.attributePoints <= 0 and self.masteryPoints <= 0 and self.standingTokens > 0 and forceBanner then
		TimingUtils:StopDelay(self, self.ExecutePointsBanner)
		self.pointsBannerDelay = nil
		TimingUtils:Delay(self.POINT_FORCED_TIME, self, self.ExecutePointsBanner)
	end
end
function BannerTriggers:ShouldSuppressPointsBanner()
	return self.suppressPointsBannersDuringCombat and self.isInCombat or self.isInWar
end
function BannerTriggers:TryTerritoryUpkeepNotification()
	if not self.guildId or not self.landClaimAvailable then
		return
	end
	local rawClaimKeys = LandClaimRequestBus.Broadcast.GetClaimKeys()
	for i = 1, #rawClaimKeys do
		local claimKey = rawClaimKeys[i]
		local governanceData = LandClaimRequestBus.Broadcast.GetTerritoryGovernanceData(claimKey)
		if governanceData.failedToPayUpkeep then
			self:OnTerritoryUpkeepChanged(claimKey, true)
		end
	end
end
function BannerTriggers:OnTerritoryUpkeepChanged(key, taxesDue)
	if FtueSystemRequestBus.Broadcast.IsFtue() then
		return
	end
	if not self:ShouldShowTerritoryNotifications() then
		return
	end
	if taxesDue then
		local ownerData = LandClaimRequestBus.Broadcast.GetClaimOwnerData(key)
		local isInGuild = ownerData.guildId == self.dataLayer:GetDataFromNode("Hud.LocalPlayer.Guild.Id") or false
		if isInGuild then
			local territoryName = TerritoryDataHandler:GetTerritoryNameFromTerritoryId(key)
			local territoryUpkeepLocText =
				GetLocalizedReplacementText("@ui_territory_upkeep_due", { name = territoryName })
			local bannerData = {}
			bannerData.TextCard1 = {
				title = territoryUpkeepLocText,
				sound = self.audioHelper.Banner_TerritoryDowngrade,
				musicSwitch = self.audioHelper.MusicSwitch_Gameplay,
				musicState = self.audioHelper.MusicState_Territory_Downgraded,
				keybindValue = "toggleMapComponent",
			}
			local priority = 4
			self.banners:EnqueueBanner(
				layouts.LAYOUT_TEXT_CARD,
				bannerData,
				layouts.DEFAULT_DISPLAY_DURATION,
				nil,
				nil,
				false,
				priority,
				self.BANNER_DRAW_ORDER_TOP
			)
			local chatMessage = BaseGameChatMessage()
			chatMessage.type = eChatMessageType_System
			chatMessage.body = territoryUpkeepLocText
			ChatComponentBus.Broadcast.WriteMessageToLocalChat(chatMessage)
		end
	end
end
function BannerTriggers:QueueTradeskillCelebration(skillData, milestones, mainLevel, postLevel)
	if #milestones <= 0 then
		return
	end
	local bannerQueue = self.banners:GetBannerQueue(layouts.LAYOUT_LEVEL_UP_BANNER)
	local existingBannerData = self.queuedTradeskillBanners[skillData.name]
	if existingBannerData then
		if bannerQueue.current and bannerQueue.current.uuid ~= existingBannerData.uuid and #bannerQueue.queue > 0 then
			for i = 1, #existingBannerData.milestones do
				local milestone =
					{ name = existingBannerData.milestones[i].name, icon = existingBannerData.milestones[i].icon }
				table.insert(milestones, milestone)
			end
			self.banners:RescindBanner(existingBannerData.uuid)
			self.queuedTradeskillBanners[skillData.name] = nil
		end
	end
	local bannerData = {}
	bannerData.BannerLevelUp1 = {
		level = mainLevel,
		postLevel = postLevel,
		play = true,
		displayName = skillData.locName,
		tradeskill = true,
		milestoneData = milestones,
		iconPath = skillData.icon,
	}
	local priority = 4
	local duration = layouts.DEFAULT_DISPLAY_DURATION * 2
	local v10 = self.queuedTradeskillBanners
	local v11_2 = skillData.name
	v10[v11_2] = {
		uuid = (self.banners:EnqueueBanner(
			layouts.LAYOUT_LEVEL_UP_BANNER,
			bannerData,
			duration,
			nil,
			nil,
			false,
			priority,
			self.BANNER_DRAW_ORDER_TOP
		)),
		milestones = milestones,
	}
end
function BannerTriggers:OnSiegeWarfareStarted(warId)
	if warId == nil then
		return
	end
	local warDetails = WarDataClientRequestBus.Broadcast.GetWarDetails(warId)
	if warDetails:IsInvasion() then
		return
	end
	local guildIds = vector_GuildId()
	guildIds:push_back(warDetails:GetAttackerGuildId())
	guildIds:push_back(warDetails:GetDefenderGuildId())
	local function successCallback(self, results)
		for i = 1, #results do
			local attackingGuildData
			local defendingGuildData
			if results[i].guildId == warDetails:GetAttackerGuildId() then
				attackingGuildData = results[i]
			elseif results[i].guildId == warDetails:GetDefenderGuildId() then
				defendingGuildData = results[i]
			end
		end
		local defendingGuildCrest = defendingGuildData.crestData
		local attackingGuildCrest = attackingGuildData.crestData
		local attackingRaidId = warDetails:GetAttackerRaidId()
		local isAttacking = self.raidId == attackingRaidId or false
		local warCalendar = warDetails:GetRemainingWarSchedule()
		local phaseEndTime = warCalendar[1]:GetPhaseEndTime()
		local bannerColor = 1
		local warTitleText
		if isAttacking then
			warTitleText = "@ui_siege_phase_capture_points_attacker"
			bannerColor = 2
		else
			warTitleText = "@ui_siege_phase_capture_points_defender"
			bannerColor = 3
		end
		self.WAR_BANNER_DISPLAY_DURATION = layouts.WAR_BANNER_DISPLAY_DURATION
		self.audioHelper:PlaySound(self.audioHelper.Banner_WarPhase_Conquest)
		local bannerData = {}
		bannerData.WarCard1 = {
			warTitleText = warTitleText,
			phaseEndTime = phaseEndTime,
			isAttacking = isAttacking,
			bannerColor = bannerColor,
			isInvasion = false,
			isSiegeState = true,
			defendingGuildCrest = defendingGuildCrest,
			attackingGuildCrest = attackingGuildCrest,
		}
		local priority = 3
		self.banners:EnqueueBanner(
			layouts.LAYOUT_WAR_CARD,
			bannerData,
			self.WAR_BANNER_DISPLAY_DURATION,
			nil,
			nil,
			false,
			priority,
			self.WAR_BANNER_DRAW_ORDER
		)
	end
	local function failureCallback(reason)
		if reason == eSocialRequestFailureReasonThrottled then
			Log("ERR - BannerTriggers:RequestGetGuilds: Throttled")
		elseif reason == eSocialRequestFailureReasonTimeout then
			Log("ERR - BannerTriggers:RequestGetGuilds: Timed Out")
		end
	end
	self.socialDataHandler:RequestGetGuilds_ServerCall(self, successCallback, failureCallback, guildIds)
end
function BannerTriggers:OnTerritoryActiveProjectChanged(claimKey, projectData, projectState)
	if FtueSystemRequestBus.Broadcast.IsFtue() then
		return
	end
	if not self:ShouldShowTerritoryNotifications() then
		return
	end
	local ownerData = LandClaimRequestBus.Broadcast.GetClaimOwnerData(claimKey)
	local isInTerritory = claimKey == self.dataLayer:GetDataFromNode("Hud.LocalPlayer.CurrentAreaTerritory.ClaimKey")
		or false
	local isInGuild = ownerData.guildId == self.dataLayer:GetDataFromNode("Hud.LocalPlayer.Guild.Id") or false
	if isInTerritory or isInGuild then
		local territoryName = TerritoryDataHandler:GetTerritoryNameFromTerritoryId(claimKey)
		if projectState == eSettlementProgressionState_Active then
			if isInTerritory then
				local stationUpgrades = {}
				self:OnTownStructureChanged(
					territoryName,
					projectData,
					stationUpgrades,
					UIStyle.COLOR_GREEN_LIGHT,
					UIStyle.COLOR_GREEN,
					"@ui_town_project_started"
				)
			end
			stationUpgrades = BaseGameChatMessage
			local chatMessage = stationUpgrades()
			chatMessage.type = eChatMessageType_System
			chatMessage.body = GetLocalizedReplacementText(
				"@ui_town_project_started_chat",
				{ name = projectData.chatNotificationTitle, territory = territoryName }
			)
			ChatComponentBus.Broadcast.WriteMessageToLocalChat(chatMessage)
		elseif projectState == eSettlementProgressionState_Blocking then
			if isInTerritory then
				local stationUpgrades = {}
				self:OnTownStructureChanged(
					territoryName,
					projectData,
					stationUpgrades,
					UIStyle.COLOR_YELLOW_GOLD,
					UIStyle.COLOR_YELLOW_GOLD,
					"@ui_town_project_completed"
				)
			else
				local notificationData = NotificationData()
				notificationData.title = "@ui_town_project_completed"
				notificationData.text = projectData.title
				notificationData.icon = projectData.icon
				UiNotificationsBus.Broadcast.EnqueueNotification(notificationData)
			end
			notificationData = BaseGameChatMessage
			local chatMessage = notificationData()
			chatMessage.type = eChatMessageType_System
			chatMessage.body = GetLocalizedReplacementText(
				"@ui_town_project_completed_chat",
				{ title = projectData.chatNotificationTitle, territory = territoryName }
			)
			ChatComponentBus.Broadcast.WriteMessageToLocalChat(chatMessage)
		elseif projectState == eSettlementProgressionState_Completed then
			if isInTerritory then
				local stationUpgrades = {}
				self:OnTownStructureChanged(
					territoryName,
					projectData,
					stationUpgrades,
					UIStyle.COLOR_YELLOW_GOLD,
					UIStyle.COLOR_YELLOW_GOLD,
					"@ui_town_project_completed"
				)
			else
				local notificationData = NotificationData()
				notificationData.title = "@ui_town_project_completed"
				notificationData.text = projectData.title
				notificationData.icon = projectData.icon
				UiNotificationsBus.Broadcast.EnqueueNotification(notificationData)
			end
			notificationData = BaseGameChatMessage
			local chatMessage = notificationData()
			chatMessage.type = eChatMessageType_System
			chatMessage.body = GetLocalizedReplacementText(
				"@ui_town_project_completed_chat",
				{ title = projectData.chatNotificationTitle, territory = territoryName }
			)
			ChatComponentBus.Broadcast.WriteMessageToLocalChat(chatMessage)
		elseif projectState == eSettlementProgressionState_Cancelled then
			local notificationData = NotificationData()
			notificationData.title = "@ui_town_project_cancelled"
			notificationData.text =
				GetLocalizedReplacementText("@ui_territory_upgrade_cancelled", { territoryName = territoryName })
			notificationData.icon = projectData.icon
			UiNotificationsBus.Broadcast.EnqueueNotification(notificationData)
			local chatMessage = BaseGameChatMessage()
			chatMessage.type = eChatMessageType_System
			chatMessage.body = GetLocalizedReplacementText(
				"@ui_town_project_cancelled_chat",
				{ title = projectData.chatNotificationTitle, territory = territoryName }
			)
			ChatComponentBus.Broadcast.WriteMessageToLocalChat(chatMessage)
		end
	end
end
function BannerTriggers:OnClaimOwnerChanged(claimId, newOwnerData, oldOwnerData)
	if FtueSystemRequestBus.Broadcast.IsFtue() then
		return
	end
	if not self:ShouldShowTerritoryNotifications() then
		return
	end
	local playerGuildId = self.dataLayer:GetDataFromNode("Hud.LocalPlayer.Guild.Id")
	local newOwnerGuildValid = newOwnerData.guildId and newOwnerData.guildId:IsValid()
	local oldOwnerGuildValid = oldOwnerData.guildId and oldOwnerData.guildId:IsValid()
	local claimDestroyed = not newOwnerGuildValid
	local unownedToOwned = not oldOwnerGuildValid and newOwnerGuildValid
	if unownedToOwned then
		local territoryName = TerritoryDataHandler:GetTerritoryNameFromTerritoryId(claimId)
		local claimedByText = GetLocalizedReplacementText(
			"@ui_territory_claimed",
			{ guildName = newOwnerData.guildName, territoryName = territoryName }
		)
		local bannerData = {}
		bannerData.TerritoryClaimedCard1 = {
			claimedByText = claimedByText,
			guildName = newOwnerData.guildName,
			guildCrestData = newOwnerData.guildCrestData,
		}
		local bannerDisplayTime = 5
		local priority = 4
		self.banners:EnqueueBanner(
			layouts.LAYOUT_TERRITORY_CLAIMED,
			bannerData,
			bannerDisplayTime,
			nil,
			nil,
			false,
			priority,
			self.TERRITORY_CLAIMED_BANNER_DRAW_ORDER
		)
		self.audioHelper:PlaySound(self.audioHelper.LandClaim_Claimed)
		self.audioHelper:SwitchMusicDB(
			self.audioHelper.MusicSwitch_Gameplay,
			self.audioHelper.MusicState_LandClaim_Claimed
		)
	end
	if claimDestroyed and playerGuildId == oldOwnerData.guildId then
		self.audioHelper:PlaySound(self.audioHelper.LandClaim_Destroyed)
		self.audioHelper:SwitchMusicDB(
			self.audioHelper.MusicSwitch_Gameplay,
			self.audioHelper.MusicState_LandClaim_Destroyed
		)
	end
end
function BannerTriggers:UpdateDiscoveredPOI(poiData)
	if poiData.id == "" then
		return
	end
	if poiData.isCharted then
		if poiData.isArea then
			return
		end
		local titleText = poiData.nameLocalizationKey
		local subjectText = "@ui_poi_charted"
		if poiData:HasPoiTag(597936596) then
			local landmarkData =
				MapComponentBus.Broadcast.GetFirstLandmarkByType(poiData.id, eTerritoryLandmarkType_FishingHotspot)
			local level = FishingRequestsBus.Event.GetRequiredLevelByHotspotId(
				self.playerEntityId,
				Math.CreateCrc32(landmarkData.landmarkData)
			)
			if CategoricalProgressionRequestBus.Event.GetRank(self.playerEntityId, 1975517117) < level then
				return
			end
			local landmarkData =
				MapComponentBus.Broadcast.GetFirstLandmarkByType(poiData.id, eTerritoryLandmarkType_FishingHotspot)
			local hotspotId = Math.CreateCrc32(landmarkData.landmarkData)
			local hotspotData = FishingRequestsBus.Event.GetFishingHotspotData(self.playerEntityId, hotspotId)
			titleText = hotspotData.displayName
		end
		local difficultyData = {}
		local poiLevel = MapComponentBus.Broadcast.GetMedianPoiLevel(poiData.id)
		if poiLevel ~= 0 then
			local v6_6 = table.insert
			local v8_5 = {}
			local v9_2 = GetLocalizedReplacementText
			local v11 = {}
			v11.level = tostring(poiLevel)
			v8_5.text = v9_2("@objective_recommendedlevel", v11)
			v8_5.minLevel = poiLevel
			v6_6(difficultyData, v8_5)
		end
		if poiData.groupSize ~= 0 then
			local minGroup, maxGroup = EncounterDataHandler:GetGroupRange(poiData)
			local groupText = tostring(minGroup)
			local groupText = groupText .. " - " .. tostring(maxGroup)
			if minGroup == maxGroup then
				groupText = tostring(maxGroup)
			end
			if maxGroup <= 1 then
				groupText = LyShineScriptBindRequestBus.Broadcast.LocalizeText("@ui_solo")
			end
			local v9_7 = table.insert
			local v11_3 = {}
			v11_3.text = GetLocalizedReplacementText("@objective_recommendedgroup", { group = groupText })
			v11_3.minGroupSize = minGroup
			v9_7(difficultyData, v11_3)
		end
		minGroup = self.banners
		local bannerQueue = minGroup:GetBannerQueue(layouts.LAYOUT_ACHIEVEMENT)
		if
			self.mDiscoveryBannerId
			and bannerQueue.current
			and bannerQueue.current.uuid ~= self.mDiscoveryBannerId
			and #bannerQueue.queue > 0
		then
			self.banners:RescindBanner(self.mDiscoveryBannerId)
			self.mDiscoveryBannerId = nil
		end
		CampCommon:UpdateCampInfo(self.dataLayer)
		local campingData = {}
		if not CampCommon:GetCanPlaceOrDestroyCamp() then
			campingData = nil
		else
			local hasCamp = not CampCommon:GetIsCampAvailable()
			local inRange = true
			if hasCamp then
				local distance = CampCommon:GetCampDistanceValue()
				if distance then
					inRange = distance < 500 or false
				else
					hasCamp = false
				end
			end
			local distance = {}
			distance.hasCamp = hasCamp
			distance.inRange = inRange
			campingData = distance
		end
		hasCamp = self.campWarningsEnabled
		if not hasCamp then
			campingData = nil
		end
		local isFastTravel = false
		if string.find(poiData.mapIconPath, "spirit_shrine") then
			isFastTravel = true
		end
		if isFastTravel then
			local bannerData = {}
			bannerData.TextCard1 = {
				title = "@ui_fast_travel_shrine_activated_header",
				titleLabel = "@ui_fast_travel_shrine_activated_body",
				titleLabelColor = UIStyle.COLOR_GREEN,
				showLine = true,
				showSequence = true,
				showBg = true,
				icon = "LyShineUI/Images/Icons/Banner/fastTravelBannerIcon.dds",
				iconScale = 2,
			}
			local bannerDisplayTime = 5
			local priority = 3
			DynamicBus.Banner.Broadcast.EnqueueBanner(
				layouts.LAYOUT_TEXT_CARD,
				bannerData,
				bannerDisplayTime,
				nil,
				nil,
				false,
				priority
			)
		else
			local bannerData = {}
			bannerData.AchievementCard1 = {
				title = titleText,
				subject = subjectText,
				icon = poiData.mapIconPath,
				iconScale = 2,
				iconColor = UIStyle.COLOR_WHITE,
				shouldPlayGlow = true,
				difficultyData = difficultyData,
				campingData = campingData,
			}
			local bannerDisplayTime = 5
			local priority = 3
			self.mDiscoveryBannerId = self.banners:EnqueueBanner(
				layouts.LAYOUT_ACHIEVEMENT,
				bannerData,
				bannerDisplayTime,
				nil,
				nil,
				false,
				priority
			)
		end
	end
end
function BannerTriggers:OnLeavingPoiObjective(gracePeriodOverTime)
	if self.leavingPoiNotification then
		UiNotificationsBus.Broadcast.RescindNotification(self.leavingPoiNotification, true, true)
	end
	self:StopObjectiveAbandonedMusicTimer()
	local secondsTillLeaving = math.max
	local v3_3 = gracePeriodOverTime:Subtract(TimePoint:Now()):ToSeconds()
	local secondsTillLeaving = secondsTillLeaving(v3_3, 1)
	local notificationData = NotificationData()
	notificationData.title = "@ui_leaving_event_area"
	notificationData.text = "@ui_leaving_event_area_description"
	notificationData.maximumDuration = secondsTillLeaving
	notificationData.showProgress = true
	self.leavingPoiNotification = UiNotificationsBus.Broadcast.EnqueueNotification(notificationData)
	if not self.activeEncounterObjectiveData then
		return
	end
	self:StartObjectiveAbandonedMusicTimer(self.activeEncounterObjectiveData.type, secondsTillLeaving)
end
function BannerTriggers:OnObjectiveTaskCompleted(objectiveTaskInstanceId) end
function BannerTriggers:StopObjectiveAbandonedMusicTimer()
	TimingUtils:StopDelay(self, self.PlayObjectiveAbandonedMusic)
	self.abandonedObjectiveMusicTimerIsSet = false
end
function BannerTriggers:StartObjectiveAbandonedMusicTimer(type, seconds)
	TimingUtils:StopDelay(self, self.PlayObjectiveAbandonedMusic)
	self.abandonedObjectiveType = type
	TimingUtils:Delay(seconds, self, self.PlayObjectiveAbandonedMusic)
	self.abandonedObjectiveMusicTimerIsSet = true
end
function BannerTriggers:PlayObjectiveAbandonedMusic()
	if self.abandonedObjectiveType == eObjectiveType_Arena then
		self.audioHelper:SwitchMusicDB(self.audioHelper.MusicSwitch_Arena, self.audioHelper.MusicState_Arena_Completed)
	end
	self.abandonedObjectiveMusicTimerIsSet = false
	self.abandonedObjectiveType = nil
end
function BannerTriggers:OnReturningToPoiObjective()
	if self.leavingPoiNotification then
		UiNotificationsBus.Broadcast.RescindNotification(self.leavingPoiNotification, true, true)
		self.leavingPoiNotification = nil
	end
	self:StopObjectiveAbandonedMusicTimer()
end
function BannerTriggers:OnObjectiveAdded(objectiveId)
	if LoadScreenBus.Broadcast.IsLoadingScreenShown() then
		return
	end
	local objectiveType = ObjectiveRequestBus.Event.GetType(objectiveId)
	if
		objectiveType == eObjectiveType_Crafting
		or objectiveType == eObjectiveType_Quest
		or objectiveType == eObjectiveType_Journey
		or objectiveType == eObjectiveType_FactionStory_Syndicate
		or objectiveType == eObjectiveType_FactionStory_Marauders
		or objectiveType == eObjectiveType_FactionStory_Covenant
		or objectiveType == eObjectiveType_SeasonQuest
		or objectiveType == eObjectiveType_SkillProgression
		or objectiveType == eObjectiveType_MountRace
		or objectiveType == eObjectiveType_MountUnlock
		or objectiveType == eObjectiveType_EpicEquipment
		or objectiveType == eObjectiveType_Invasion
		or objectiveType == eObjectiveType_Event
		or self:IsEncounter(objectiveType)
	then
		return
	end
	local currentState = LyShineManagerBus.Broadcast.GetCurrentState()
	if currentState == self.TOWN_PROJECTS_STATE and objectiveType == eObjectiveType_CommunityGoal then
		return
	end
	if currentState == self.OWMISSION_BOARD_STATE and objectiveType == eObjectiveType_Mission then
		return
	end
	local isFtue = FtueSystemRequestBus.Broadcast.IsFtue()
	local styleData = ObjectiveTypeData:GetType(objectiveType)
	local objectiveName = ObjectiveRequestBus.Event.GetTitle(objectiveId)
	local titleText = "@objective_started"
	local promptText = not isFtue and "@ui_openjournal" or nil
	local promptActionComponent = "toggleJournalComponent"
	local iconPath = styleData.iconPath
	local iconColor = styleData.iconColor
	local sound
	local difficultyData = {}
	if objectiveType == eObjectiveType_Mission then
		titleText = "@mission_accepted"
	end
	local isMSQ = objectiveType == eObjectiveType_MainStoryQuest or false
	local bannerData = {}
	local v16 = {}
	v16.bgColor = styleData.bgColor
	v16.title = objectiveName
	v16.titleColor = styleData.textColor
	v16.subject = isMSQ and "@objective_main_story_quest" or titleText
	v16.prompt = promptText
	v16.promptAction = promptActionComponent
	v16.icon = iconPath
	v16.iconColor = iconColor
	v16.sound = sound
	v16.difficultyData = difficultyData
	v16.useEffectsForMSQ = isMSQ
	bannerData.AchievementCard1 = v16
	local bannerDisplayTime = 5
	local priority = isMSQ and 5 or 3
	self.banners:EnqueueBanner(
		layouts.LAYOUT_ACHIEVEMENT,
		bannerData,
		bannerDisplayTime,
		nil,
		nil,
		false,
		priority,
		self.BANNER_DRAW_ORDER_TOP
	)
end
function BannerTriggers:IsEncounter(objectiveType)
	if
		objectiveType == eObjectiveType_Darkness_Minor
		or objectiveType == eObjectiveType_Darkness_Major
		or objectiveType == eObjectiveType_Arena
		or objectiveType == eObjectiveType_Dungeon
		or objectiveType == eObjectiveType_DefendObject
		or objectiveType == eObjectiveType_EventEncounter
		or objectiveType == eObjectiveType_SeasonEncounter
		or objectiveType == eObjectiveType_DynamicPOI
		or objectiveType == eObjectiveType_POI
		or objectiveType == eObjectiveType_Scenario
		or objectiveType == eObjectiveType_Trial
	then
		return true
	end
	return false
end
function BannerTriggers:GetAzothStaffInfo(spawnerTag)
	local gatheringEntityId = self.dataLayer:GetDataFromNode("Hud.LocalPlayer.HudComponent.GatheringEntityId")
	local itemDescriptor = ItemDescriptor()
	itemDescriptor.itemId = EncounterDataHandler:GetRequiredItem(spawnerTag)
	local tier = StaticItemDataManager:GetItem(itemDescriptor.itemId).tier
	local hasValidAzothStaff = false
	local equippedAzothStaff =
		UiGatheringComponentRequestsBus.Event.GetValidGatheringToolList(gatheringEntityId, 3374678500)
	if equippedAzothStaff and equippedAzothStaff:IsValid() then
		local equippedTier = StaticItemDataManager:GetItem(equippedAzothStaff:GetItemId()).tier
		if tier <= equippedTier then
			hasValidAzothStaff = true
		end
	else
		local staffIdsByTier = EncounterDataHandler:GetAzothStaffItemIdsByTier()
		for i = tier, #staffIdsByTier do
			local curItemDescriptor = ItemDescriptor()
			curItemDescriptor.itemId = staffIdsByTier[i]
			if inventoryCommon:GetInventoryItemCount(curItemDescriptor) > 0 then
				hasValidAzothStaff = true
				break
			end
		end
	end
	staffIdsByTier = itemDescriptor
	return staffIdsByTier, tier, hasValidAzothStaff
end
function BannerTriggers:OnTrackedObjectiveAdded(objectiveId)
	local objectiveType = ObjectiveRequestBus.Event.GetType(objectiveId)
	local isDarkness = objectiveType == eObjectiveType_Darkness_Minor
		or objectiveType == eObjectiveType_Darkness_Major
		or false
	if not self:IsEncounter(objectiveType) then
		return
	end
	local objectiveName = ObjectiveRequestBus.Event.GetTitle(objectiveId)
	local styleData = ObjectiveTypeData:GetType(objectiveType)
	if objectiveType == eObjectiveType_Dungeon or objectiveType == eObjectiveType_Trial then
		local gameModeId = GameModeParticipantComponentRequestBus.Event.GetCurrentDungeonGameModeId(self.rootPlayerId)
		if gameModeId == 0 then
			return
		end
		local gameModeData =
			GameModeParticipantComponentRequestBus.Event.GetGameModeStaticData(self.rootPlayerId, gameModeId)
		local postFix = ExpeditionsCommon:GetPostFixForGameMode(gameModeId, true)
		local mutationInfo =
			GameModeParticipantComponentRequestBus.Event.GetCurrentDungeonGameModeMutation(self.rootPlayerId)
		local subjectText = ""
		local rewards, additionalTextData
		local possibleItemDropIds = gameModeData:GetPossibleItemDropIds(self.rootPlayerId)
		if #possibleItemDropIds > 0 then
			rewards = possibleItemDropIds
			subjectText = "@ui_available_rewards"
		end
		local isInMutation = mutationInfo.difficultyIndex > 0 or false
		if isInMutation then
			local currentDifficulty = mutationInfo.difficultyIndex
			local difficultyData =
				GameModeMutationSchedulerRequests.Broadcast.GetMutationDifficultyStaticData(currentDifficulty)
			local difficultyText = GetLocalizedReplacementText(
				"@ui_dungeon_mutator_appended_difficulty",
				{ difficulty = currentDifficulty }
			)
			subjectText = difficultyText
			rewards = nil
		end
		local bannerData = {}
		bannerData.AchievementCard1 = {
			title = objectiveName,
			titleColor = styleData.textColor,
			subject = subjectText,
			additionalTextData = additionalTextData,
			rewards = rewards,
			sound = self.audioHelper.Banner_ArenaStarted,
			isMutation = isInMutation,
			shouldPlayGlow = true,
		}
		if gameModeData.dailyLootLimitId ~= 0 and gameModeData.weeklyLootLimitId ~= 0 then
			local v16_4 = {}
			v16_4.AchievementCard1 = {
				title = objectiveName,
				titleColor = styleData.textColor,
				icon = gameModeData.iconPath,
				iconScale = 1.5,
				subject = subjectText,
				additionalTextData = additionalTextData,
				rewards = rewards,
				sound = self.audioHelper.Banner_ArenaStarted,
				shouldPlayGlow = true,
				isInEliteTrial = true,
			}
			bannerData = v16_4
		end
		local bannerDisplayTime = 5
		local priority = 3
		self.banners:EnqueueBanner(
			layouts.LAYOUT_ACHIEVEMENT,
			bannerData,
			bannerDisplayTime,
			nil,
			nil,
			false,
			priority,
			self.BANNER_DRAW_ORDER_TOP
		)
		local isSinglePlayerDungeon = PlayerArenaRequestBus.Event.IsInSinglePlayerGameMode(self.rootPlayerId)
		local maxDailyDungeons = ConfigProviderEventBus.Broadcast.GetUInt("javelin.gamemode-dungeon-base-max-runs")
		local dailyDungeonsCompleted =
			PlayerArenaRequestBus.Event.GetNumDungeonsEnteredSinceLastRefresh(self.rootPlayerId, false)
		local currentDailyDungeonsRemaining = maxDailyDungeons - dailyDungeonsCompleted
		local maxDailyTrials = ConfigProviderEventBus.Broadcast.GetUInt("javelin.gamemode-group-trial-max-runs")
		local groupTrialsCompleted =
			PlayerArenaRequestBus.Event.GetNumGroupTrialsEnteredSinceLastRefresh(self.rootPlayerId)
		local currentTrialsRemaining = maxDailyTrials - groupTrialsCompleted
		local isGroupTrial = ExpeditionsCommon:IsGameModeGroupTrial(gameModeId)
		local description, remainingAmountToUse, maxToUse
		local checkRegular = true
		local maxWeeklyMutated = ConfigProviderEventBus.Broadcast.GetUInt("javelin.gamemode-dungeon-mutated-max-runs")
		local currentWeeklyMutatedCompleted =
			PlayerArenaRequestBus.Event.GetNumDungeonsEnteredSinceLastRefresh(self.rootPlayerId, true)
		local currentWeeklyMutatedRemaining = maxWeeklyMutated - currentWeeklyMutatedCompleted
		if not isSinglePlayerDungeon and mutationInfo.difficultyIndex > 0 then
			checkRegular = false
			if currentWeeklyMutatedRemaining <= self.DUNGEON_LIMIT_WARNING_THRESHOLD then
				remainingAmountToUse = currentWeeklyMutatedRemaining
				maxToUse = maxWeeklyMutated
				if remainingAmountToUse == 0 then
					local v34 = "@ui_dungeon_mutator_max_warning" .. postFix
					description = GetLocalizedReplacementText(v34, { amount = maxWeeklyMutated })
				else
					local v33_5 = GetLocalizedReplacementText
					local v34_2 = "@ui_dungeon_enter_warning_notification_mutated" .. postFix
					local v35_4 = {}
					v35_4.color = ColorRgbaToHexString(UIStyle.COLOR_YELLOW)
					v35_4.remaining = remainingAmountToUse
					v35_4.max = maxToUse
					description = v33_5(v34_2, v35_4)
				end
			end
		end
		local atGroupTrialLimitThreshold
		if isGroupTrial then
			atGroupTrialLimitThreshold = currentTrialsRemaining <= self.DUNGEON_LIMIT_WARNING_THRESHOLD
			atGroupTrialLimitThreshold = currentTrialsRemaining <= self.DUNGEON_LIMIT_WARNING_THRESHOLD or false
		end
		return
	end
	gameModeId = ObjectiveRequestBus
	local objectiveEntityId = gameModeId.Event.GetOwningEntityId(objectiveId)
	local objectiveDefinition = ObjectiveDataHelper:GetDefinitionFromExternalObjective(objectiveId)
	local spawnerTag = SpawnerRequestBus.Event.GetSpawnerTag(objectiveEntityId)
	if not objectiveDefinition then
		local titleText = Debug.Log
		local v11_2 = tostring(objectiveId)
		titleText(
			"BannerTriggers:OnObjectiveAdded attempted to display banner without an available objectiveDefinition ("
				.. v11_2
				.. ")"
		)
		return
	end
	local titleText = "@objective_started"
	local iconPath = styleData.iconPath
	local iconColor = styleData.iconColor
	local sound
	local difficultyData = {}
	if objectiveDefinition.groupSize ~= 0 then
		local minGroup, maxGroup = EncounterDataHandler:GetGroupRange(objectiveDefinition)
		local difficultyText = tostring(minGroup)
		local difficultyText = difficultyText .. " - " .. tostring(maxGroup)
		if minGroup == maxGroup then
			local groupText = tostring(maxGroup)
			difficultyText = GetLocalizedReplacementText("@objective_recommendedgroup", { group = groupText })
		end
		if maxGroup <= 1 then
			local groupText = LyShineScriptBindRequestBus.Broadcast.LocalizeText("@ui_solo")
			difficultyText = GetLocalizedReplacementText("@objective_recommendedgroup", { group = groupText })
		end
		groupText = self.dataLayer
		local maxGroupMembers = groupText:GetDataFromNode("Hud.LocalPlayer.Group.MaxMembers")
		if maxGroupMembers < minGroup then
			difficultyText = GetLocalizedReplacementText("@objective_recommendedplayers", { amount = minGroup })
		end
		table.insert(difficultyData, { text = difficultyText, minGroupSize = minGroup })
	end
	local minLevel = objectiveDefinition.recommendedLevel
	if not minLevel or minLevel == 0 then
		minLevel = EncounterDataHandler:GetLevel(spawnerTag)
	end
	local v15_3 = table.insert
	local v17_8 = {}
	local v18_14 = GetLocalizedReplacementText
	local v20_7 = {}
	v20_7.level = tostring(minLevel)
	v17_8.text = v18_14("@objective_recommendedlevel", v20_7)
	v17_8.minLevel = minLevel
	v15_3(difficultyData, v17_8)
	if isDarkness and self.abandonedObjectiveMusicTimerIsSet then
		self:StopObjectiveAbandonedMusicTimer()
	end
	if objectiveType == eObjectiveType_Darkness_Minor then
		local itemDescriptor, tier, hasValidAzothStaff = self:GetAzothStaffInfo(spawnerTag)
		titleText = "@incursion_started_minor"
		sound = self.audioHelper.Banner_DarknessStarted
		local v18_16 = table.insert
		local v20_8 = {}
		local v21_3 = GetLocalizedReplacementText
		local v23_3 = {}
		v23_3.itemName = itemDescriptor:GetDisplayName()
		v23_3.tier = tier
		v20_8.text = v21_3("@objective_requiresitem", v23_3)
		v20_8.isMet = hasValidAzothStaff
		v18_16(difficultyData, v20_8)
	elseif objectiveType == eObjectiveType_Darkness_Major then
		local itemDescriptor, tier, hasValidAzothStaff = self:GetAzothStaffInfo(spawnerTag)
		titleText = "@incursion_started_major"
		sound = self.audioHelper.Banner_DarknessStarted
		local v18_18 = table.insert
		local v20_9 = {}
		local v21_4 = GetLocalizedReplacementText
		local v23_4 = {}
		v23_4.itemName = itemDescriptor:GetDisplayName()
		v23_4.tier = tier
		v20_9.text = v21_4("@objective_requiresitem", v23_4)
		v20_9.isMet = hasValidAzothStaff
		v18_18(difficultyData, v20_9)
		for i = 1, #difficultyData do
			if difficultyData[i].isMet == false then
				iconPath = string.gsub(iconPath, "%.png$", "_danger.png")
				iconColor = UIStyle.COLOR_RED
			end
		end
	elseif objectiveType == eObjectiveType_Arena then
		titleText = "@arena_started"
		sound = self.audioHelper.Banner_ArenaStarted
		iconColor = UIStyle.COLOR_GREEN_LIGHT
		self.audioHelper:SwitchMusicDB(self.audioHelper.MusicSwitch_Arena, self.audioHelper.MusicState_Arena_Countdown)
	end
	self.activeEncounterObjectiveData = { title = objectiveName, type = objectiveType }
	if objectiveType == eObjectiveType_EventEncounter then
		titleText = "@objective_daily_reward_one"
	end
	if objectiveType == eObjectiveType_SeasonEncounter then
		titleText = GetLocalizedReplacementText("@objective_daily_reward_multiple", { amount = 3 })
	end
	local bannerData = {}
	bannerData.AchievementCard1 = {
		darkness = isDarkness,
		bgColor = styleData.bgColor,
		title = objectiveName,
		titleColor = styleData.textColor,
		subject = titleText,
		promptAction = "toggleJournalComponent",
		icon = iconPath,
		iconColor = iconColor,
		sound = sound,
		difficultyData = difficultyData,
	}
	local bannerDisplayTime = 5
	local priority = 3
	self.banners:EnqueueBanner(
		layouts.LAYOUT_ACHIEVEMENT,
		bannerData,
		bannerDisplayTime,
		nil,
		nil,
		false,
		priority,
		self.BANNER_DRAW_ORDER_TOP
	)
end
function BannerTriggers:OnObjectiveCompleted(objectiveId, objectiveCrcId, objCreation)
	local objectiveData
	local objectiveType
	if objectiveCrcId then
		objectiveData = ObjectivesDataManagerBus.Broadcast.GetObjectiveData(objectiveCrcId)
		objectiveType = objectiveData.type
	elseif objectiveId then
		objectiveData = ObjectiveRequestBus.Event.GetObjectiveData(objectiveId)
		objectiveType = objectiveData.type
	elseif objCreation.isDynamicPoiObjective then
		objectiveType = eObjectiveType_DynamicPOI
	else
		return
	end
	if
		objectiveType == eObjectiveType_Crafting
		or objectiveType == eObjectiveType_Quest
		or objectiveType == eObjectiveType_FactionStory_Syndicate
		or objectiveType == eObjectiveType_FactionStory_Marauders
		or objectiveType == eObjectiveType_FactionStory_Covenant
		or objectiveType == eObjectiveType_SeasonQuest
		or objectiveType == eObjectiveType_Event
	then
		return
	end
	if
		objectiveType ~= eObjectiveType_Journey
		and objectiveType ~= eObjectiveType_SkillProgression
		and objectiveType ~= eObjectiveType_MountRace
		and objectiveType ~= eObjectiveType_MountUnlock
		and objectiveType == eObjectiveType_EpicEquipment
		and objectiveData.npcDestinationId ~= GetNilCrc()
	then
		return
	end
	if objectiveType == eObjectiveType_Dungeon and self.isInMutation == true then
		self.isInMutation = false
	end
	local currentState = LyShineManagerBus.Broadcast.GetCurrentState()
	if currentState == self.TOWN_PROJECTS_STATE and objectiveType == eObjectiveType_CommunityGoal then
		return
	end
	if currentState == self.OWMISSION_BOARD_STATE and objectiveType == eObjectiveType_Mission then
		return
	end
	local styleData = ObjectiveTypeData:GetType(objectiveType)
	local titleColor = UIStyle.COLOR_GREEN_LIGHT
	local iconColor = UIStyle.COLOR_GREEN
	local titleText = "@objective_completed"
	local objectiveName = objectiveData.title
	if objCreation.isDynamicPoiObjective then
		objectiveName = TerritoryDataHandler:GetTerritoryNameFromTerritoryId(objCreation.originTerritoryId)
		titleText = "@dynamic_poi_objective_completed"
	end
	local sound
	if objectiveType == eObjectiveType_Mission then
		titleText = "@mission_completed"
	elseif objectiveType == eObjectiveType_MountRace then
		titleText = "@objective_timetrial_completed"
	elseif self:IsEncounter(objectiveType) then
		return
	end
	local isMSQ = objectiveType == eObjectiveType_MainStoryQuest or false
	if isMSQ then
		self.titleColorOverride = UIStyle.COLOR_WHITE
		self.subjectOverride = "@objective_main_story_quest"
		self.iconColorOverride = UIStyle.COLOR_BRIGHT_YELLOW
	end
	local bannerData = {}
	local v15_2 = {}
	v15_2.bgColor = styleData.bgColor
	v15_2.title = titleText
	v15_2.titleColor = isMSQ and self.titleColorOverride or titleColor
	v15_2.subject = isMSQ and self.subjectOverride or objectiveName
	v15_2.icon = styleData.iconPath
	v15_2.iconColor = isMSQ and self.iconColorOverride or iconColor
	v15_2.shouldPlayGlow = true
	v15_2.scratchOutSubject = true
	v15_2.sound = sound
	v15_2.useEffectsForMSQ = isMSQ
	v15_2.isQuestCompleteBanner = isMSQ
	bannerData.AchievementCard1 = v15_2
	local priority = objectiveType == eObjectiveType_MountRace and 5 or 3
	self.banners:EnqueueBanner(
		layouts.LAYOUT_ACHIEVEMENT,
		bannerData,
		5,
		nil,
		nil,
		false,
		priority,
		self.BANNER_DRAW_ORDER_TOP
	)
end
function BannerTriggers:OnTrackedObjectiveRemoved(objectiveId)
	if not self.activeEncounterObjectiveData or self.abandonedObjectiveMusicTimerIsSet then
		return
	end
	self:StartObjectiveAbandonedMusicTimer(self.activeEncounterObjectiveData.type, 0)
end
if BannerTriggers.DEBUG_OBJECTIVE_COMPLETED then
	function BannerTriggers:OnTrackedObjectiveRemoved(objectiveId)
		ObjectiveDataHelper:DebugLogObjective(objectiveId)
		self:OnObjectiveCompleted(objectiveId)
	end
	function BannerTriggers:OnTrackedObjectiveAdded(objectiveId)
		self:OnObjectiveAdded(objectiveId)
	end
end
function BannerTriggers:OnTaskBannerTriggerActivated(bannerTitle, bannerDescription, parentObjectiveId)
	local objectiveType = ObjectiveRequestBus.Event.GetType(parentObjectiveId)
	if objectiveType == eObjectiveType_Invasion then
		local bannerData = {}
		bannerData.WarCard1 = {
			warTitleText = bannerTitle,
			warGuildsText = "",
			warDurationText = "",
			warMessageText = "",
			warDetailText = bannerDescription,
			isSingleCrest = true,
			bannerColor = 1,
			isInvasion = true,
		}
		local priority = 3
		self.banners:EnqueueBanner(
			layouts.LAYOUT_WAR_CARD,
			bannerData,
			self.WAR_BANNER_DISPLAY_DURATION,
			nil,
			nil,
			false,
			priority,
			self.WAR_BANNER_DRAW_ORDER
		)
		DynamicBus.WarHUD.Broadcast.SetInvasionWaveText(bannerTitle)
	elseif objectiveType == eObjectiveType_Trial or objectiveType == eObjectiveType_Dungeon then
		local iconPath, iconColor, _ = ObjectiveTypeData:GetObjectiveIconByType(objectiveType)
		local v10_2 = layouts.LAYOUT_WAR_CARD
		local v11_2 = {}
		v11_2.WarCard1 = {
			warTitleText = bannerTitle,
			warDetailText = bannerDescription,
			bannerColor = 1,
			customIcon = iconPath,
			customIconColor = iconColor,
		}
		self.banners:EnqueueBanner(
			v10_2,
			v11_2,
			self.WAR_BANNER_DISPLAY_DURATION,
			nil,
			nil,
			false,
			3,
			self.WAR_BANNER_DRAW_ORDER
		)
	else
		if self:IsEncounter(objectiveType) then
			return
		end
		local bannerData
		local v6_3 = {}
		v6_3.TextCard1 = { title = bannerTitle, titleLabel = bannerDescription, keybindValue = "toggleMapComponent" }
		bannerData = v6_3
		local priority = 4
		self.banners:EnqueueBanner(
			layouts.LAYOUT_TEXT_CARD,
			bannerData,
			layouts.DEFAULT_DISPLAY_DURATION,
			nil,
			nil,
			false,
			priority,
			self.BANNER_DRAW_ORDER_TOP
		)
	end
end
function BannerTriggers:OnTypedUiGameEvent(
	gameEventType,
	progressionReward,
	currencyReward,
	itemReward,
	categoricalProgressionId,
	categoricalProgressionReward,
	territoryStandingReward,
	factionRepReward,
	factionTokensReward,
	azothReward
)
	if
		gameEventType ~= eGameEventType_Darkness
		and gameEventType ~= eGameEventType_Arena
		and gameEventType ~= eGameEventType_EventEncounter
		and gameEventType ~= eGameEventType_Scenario
	then
		return
	end
	if not self.activeEncounterObjectiveData then
		return
	end
	local styleData = ObjectiveTypeData:GetType(self.activeEncounterObjectiveData.type)
	local subjectText = self.activeEncounterObjectiveData.title
	local personalBestText, titleText
	if self.activeEncounterObjectiveData.type == eObjectiveType_Darkness_Minor then
		titleText = "@incursion_completed_minor"
		self.audioHelper:SwitchMusicDB(
			self.audioHelper.MusicSwitch_Darkness,
			self.audioHelper.MusicState_Darkness_Completed
		)
	elseif self.activeEncounterObjectiveData.type == eObjectiveType_Darkness_Major then
		titleText = "@incursion_completed_major"
		self.audioHelper:SwitchMusicDB(
			self.audioHelper.MusicSwitch_Darkness,
			self.audioHelper.MusicState_Darkness_Completed
		)
	elseif self.activeEncounterObjectiveData.type == eObjectiveType_Arena then
		titleText = "@arena_completed"
		local newPersonalBestTime = self.dataLayer:GetData("Hud.LocalPlayer.Trial.NewPersonalBestTime")
		if newPersonalBestTime and newPersonalBestTime > 0 then
			personalBestText = "@arena_completed_personal_best"
		end
		self.audioHelper:SwitchMusicDB(self.audioHelper.MusicSwitch_Arena, self.audioHelper.MusicState_Arena_Completed)
	elseif self.activeEncounterObjectiveData.type == eObjectiveType_EventEncounter then
		titleText = "@event_completed"
		local utcTime = 1671454800
		local timeText = timeHelpers:GetLocalizedServerTime(utcTime, true, false)
		subjectText = GetLocalizedReplacementText("@ui_worldboss_daily_reset", { time = timeText })
		titleText = GetLocalizedReplacementText(
			"@ui_worldboss_defeated",
			{ objective = self.activeEncounterObjectiveData.title }
		)
		self.audioHelper:SwitchMusicDB(self.audioHelper.MusicSwitch_Arena, self.audioHelper.MusicState_Arena_Completed)
	elseif self.activeEncounterObjectiveData.type == eObjectiveType_SeasonEncounter then
		titleText = "@event_completed"
		local utcTime = 1671454800
		local timeText = timeHelpers:GetLocalizedServerTime(utcTime, true, false)
		subjectText = GetLocalizedReplacementText("@ui_worldboss_daily_reset", { time = timeText })
		titleText = GetLocalizedReplacementText(
			"@ui_worldboss_defeated",
			{ objective = self.activeEncounterObjectiveData.title }
		)
		self.audioHelper:SwitchMusicDB(self.audioHelper.MusicSwitch_Arena, self.audioHelper.MusicState_Arena_Completed)
	elseif self.activeEncounterObjectiveData.type == eObjectiveType_Scenario then
		titleText = "@event_completed"
		self.audioHelper:SwitchMusicDB(self.audioHelper.MusicSwitch_Arena, self.audioHelper.MusicState_Arena_Completed)
	end
	local bannerData = {}
	bannerData.AchievementCard1 = {
		bgColor = styleData.bgColor,
		title = titleText,
		titleColor = styleData.textColor,
		subject = subjectText,
		icon = styleData.iconPath,
		iconColor = styleData.iconColor,
		shouldPlayGlow = true,
		scratchOutSubject = true,
		sound = self.audioHelper.Banner_DarknessCompleted,
		personalBest = personalBestText,
	}
	local priority = 3
	self.banners:EnqueueBanner(layouts.LAYOUT_ACHIEVEMENT, bannerData, 5, nil, nil, false, priority)
	self:StopObjectiveAbandonedMusicTimer()
	self.activeEncounterObjectiveData = nil
end
function BannerTriggers:OnObjectiveFailed(objectiveInstanceId, objectiveId, missionId)
	local objectiveType = ObjectiveRequestBus.Event.GetType(objectiveInstanceId)
	if
		objectiveType == eObjectiveType_Crafting
		or objectiveType == eObjectiveType_DynamicPOI
		or FtueSystemRequestBus.Broadcast.IsFtue()
	then
		return
	end
	local currentState = LyShineManagerBus.Broadcast.GetCurrentState()
	if currentState == self.TOWN_PROJECTS_STATE and objectiveType == eObjectiveType_CommunityGoal then
		return
	end
	if currentState == self.OWMISSION_BOARD_STATE and objectiveType == eObjectiveType_Mission then
		return
	end
	local objectiveData = ObjectiveRequestBus.Event.GetObjectiveData(objectiveInstanceId)
	if objectiveData.flagPvp then
		local notificationData = NotificationData()
		notificationData.type = "Minor"
		notificationData.text = "@ui_pvp_missions_failed"
		notificationData.allowDuplicates = false
		UiNotificationsBus.Broadcast.EnqueueNotification(notificationData)
		return
	end
	notificationData = ObjectiveRequestBus
	local missionParams = notificationData.Event.GetCreationParams(objectiveInstanceId)
	local objectiveName, _ = ObjectivesDataHandler:GetMissionTitleAndDescription(missionParams, objectiveInstanceId)
	local titleText = "@objective_failed"
	if objectiveType == eObjectiveType_Mission then
		titleText = "@mission_failed"
	elseif objectiveType == eObjectiveType_MountRace then
		titleText = "@objective_timetrial_failed"
	end
	local bannerData = {}
	bannerData.AchievementCard1 = {
		title = titleText,
		titleColor = UIStyle.COLOR_RED,
		subject = objectiveName,
		icon = "lyshineui/images/icons/objectives/icon_objectiveFailed.png",
		iconColor = UIStyle.COLOR_RED,
	}
	local bannerDisplayTime = 5
	local priority = 3
	self.banners:EnqueueBanner(layouts.LAYOUT_ACHIEVEMENT, bannerData, bannerDisplayTime, nil, nil, false, priority)
end
function BannerTriggers:GetNearestNamedTerritory(vec3Pos)
	if not vec3Pos then
		Log("BannerTriggers:GetNearestNamedTerritory(): vec3Pos is invalid, returning nil")
		return nil
	end
	return MapComponentBus.Broadcast.GetNearestNamedTerritory(Vector2(vec3Pos.x, vec3Pos.y))
end
function BannerTriggers:GetBiomeAtPosition(vec3Pos)
	if not vec3Pos then
		Log("BannerTriggers:GetBiomeAtPosition(): vec3Pos is invalid, returning empty string")
		return ""
	end
	local pos = Vector2(vec3Pos.x, vec3Pos.y)
	return MapComponentBus.Broadcast.GetTractAtPosition(pos)
end
function BannerTriggers:AnimateIn(bannerEntityId, layoutName, callback)
	for i = 1, #self.layoutsWithCustomAnimateIn do
		if layoutName == self.layoutsWithCustomAnimateIn[i] then
			local v10 = self.banners:GetRow(self.layoutsWithCustomAnimateIn[i], 1)
			self.banners:TransitionRow(v10, true)
			self.ScriptedEntityTweener:Set(bannerEntityId, { opacity = 0 })
			local duration = 0.2
			local fadeValue = UiFaderBus.Event.GetFadeValue(bannerEntityId)
			duration = (1 - fadeValue) * duration
			self.ScriptedEntityTweener:StartAnimation({
				id = bannerEntityId,
				duration = duration,
				opacity = 1,
				onComplete = callback,
			})
			return true
		end
	end
	return false
end
function BannerTriggers:AnimateOut(bannerEntityId, layoutName, callback)
	for i = 1, #self.layoutsWithCustomAnimateOut do
		if layoutName == self.layoutsWithCustomAnimateOut[i] then
			local v10 = self.banners:GetRow(self.layoutsWithCustomAnimateOut[i], 1)
			self.banners:TransitionRow(v10, false, callback)
			if not FtueSystemRequestBus.Broadcast.IsFtue() or layoutName == layouts.LAYOUT_ACHIEVEMENT then
				if self.layoutsWithCustomAnimateOutCallback[layoutName] then
					callback = nil
				end
				local duration = 1
				local fadeValue = UiFaderBus.Event.GetFadeValue(bannerEntityId)
				duration = fadeValue * duration
				self.ScriptedEntityTweener:StartAnimation({
					id = bannerEntityId,
					duration = duration,
					opacity = 0,
					onComplete = callback,
				})
			end
			duration = true
			return duration
		end
	end
	return false
end
function BannerTriggers:DoesContainMilestone(milestonesTable, name, icon)
	local localizedName = LyShineScriptBindRequestBus.Broadcast.LocalizeText(name)
	for _, entry in pairs(milestonesTable) do
		local localizedEntryName = LyShineScriptBindRequestBus.Broadcast.LocalizeText(entry.name)
		if localizedEntryName == localizedName and entry.icon == icon then
			return true
		end
	end
	return false
end
function BannerTriggers:OnCategoricalProgressionRankChanged(
	progressionId,
	oldRank,
	newRank,
	oldPoints,
	isInitialReplication
)
	if isInitialReplication and FtueSystemRequestBus.Broadcast.IsFtue() == false then
		return
	end
	local progressionData =
		CategoricalProgressionRequestBus.Event.GetCategoricalProgressionData(self.playerEntityId, progressionId)
	if progressionData.rankTableCrc == 1350602995 then
		local weaponMasteryData = WeaponMasteryData:GetByTableNameId(progressionId)
		local bannerData = {}
		bannerData.BannerLevelUp1 = {
			level = newRank + 1,
			play = true,
			weaponMastery = true,
			displayName = weaponMasteryData.text,
			iconPath = weaponMasteryData.icon,
		}
		local priority = 4
		self.banners:EnqueueBanner(
			layouts.LAYOUT_LEVEL_UP_BANNER,
			bannerData,
			layouts.DEFAULT_DISPLAY_DURATION,
			nil,
			nil,
			false,
			priority,
			self.BANNER_DRAW_ORDER_TOP
		)
		LyShineDataLayerBus.Broadcast.SetData("Hud.LocalPlayer.Skills.ScreenChecked", false)
	elseif TradeSkillsCommon:IsGatheringSkill(progressionId) then
		local milestones = {}
		local mainLevel = newRank
		local postLevel
		local skillData = TradeSkillsCommon:GetTradeSkillDataFromTableId(progressionId)
		for i = oldRank + 1, newRank do
			if not skillData.isPostSkill then
				local tradeskillLockedGatherableData =
					CategoricalProgressionRequestBus.Event.GetTradeskillLockedGatherableData(
						self.playerEntityId,
						skillData.name,
						i
					)
				for i = 1, #tradeskillLockedGatherableData do
					local gatherData = tradeskillLockedGatherableData[i]
					if gatherData.iconTypeUnlock and gatherData.iconTypeUnlock ~= "" then
						local icon = string.format(self.TRADESKILL_ICON_PATH, gatherData.iconTypeUnlock)
						if not self:DoesContainMilestone(milestones, gatherData.displayName, icon) then
							local milestone = {}
							milestone.name = gatherData.displayName
							milestone.icon = icon
							table.insert(milestones, milestone)
						end
					end
				end
				local rankData = CategoricalProgressionRequestBus.Event.GetStaticTradeskillRankData(
					self.playerEntityId,
					progressionId,
					i
				)
				if rankData and rankData:IsValid() and rankData.iconTypeUnlock and rankData.iconTypeUnlock ~= "" then
					local icon = string.format(self.TRADESKILL_ICON_PATH, rankData.iconTypeUnlock)
					if not self:DoesContainMilestone(milestones, rankData.displayName, icon) then
						local title =
							GetLocalizedReplacementText("@ui_now_track_banner", { resourceName = rankData.displayName })
						local milestone = {}
						milestone.name = title
						milestone.icon = icon
						table.insert(milestones, milestone)
					end
				end
				if progressionId == 829505831 then
					local musicData = MusicalRewardsDataManagerBus.Broadcast.GetRewardIdsByType(2847879834)
					for j = 1, #musicData do
						local rewardData = MusicalRewardsDataManagerBus.Broadcast.GetRewardData(musicData[j])
						if rewardData and rewardData:GetRewardByScore(0) > 0 and rewardData.rank == i then
							local milestone = {}
							milestone.name = rewardData.name
							milestone.icon = string.format(self.TRADESKILL_ICON_PATH, "bgMusicUnlocked")
							milestone.type = eMilestoneType_Major
							table.insert(milestones, milestone)
						end
					end
				end
			end
			tradeskillLockedGatherableData = self.usePostSkillCapProgression
			if tradeskillLockedGatherableData and skillData.isPostSkill then
				mainLevel = CategoricalProgressionRequestBus.Event.GetMaxRank(
					self.playerEntityId,
					progressionData.preSkillCapSkill
				)
				postLevel = newRank
				local postSkillCapData = CategoricalProgressionRequestBus.Event.GetPostSkillCapProgressionData(
					self.playerEntityId,
					progressionId
				)
				for i = 1, #postSkillCapData.momentRewardPercentages do
					if postSkillCapData.momentRewardPercentages[i] >= 1 then
						local itemId = postSkillCapData:GetItemReward(i - 1)
						local staticItemData = StaticItemDataManager:GetItem(itemId)
						local milestone = {}
						milestone.name = staticItemData.displayName
						milestone.icon = ItemDataManagerBus.Broadcast.GetHiresIconPath(itemId)
						milestone.type = eMilestoneType_Major
						table.insert(milestones, milestone)
					end
				end
			end
		end
		self:QueueTradeskillCelebration(skillData, milestones, mainLevel, postLevel)
	elseif TradeSkillsCommon:IsCraftingSkill(progressionId) then
		local milestones = {}
		local mainLevel = newRank
		local postLevel
		local skillData = TradeSkillsCommon:GetTradeSkillDataFromTableId(progressionId)
		if not skillData.isPostSkill then
			for i = oldRank + 1, newRank do
				local recipeIds = RecipeDataManagerBus.Broadcast.GetCraftingRecipesForTradeskillLevel(skillData.name, i)
				for i = 1, #recipeIds do
					local recipeData = RecipeDataManagerBus.Broadcast.GetCraftingRecipeData(recipeIds[i])
					if recipeData.knownByDefault and recipeData.listedByDefault then
						local isProcedural = RecipeDataManagerBus.Broadcast.IsRecipeProcedural(recipeData.id)
						local resultItemId = Math.CreateCrc32(recipeData.resultItemId)
						local itemData = ItemDataManagerBus.Broadcast.GetItemData(resultItemId)
						local milestone = {}
						milestone.name = recipeData.name ~= "" and recipeData.name or itemData.displayName
						milestone.icon = itemData:GetIconPath()
						table.insert(milestones, milestone)
					end
				end
			end
		end
		if self.usePostSkillCapProgression and skillData.isPostSkill then
			mainLevel =
				CategoricalProgressionRequestBus.Event.GetMaxRank(self.playerEntityId, progressionData.preSkillCapSkill)
			postLevel = newRank
			local postSkillCapData = CategoricalProgressionRequestBus.Event.GetPostSkillCapProgressionData(
				self.playerEntityId,
				progressionId
			)
			for i = 1, #postSkillCapData.momentRewardPercentages do
				if postSkillCapData.momentRewardPercentages[i] >= 1 then
					local itemId = postSkillCapData:GetItemReward(i - 1)
					local staticItemData = StaticItemDataManager:GetItem(itemId)
					local milestone = {}
					milestone.name = staticItemData.displayName
					milestone.icon = ItemDataManagerBus.Broadcast.GetHiresIconPath(itemId)
					milestone.type = eMilestoneType_Major
					table.insert(milestones, milestone)
				end
			end
		end
		self:QueueTradeskillCelebration(skillData, milestones, mainLevel, postLevel)
	elseif TradeSkillsCommon:IsRidingSkill(progressionId) then
		local milestones = {}
		local mainLevel = newRank
		local skillData = TradeSkillsCommon:GetTradeSkillDataFromTableId(progressionId)
		for i = oldRank + 1, newRank do
			if not skillData.isPostSkill then
				local staticRankData = CategoricalProgressionRequestBus.Event.GetStaticTradeskillRankData(
					self.playerEntityId,
					progressionId,
					i
				)
				if staticRankData and staticRankData:IsValid() then
					if
						staticRankData.iconPath
						and staticRankData.iconPath ~= ""
						and not self:DoesContainMilestone(
							milestones,
							staticRankData.displayName,
							staticRankData.iconPath
						)
					then
						local milestone = {
							name = staticRankData.displayName,
							icon = staticRankData.iconPath,
							isHighlighted = staticRankData.isHighlighted,
						}
						table.insert(milestones, milestone)
					end
					milestone = CategoricalProgressionRequestBus
					local rankData = milestone.Event.GetRankData(self.playerEntityId, skillData.tableId, i)
					if rankData.gameEventId ~= GetNilCrc() then
						local gameEventData = GameEventRequestBus.Broadcast.GetGameSystemData(rankData.gameEventId)
						local itemReward = gameEventData.itemReward
						local itemData = ItemDataManagerBus.Broadcast.GetItemData(Math.CreateCrc32(itemReward))
						if
							itemData
							and not self:DoesContainMilestone(milestones, itemData.displayName, itemData:GetIconPath())
						then
							local milestone = {}
							milestone.name = itemData.displayName
							milestone.icon = itemData:GetIconPath()
							milestone.quantity = gameEventData.itemRewardQuantity
							table.insert(milestones, milestone)
						end
					end
				end
			end
		end
		self:QueueTradeskillCelebration(skillData, milestones, mainLevel)
	elseif progressionId == 425035534 then
		if LyShineManagerBus.Broadcast.GetCurrentState() ~= 1652736112 then
			local milestones = {}
			local mainLevel = newRank
			local data = {}
			data.name = "SeasonPass"
			data.locName = "@ui_seasonpass"
			data.isPostSkill = false
			data.icon = "LyShineUI\\Images\\SeasonsRewards\\Crest_Basic.dds"
			data.tableId = 425035534
			local seasonId = SeasonsRewardsRequestBus.Event.GetCurrentSeasonId(self.playerEntityId)
			for i = oldRank + 1, newRank do
				local levelData = SeasonsRewardsBattlePassDataManagerBus.Broadcast.GetDataForLevel(i, seasonId)
				if levelData.freeRewardId ~= nil then
					local freeRewardData =
						SeasonsRewardsDataManagerBus.Broadcast.GetSeasonsRewardData(levelData.freeRewardId, seasonId)
					local freeItemData
					if freeRewardData.displayItemId ~= nil and freeRewardData.displayItemId ~= 0 then
						freeItemData = ItemDataManagerBus.Broadcast.GetItemData(freeRewardData.displayItemId)
					elseif freeRewardData.itemId ~= nil and freeRewardData.itemId ~= 0 then
						freeItemData = ItemDataManagerBus.Broadcast.GetItemData(freeRewardData.itemId)
					end
					if freeItemData ~= nil then
						local icon = freeItemData:GetIconPath()
						if not self:DoesContainMilestone(milestones, freeItemData.displayName, icon) then
							local milestone = {}
							milestone.name = freeItemData.displayName
							milestone.icon = icon
							table.insert(milestones, milestone)
						end
					end
				end
				freeRewardData = levelData.premiumRewardId
				if freeRewardData ~= nil and SeasonsRewardsRequestBus.Broadcast.IsPremiumEnabled() then
					local premiumRewardData =
						SeasonsRewardsDataManagerBus.Broadcast.GetSeasonsRewardData(levelData.premiumRewardId, seasonId)
					local premiumItemData
					if premiumRewardData.displayItemId ~= nil and premiumRewardData.displayItemId ~= 0 then
						premiumItemData = ItemDataManagerBus.Broadcast.GetItemData(premiumRewardData.displayItemId)
					elseif premiumRewardData.itemId ~= nil and premiumRewardData.itemId ~= 0 then
						premiumItemData = ItemDataManagerBus.Broadcast.GetItemData(premiumRewardData.itemId)
					end
					if premiumItemData ~= nil then
						local icon = premiumItemData:GetIconPath()
						if not self:DoesContainMilestone(milestones, premiumItemData.displayName, icon) then
							local milestone = {}
							milestone.name = premiumItemData.displayName
							milestone.icon = icon
							table.insert(milestones, milestone)
						end
					end
				end
			end
			self:QueueTradeskillCelebration(data, milestones, mainLevel, false)
		end
	else
		local showBanner = self:ShouldShowTerritoryNotifications()
		if not self.claimKeys or #self.claimKeys == 0 then
			local rawClaimKeys = LandClaimRequestBus.Broadcast.GetClaimKeys()
			self.claimKeys = {}
			for i = 1, #rawClaimKeys do
				local rawClaimKey = rawClaimKeys[i]
				local v14_8 = table.insert
				local v15_14 = self.claimKeys
				local v16_28 = {}
				v16_28.originalKey = rawClaimKey
				v16_28.crcKey = Math.CreateCrc32(tostring(rawClaimKey))
				v14_8(v15_14, v16_28)
			end
		end
		for i = 1, #self.claimKeys do
			local keyData = self.claimKeys[i]
			if progressionId == keyData.crcKey then
				if showBanner then
					local territoryDefinition =
						TerritoryDefinitionsDataManagerBus.Broadcast.GetTerritoryDefinition(keyData.originalKey)
					local territoryName = territoryDefinition.nameLocalizationKey
					local rankData =
						CategoricalProgressionRequestBus.Event.GetRankData(self.playerEntityId, keyData.crcKey, newRank)
					local bannerData = {}
					bannerData.BannerTerritoryLevelUp1 =
						{ level = newRank, territoryName = territoryName, rankName = rankData.displayName, play = true }
					local priority = 4
					self.banners:EnqueueBanner(
						layouts.LAYOUT_TERRITORY_LEVEL_UP_BANNER,
						bannerData,
						layouts.DEFAULT_DISPLAY_DURATION,
						nil,
						nil,
						false,
						priority,
						self.BANNER_DRAW_ORDER_TOP
					)
					territoryDefinition = LyShineDataLayerBus
					territoryDefinition.Broadcast.SetData("Hud.LocalPlayer.Map.ScreenChecked", false)
					break
				end
			end
		end
	end
end
function BannerTriggers:OnTownStructureChanged(
	territoryName,
	progressionData,
	benefits,
	primaryColor,
	secondaryColor,
	projectStatus
)
	if not self:ShouldShowTerritoryNotifications() then
		return
	end
	local bannerData = {}
	bannerData.holdDuringCombat = self.suppressStationBannersDuringCombat
	bannerData.TownStructureChanged1 = {
		territoryName = territoryName,
		title = progressionData.title,
		description = progressionData.description,
		imagePath = progressionData.image,
		benefits = benefits,
		play = true,
		primaryColor = primaryColor,
		secondaryColor = secondaryColor,
		projectStatus = projectStatus,
	}
	local priority = 4
	self.banners:EnqueueBanner(
		layouts.LAYOUT_TOWN_STRUCTURE_CHANGED,
		bannerData,
		6,
		nil,
		nil,
		false,
		priority,
		self.BANNER_DRAW_ORDER_TOP
	)
end
function BannerTriggers:OnTerritoryProgressionChanged(key, category, prevLevel, level, projectId)
	if FtueSystemRequestBus.Broadcast.IsFtue() then
		return
	end
	if not self:ShouldShowTerritoryNotifications() then
		return
	end
	if level < prevLevel then
		local projectData = TerritoryDataHandler:GetTerritoryProjectDataFromProjectId(projectId)
		local territoryName = TerritoryDataHandler:GetTerritoryNameFromTerritoryId(key)
		local bannerData = {}
		bannerData.holdDuringCombat = self.suppressStationBannersDuringCombat
		bannerData.TextCard1 = {
			title = (GetLocalizedReplacementText(
				"@ui_territory_downgraded_banner",
				{ structure = projectData.projectCategoryName, territoryName = territoryName }
			)),
			sound = self.audioHelper.Banner_TerritoryDowngrade,
			musicSwitch = self.audioHelper.MusicSwitch_Gameplay,
			musicState = self.audioHelper.MusicState_Territory_Downgraded,
		}
		local priority = 4
		self.banners:EnqueueBanner(
			layouts.LAYOUT_TEXT_CARD,
			bannerData,
			layouts.DEFAULT_DISPLAY_DURATION,
			nil,
			nil,
			false,
			priority,
			self.BANNER_DRAW_ORDER_TOP
		)
		local chatMessage = BaseGameChatMessage()
		chatMessage.type = eChatMessageType_System
		chatMessage.body = GetLocalizedReplacementText(
			"@ui_territory_downgraded_chat",
			{ structure = projectData.projectCategoryName, territoryName = territoryName }
		)
		ChatComponentBus.Broadcast.WriteMessageToLocalChat(chatMessage)
	end
end
function BannerTriggers:OnRespawn()
	self:TryPointsBanner(true)
end
function BannerTriggers:OnTerritoryConflictFactionChanged(key, factionType)
	if FtueSystemRequestBus.Broadcast.IsFtue() then
		return
	end
	if not self.initialConflictFactions then
		self.initialConflictFactions = {}
	end
	if
		self:ShouldShowWarNotifications()
		and self.initialConflictFactions[key] ~= nil
		and self.initialConflictFactions[key] ~= factionType
		and factionType ~= eFactionType_None
	then
		local factionData = FactionCommon.factionInfoTable[factionType]
		local factionName = ""
		if factionData then
			factionName = factionData.factionName
		end
		local locText = GetLocalizedReplacementText
		local v7 = {}
		v7.territoryName = TerritoryDataHandler:GetTerritoryNameFromTerritoryId(key)
		v7.faction = factionName
		local locText = locText("@owg_influence_conflict_notification_desc", v7)
		local notificationData = NotificationData()
		notificationData.type = "Social"
		notificationData.icon = "LyShineUI/Images/Icons/Misc/icon_warUncolored.dds"
		notificationData.title = "@owg_influence_conflict_notification_title"
		notificationData.text = locText
		UiNotificationsBus.Broadcast.EnqueueNotification(notificationData)
		local chatMessage = BaseGameChatMessage()
		chatMessage.type = eChatMessageType_System
		chatMessage.body = locText
		ChatComponentBus.Broadcast.WriteMessageToLocalChat(chatMessage)
	end
	factionData = self.initialConflictFactions
	factionData[key] = factionType
end
function BannerTriggers:OnTerritoryConflictLotteryEndTimeChanged(key, endTime)
	if FtueSystemRequestBus.Broadcast.IsFtue() then
		return
	end
	if not self:ShouldShowWarNotifications() then
		return
	end
	local now = LocalPlayerComponentRequestBus.Broadcast.GetCurrentSyncedWallClockTime()
	if not now then
		return
	end
	local timeUntilLotteryEnd = endTime:Subtract(now):ToSecondsRoundedUp()
	local notificationTolerance = 60
	if notificationTolerance < timeUntilLotteryEnd then
		local territoryName = TerritoryDataHandler:GetTerritoryNameFromTerritoryId(key)
		local factionType = self.initialConflictFactions[key]
		local factionData = FactionCommon.factionInfoTable[factionType]
		local factionName = ""
		if factionData then
			factionName = factionData.factionName
		end
		if not factionName then
			return
		end
		local locText = GetLocalizedReplacementText
		local v12 = {}
		v12.territoryName = territoryName
		v12.faction = factionName
		v12.time = timeHelpers:ConvertToShorthandString(timeUntilLotteryEnd)
		local locText = locText("@owg_war_declared_lottery_active_desc", v12)
		local notificationData = NotificationData()
		notificationData.type = "Social"
		notificationData.icon = "LyShineUI/Images/Icons/Misc/icon_warUncolored.dds"
		notificationData.title =
			GetLocalizedReplacementText("@owg_war_declared_lottery_active", { territoryName = territoryName })
		notificationData.text = locText
		UiNotificationsBus.Broadcast.EnqueueNotification(notificationData)
		local chatMessage = BaseGameChatMessage()
		chatMessage.type = eChatMessageType_System
		chatMessage.body = locText
		ChatComponentBus.Broadcast.WriteMessageToLocalChat(chatMessage)
	end
end
function BannerTriggers:OnUiTriggerAreaEventEntered(enteringEntityId, triggerEntityId, eventId, identifier)
	local isInLoadingScreen = LoadScreenBus.Broadcast.IsLoadingScreenShown()
	local cardType, additionalData
	if eventId == 3718191953 then
		cardType = TerritoryEnteredCardTypes.SettlementType
		local claimKey = self.dataLayer:GetDataFromNode("Hud.LocalPlayer.CurrentAreaTerritory.ClaimKey")
		LyShineDataLayerBus.Broadcast.SetData("Hud.LocalPlayer.EnteredSettlementId", claimKey)
		self.enteredSettlementTime = WallClockTimePoint:Now()
		local playerPosition = self.dataLayer:GetDataFromNode("Hud.LocalPlayer.Position")
		if playerPosition then
			self.enteredSettlementPOIId =
				PlayerHousingClientRequestBus.Broadcast.GetFastTravelToTerritoryIdByPosition(playerPosition, true)
		end
	elseif eventId == 114609139 then
		cardType = TerritoryEnteredCardTypes.FortType
	elseif additionalData then
		cardType = TerritoryEnteredCardTypes.HQType
		additionalData.eventId = eventId
	elseif
		TerritoryDefinitionsDataManagerBus.Broadcast.GetTerritoryDefinitionFromStr(identifier)
		and TerritoryDefinitionsDataManagerBus.Broadcast.GetTerritoryDefinitionFromStr(identifier).nameLocalizationKey
			~= ""
	then
		local locKey = territoryDefn.nameLocalizationKey
		local localizedEvent = LyShineScriptBindRequestBus.Broadcast.LocalizeText(locKey)
		if localizedEvent and localizedEvent ~= locKey then
			cardType = TerritoryEnteredCardTypes.OpenWorld
			local gameModeId =
				GameModeParticipantComponentRequestBus.Event.GetCurrentDungeonGameModeId(self.rootPlayerId)
			local gameModeData =
				GameModeParticipantComponentRequestBus.Event.GetGameModeStaticData(self.rootPlayerId, gameModeId)
			local isInMutatedDungeon = false
			if gameModeData.isDungeon ~= 0 then
				local mutationInfo =
					GameModeParticipantComponentRequestBus.Event.GetCurrentDungeonGameModeMutation(self.rootPlayerId)
				if mutationInfo and mutationInfo.difficultyIndex > 0 then
					isInMutatedDungeon = true
				end
			end
			if isInMutatedDungeon then
				additionalData = { name = locKey, eventId = eventId, mutatedDungeon = true }
			else
				additionalData = { name = locKey, eventId = eventId }
			end
		end
	end
	if cardType and not isInLoadingScreen then
		local claimKey = self.dataLayer:GetDataFromNode("Hud.LocalPlayer.CurrentAreaTerritory.ClaimKey")
		self:ShowTerritoryEnteredCard(claimKey, cardType, additionalData)
	end
end
function BannerTriggers:OnUiTriggerAreaEventExited(enteringEntityId, eventId)
	if LoadScreenBus.Broadcast.IsLoadingScreenShown() then
		return
	end
	if eventId == 3718191953 then
		LyShineDataLayerBus.Broadcast.SetData("Hud.LocalPlayer.EnteredSettlementId", 0)
		TimingUtils:Delay(1, self, function(self)
			if LoadScreenBus.Broadcast.IsLoadingScreenShown() then
				return
			end
			if
				self.enteredSettlementTime
				and WallClockTimePoint:Now():Subtract(self.enteredSettlementTime):ToSeconds()
					< self.TOWN_CHECKIN_THRESHOLD
			then
				return
			end
			local bannerTitle, bannerDescription
			local fastTravelCommon = RequireScript("LyShineUI._Common.FastTravelCommon")
			local currentInnTerritoryId = fastTravelCommon:GetCurrentlySetInnTerritoryId()
			local bannerIcon = "LyShineUI\\Images\\icons\\objectives\\npc_inn.dds"
			local showBanner = true
			local titleRefresh = false
			if currentInnTerritoryId == self.enteredSettlementPOIId then
				local currentInnCooldownTime = fastTravelCommon:GetCurrentlySetInnCooldownTime()
				if currentInnCooldownTime <= 0 then
					showBanner = false
				else
					local v9_2 = GetLocalizedReplacementText
					local v11 = {}
					v11.numMinutes = tostring(fastTravelCommon:GetInnCooldownTimeMinutes())
					bannerDescription = v9_2("@ui_leaving_settlement_recall_time_desc", v11)
					local timeBeforeRecall = timeHelpers:ConvertSecondsToHrsMinSecString(currentInnCooldownTime)
					bannerTitle =
						GetLocalizedReplacementText("@ui_leaving_settlement_recall_time", { time = timeBeforeRecall })
					titleRefresh = true
				end
			elseif currentInnTerritoryId == 0 then
				bannerDescription = "@ui_leaving_settlement_no_inn_desc"
				bannerIcon = "LyShineUI\\Images\\icons\\objectives\\npc_inn_inactive.dds"
				bannerTitle = "@ui_leaving_settlement_no_inn"
			else
				local hasInnHomePoint = true
			end
			if showBanner then
				local bannerData = {}
				bannerData.TextCard1 = {
					title = bannerTitle,
					titleLabel = bannerDescription,
					showLine = true,
					showBg = true,
					icon = bannerIcon,
					titleRefresh = titleRefresh,
					titleLocTag = "@ui_leaving_settlement_recall_time",
					titleWallClock = (fastTravelCommon:GetCurrentlySetInnCooldownTime(true)),
				}
				self.banners:EnqueueBanner(layouts.LAYOUT_TEXT_CARD, bannerData, 5, nil, nil, false, 5)
			end
			self.enteredSettlementPOIId = 0
		end)
	end
end
function BannerTriggers:ShowTimeTrialFailedNotification(objectiveInstanceId)
	local objectiveName = ObjectiveRequestBus.Event.GetTitle(objectiveInstanceId)
	local bannerData = {}
	bannerData.AchievementCard1 = {
		title = "@objective_timetrial_failed",
		titleColor = UIStyle.COLOR_RED,
		subject = objectiveName,
		icon = "lyshineui/images/icons/objectives/icon_objectiveFailed.png",
		iconColor = UIStyle.COLOR_RED,
	}
	local bannerDisplayTime = 5
	local priority = 5
	self.banners:EnqueueBanner(layouts.LAYOUT_ACHIEVEMENT, bannerData, bannerDisplayTime, nil, nil, false, priority)
end
function BannerTriggers:ShowTimeTrialRestartPromptNotification()
	local notificationData = NotificationData()
	notificationData.type = "DungeonInvite"
	notificationData.maximumDuration =
		ConfigProviderEventBus.Broadcast.GetFloat("javelin.mount-race-teleport-time-limit")
	local v2_2 = GetLocalizedReplacementText
	local v4 = {}
	v4.color = ColorRgbaToHexString(UIStyle.COLOR_RED)
	notificationData.title = v2_2("@objective_timetrial_timeout_teleport_prompt_title", v4)
	notificationData.text = "@objective_timetrial_timeout_teleport_prompt_description"
	notificationData.icon = "lyshineui/images/icons/objectives/icon_objectiveFailed_colored.png"
	notificationData.hasChoice = true
	notificationData.declineOnTimeout = true
	notificationData.contextId = self.banners.entityId
	notificationData.callbackName = "OnTeleportRaceStartNotificationChoice"
	self.timeTrialRestartPromptNotificationId = UiNotificationsBus.Broadcast.EnqueueNotification(notificationData)
end
function BannerTriggers:OnQuickCourseNodeHit(courseId, sectionIndex, sfx, secondsToAdd, addTime)
	if Math.IsClose(secondsToAdd, 0) then
		return
	end
	local showNotification = false
	local notificationData = NotificationData()
	notificationData.type = "Minor"
	local v8_2 = GetLocalizedReplacementText
	local v10 = {}
	v10.secondsToAdd = GetFormattedNumber(secondsToAdd, 0)
	notificationData.text = v8_2(addTime and "@objective_timetrial_addTime" or "@objective_timetrial_timeLeft", v10)
	local objectiveInstanceId =
		ObjectivesComponentRequestBus.Event.GetObjectiveIdFromQuickCourseId(self.playerEntityId, courseId)
	local objectiveData = ObjectiveRequestBus.Event.GetObjectiveData(objectiveInstanceId)
	if objectiveData.type == eObjectiveType_MountRace then
		showNotification = true
	end
	if showNotification then
		local tutorialCompleted = false
		local territories = MapComponentBus.Broadcast.GetTerritories()
		for i = 1, #territories do
			if tutorialCompleted then
				break
			end
			local horseVendor = MapComponentBus.Broadcast.GetFirstLandmarkByType(
				territories[i].id,
				eTerritoryLandmarkType_HorseProvider
			)
			local npcId = Math.CreateCrc32(horseVendor.landmarkData)
			local availableConversationServices =
				ConversationRequestBus.Event.GetAvailableConversationServices(self.playerEntityId, npcId)
			for k = 1, #availableConversationServices do
				if availableConversationServices[k] == eConversationServices_Inn then
					tutorialCompleted = true
					break
				end
			end
		end
		local numNodes =
			PlayerQuickCourseComponentRequestBus.Event.GetQuickCourseSectionSize(self.playerEntityId, courseId)
		local isTour = numNodes <= 2 or false
		if tutorialCompleted and isTour and sectionIndex == 0 then
			notificationData.text = GetLocalizedReplacementText("@objective_timetrial_tourStart")
		elseif not tutorialCompleted and sectionIndex < 3 then
			local v14_4 = GetLocalizedReplacementText
			local v16 = {}
			v16.key = LyShineManagerBus.Broadcast.GetKeybind("mount_dash", "player")
			notificationData.text = v14_4("@objective_timetrial_tutorial", v16)
		end
	end
	if showNotification then
		UiNotificationsBus.Broadcast.EnqueueNotification(notificationData)
	end
end
function BannerTriggers:OnTeleportRequestDeclined()
	if not self.timeTrialRestartPromptNotificationId then
		return
	end
	UiNotificationsBus.Broadcast.RescindNotification(self.timeTrialRestartPromptNotificationId, true, true)
	self.timeTrialRestartPromptNotificationId = nil
end
function BannerTriggers:OnQuickCourseEnded(courseId, reason)
	local objectiveInstanceId =
		ObjectivesComponentRequestBus.Event.GetObjectiveIdFromQuickCourseId(self.playerEntityId, courseId)
	local objectiveData = ObjectiveRequestBus.Event.GetObjectiveData(objectiveInstanceId)
	if objectiveData.type ~= eObjectiveType_MountRace then
		return
	end
	local shouldShowRestartPrompt = ConfigProviderEventBus.Broadcast.GetBool("javelin.mount-race-teleport-feature")
	if not self.isInCutscene then
		if reason == eCourseEndReason_PlayerTimeOut then
			local playerRootEntityId = self.dataLayer:GetDataFromNode("Hud.LocalPlayer.HudComponent.GDERootEntityId")
			local flaggedForPvp = FactionRequestBus.Event.IsPvpFlaggedOrPending(playerRootEntityId)
			if not shouldShowRestartPrompt or flaggedForPvp then
				self:ShowTimeTrialFailedNotification(objectiveInstanceId)
			else
				self:ShowTimeTrialRestartPromptNotification()
			end
		elseif
			reason ~= eCourseEndReason_Completed
			and reason ~= eCourseEndReason_FromTask
			and reason ~= eCourseEndReason_PlayerLogOut
		then
			self:ShowTimeTrialFailedNotification(objectiveInstanceId)
		end
	elseif reason == eCourseEndReason_PlayerTimeOut and shouldShowRestartPrompt then
		PlayerQuickCourseComponentRequestBus.Event.CancelOrCompleteTeleportToQuickCourseStart(self.playerEntityId)
	end
end
function BannerTriggers:OnTeleportRaceStartNotificationChoice(isAccepted)
	if isAccepted then
		PlayerQuickCourseComponentRequestBus.Event.RequestTeleportToQuickCourseStart(self.playerEntityId)
		self.timeTrialRestartPromptNotificationId = nil
	end
	PlayerQuickCourseComponentRequestBus.Event.CancelOrCompleteTeleportToQuickCourseStart(self.playerEntityId)
end
function BannerTriggers:ShowTerritoryEnteredCard(claimKey, territoryEnteredCardType, additionalData)
	if self.isPlayerAtWar then
		return
	end
	local territoryDefinition = TerritoryDefinitionsDataManagerBus.Broadcast.GetTerritoryDefinition(claimKey)
	local isClaimable = LandClaimRequestBus.Broadcast.GetIsClaimable(claimKey)
	local hasSecondPhase = territoryEnteredCardType == TerritoryEnteredCardTypes.TerritoryType and isClaimable
	local duration = hasSecondPhase and 9 or layouts.DEFAULT_DISPLAY_DURATION
	local priority = 4
	local bannerData = {}
	bannerData.TerritoryEnteredCard1 = { isClaimable = isClaimable, hasSecondPhase = hasSecondPhase, showBg = true }
	if territoryEnteredCardType == TerritoryEnteredCardTypes.OutpostType then
		if isClaimable then
			return
		end
		local outpostCapitals = MapComponentBus.Broadcast.GetOutposts()
		if not outpostCapitals or #outpostCapitals == 0 then
			return
		end
		for i = 1, #outpostCapitals do
			local outpostData = outpostCapitals[i]
			if additionalData.outpostId == outpostData.id then
				bannerData.TerritoryEnteredCard1.title = outpostData.nameLocalizationKey
				bannerData.TerritoryEnteredCard1.titleLabel = "@ui_outpost"
				self.banners:EnqueueBanner(
					layouts.LAYOUT_TERRITORY_ENTERED,
					bannerData,
					duration,
					nil,
					nil,
					false,
					priority
				)
			end
		end
		return
	end
	local retrieveGuildData = false
	local territoryName = TerritoryDataHandler:GetTerritoryNameFromTerritoryId(claimKey)
	if territoryEnteredCardType == TerritoryEnteredCardTypes.TerritoryType then
		if not territoryName or territoryName == "" then
			return
		end
		retrieveGuildData = true
		local territoryStanding = TerritoryDataHandler:GetTerritoryStanding(claimKey)
		bannerData.TerritoryEnteredCard1.title = territoryName
		local v13_5 = bannerData.TerritoryEnteredCard1
		v13_5.titleLabel = isClaimable and "@ui_territory" or "@ui_region"
		local v13_6 = bannerData.TerritoryEnteredCard1
		v13_6.standingLabel =
			GetLocalizedReplacementText("@ui_territory_standinglabel", { territoryName = territoryName })
		bannerData.TerritoryEnteredCard1.rank = tostring(territoryStanding.rank)
		bannerData.TerritoryEnteredCard1.rankName = territoryStanding.displayName
		bannerData.TerritoryEnteredCard1.description = "@ui_unclaimed_territory"
	elseif
		territoryEnteredCardType == TerritoryEnteredCardTypes.SettlementType
		or territoryEnteredCardType == TerritoryEnteredCardTypes.FortType
	then
		if not territoryName or territoryName == "" then
			return
		end
		retrieveGuildData = true
		local isSettlementData = territoryEnteredCardType == TerritoryEnteredCardTypes.SettlementType or false
		local upgradeType = isSettlementData and eTerritoryUpgradeType_Settlement or eTerritoryUpgradeType_Fortress
		local tierInfo, numTier = TerritoryDataHandler:GetUpgradeTierInfoByTerritoryId(claimKey, upgradeType)
		local locTag = isSettlementData and "@ui_territory_name_with_settlement_tier_name"
			or "@ui_territory_name_with_fort_tier_name"
		local unclaimedText =
			GetLocalizedReplacementText("@ui_unclaimed_settlementorfort", { tierName = tierInfo.name })
		local territoryNameOverride = territoryName
		if territoryEnteredCardType == TerritoryEnteredCardTypes.SettlementType then
			territoryNameOverride = TerritoryDataHandler:GetSettlementNameForTerritoryId(claimKey)
		end
		local territoryNameWithTierName =
			GetLocalizedReplacementText(locTag, { territoryName = territoryNameOverride, tierName = tierInfo.name })
		bannerData.TerritoryEnteredCard1.title = territoryNameWithTierName
		bannerData.TerritoryEnteredCard1.tierLabel = GetRomanFromNumber(numTier)
		bannerData.TerritoryEnteredCard1.description = unclaimedText
	elseif territoryEnteredCardType == TerritoryEnteredCardTypes.HQType then
		retrieveGuildData = true
		bannerData.TerritoryEnteredCard1.title = additionalData.name
		bannerData.TerritoryEnteredCard1.description = additionalData.description
		bannerData.TerritoryEnteredCard1.eventId = additionalData.eventId
		bannerData.TerritoryEnteredCard1.showBg = false
	elseif territoryEnteredCardType == TerritoryEnteredCardTypes.OpenWorld then
		bannerData.TerritoryEnteredCard1.title = additionalData.name
		bannerData.TerritoryEnteredCard1.description = additionalData.description
		bannerData.TerritoryEnteredCard1.eventId = additionalData.eventId
		bannerData.TerritoryEnteredCard1.showBg = false
		bannerData.TerritoryEnteredCard1.mutatedDungeon = additionalData.mutatedDungeon
	end
	local ownerData = isClaimable and LandClaimRequestBus.Broadcast.GetClaimOwnerData(claimKey) or nil
	if retrieveGuildData and ownerData and ownerData.guildId:IsValid() then
		self.socialDataHandler:GetGuildDetailedData_ServerCall(self, function(self, result)
			if #result <= 0 then
				Log("ERR - BannerTriggers:WarBanner: GuildData request returned with no data")
				return
			end
			local guildData = type(result[1]) == "table" and result[1].guildData or result[1]
			if guildData and guildData:IsValid() then
				bannerData.TerritoryEnteredCard1.guildName = guildData.guildName
				bannerData.TerritoryEnteredCard1.crestData = guildData.crestData
				self.banners:EnqueueBanner(
					layouts.LAYOUT_TERRITORY_ENTERED,
					bannerData,
					duration,
					nil,
					nil,
					false,
					priority
				)
			end
		end, self.GetGuildDetailedDataFailure, ownerData.guildId)
	else
		self.banners:EnqueueBanner(layouts.LAYOUT_TERRITORY_ENTERED, bannerData, duration, nil, nil, false, priority)
	end
end
function BannerTriggers:ShowArenaActivatedNotification(secondsTillTeleport)
	local notificationData = NotificationData()
	notificationData.title = "@arena_teleport_title"
	notificationData.text = "@arena_teleport_desc"
	notificationData.maximumDuration = secondsTillTeleport - 1
	notificationData.showProgress = true
	UiNotificationsBus.Broadcast.EnqueueNotification(notificationData)
	local bannerData = {}
	bannerData.AchievementCard1 = { title = "@arena_started" }
	local priority = 4
	self.banners:EnqueueBanner(
		layouts.LAYOUT_ACHIEVEMENT,
		bannerData,
		layouts.DEFAULT_DISPLAY_DURATION,
		nil,
		nil,
		false,
		priority
	)
	self.audioHelper:SwitchMusicDB(self.audioHelper.MusicSwitch_Arena, self.audioHelper.MusicState_Arena_Countdown)
end
function BannerTriggers:ShowMinimalTextBanner(titleText, descriptionText, titleLabelText, iconPath)
	local bannerData = {}
	bannerData.TerritoryEnteredCard1 = {
		title = titleText,
		description = descriptionText,
		titleLabel = titleLabelText,
		icon = iconPath,
		isClaimable = true,
	}
	return self.banners:EnqueueBanner(layouts.LAYOUT_TERRITORY_ENTERED, bannerData, 5, nil, nil, false, 4)
end
function BannerTriggers:OnCutsceneStarted(cutSceneInfo)
	self.isInCutscene = true
	self.banners:RemoveAllPendingBanners()
	if cutSceneInfo.bannerTitleText == nil or cutSceneInfo.bannerTitleText == "" then
		return
	end
	self:ShowMinimalTextBanner(
		cutSceneInfo.bannerTitleText,
		cutSceneInfo.bannerDescriptionText,
		cutSceneInfo.bannerTitleLabelText,
		"LyShineUI/Images/Icons/Misc/icon_warUncolored.dds"
	)
end
function BannerTriggers:OnCutsceneEnded(cutSceneInfo)
	self.isInCutscene = false
end
function BannerTriggers:ShouldShowTerritoryNotifications()
	return DynamicBus.NotificationsRequestBus.Broadcast.ShouldShowTerritoryNotifications()
end
function BannerTriggers:ShouldShowWarNotifications()
	return DynamicBus.NotificationsRequestBus.Broadcast.ShouldShowWarNotifications() and not self.isInCutscene
end
function BannerTriggers:ShowInfluenceRaceBanner(territoryIds, title, label)
	local bannerData = {}
	bannerData.TextCard1 = {
		title = title,
		icon = "lyshineui/images/icons/leaderboards/category_spreadsheet/factionwars_influence.dds",
		iconScale = 1.7,
		offset = 70,
		bgOffset = -40,
		showBg = true,
		showLine = true,
		keybindValue = "toggleMapComponent",
		hintDescription = "@influence_race_banner_viewmap",
	}
	if #territoryIds > 1 then
		local territoryNameList = {}
		local listCount = #territoryIds <= 6 and #territoryIds or 6
		for i = 1, listCount do
			local v11 = "item" .. tostring(i)
			territoryNameList[v11] = TerritoryDataHandler:GetTerritoryNameFromTerritoryId(territoryIds[i])
		end
		local territoryNameListString = GetLocalizedReplacementText
		local v8_2 = "@generic_list_" .. tostring(listCount)
		local territoryNameListString = territoryNameListString(v8_2, territoryNameList)
		local v8_3 = bannerData.TextCard1
		v8_3.titleLabel = GetLocalizedReplacementText(label, { territoryNameList = territoryNameListString })
	else
		local claimKey = territoryIds[1]
		local territoryName = TerritoryDataHandler:GetTerritoryNameFromTerritoryId(claimKey)
		local influenceRace = WarDataServiceBus.Broadcast.GetRaceForTerritory(claimKey)
		local startTime = influenceRace.startTime:Subtract(WallClockTimePoint()):ToSecondsRoundedUp()
		local endTime = ConfigProviderEventBus.Broadcast.GetUInt(
			"javelin.faction-influence-v2-race-scheduling-max-race-length-minutes"
		)
		local endTime = startTime + endTime * timeHelpers.secondsInMinute
		local v10_6 = bannerData.TextCard1
		local v11_3 = GetLocalizedReplacementText
		local v13_3 = {}
		v13_3.territoryName = territoryName
		v13_3.date = timeHelpers:GetLocalizedLongDate(startTime)
		v13_3.startTime = timeHelpers:GetLocalizedServerTime(startTime, false)
		v13_3.endTime = timeHelpers:GetLocalizedServerTime(endTime, true)
		v10_6.titleLabel = v11_3(label, v13_3)
	end
	if bannerData then
		local bannerDisplayTime = 5
		local priority = 3
		DynamicBus.Banner.Broadcast.EnqueueBanner(
			layouts.LAYOUT_TEXT_CARD,
			bannerData,
			bannerDisplayTime,
			nil,
			nil,
			false,
			priority,
			self.BANNER_DRAW_ORDER_TOP
		)
	end
end
function BannerTriggers:OnInfluenceRaceScheduled(territoryIds)
	if FtueSystemRequestBus.Broadcast.IsFtue() then
		return
	end
	if #territoryIds > 0 then
		self:ShowInfluenceRaceBanner(
			territoryIds,
			"@influence_race_scheduled_title",
			#territoryIds > 1 and "@influence_race_scheduled_multiple" or "@influence_race_schedule_info"
		)
	end
end
function BannerTriggers:OnInfluenceRaceStartingSoon(territoryIds)
	if FtueSystemRequestBus.Broadcast.IsFtue() then
		return
	end
	if #territoryIds > 0 then
		self:ShowInfluenceRaceBanner(
			territoryIds,
			"@influence_race_upcoming_title",
			#territoryIds > 1 and "@influence_race_upcoming_multiple" or "@influence_race_upcoming_info"
		)
	end
end
function BannerTriggers:OnInfluenceRaceStartingNow(territoryIds)
	if FtueSystemRequestBus.Broadcast.IsFtue() then
		return
	end
	if #territoryIds > 0 then
		self:ShowInfluenceRaceBanner(
			territoryIds,
			"@influence_race_starting_title",
			#territoryIds > 1 and "@influence_race_starting_multiple" or "@influence_race_starting_info"
		)
	end
end
function BannerTriggers:OnInfluenceRaceEnded(territoryIds)
	if FtueSystemRequestBus.Broadcast.IsFtue() then
		return
	end
	if #territoryIds > 0 then
		self:ShowInfluenceRaceBanner(territoryIds, "@influence_race_ended_title", "@influence_race_ended_info")
	end
end
return BannerTriggers
