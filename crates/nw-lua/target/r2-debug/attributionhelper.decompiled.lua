local AttributionHelper = {}
local attributionIdToAttributionData = {}
attributionIdToAttributionData[2455295778] = {
	displayName = "@attribution_Housing_SettlerSet_2021",
	imagePathIcon = "LyShineUI\\Images\\Icons\\Misc\\icon_house.png",
}
attributionIdToAttributionData[2965763911] = {
	displayName = "@attribution_Housing_PirateSet_2021",
	imagePathIcon = "LyShineUI\\Images\\Icons\\Misc\\icon_house.png",
}
attributionIdToAttributionData[1823011872] = {
	displayName = "@attribution_Housing_DynastySet_2021",
	imagePathIcon = "LyShineUI\\Images\\Icons\\Misc\\icon_house.png",
}
attributionIdToAttributionData[2354195756] = {
	displayName = "@attribution_Housing_LegionSet_2022",
	imagePathIcon = "LyShineUI\\Images\\Icons\\Misc\\icon_house.png",
}
attributionIdToAttributionData[3700308041] = {
	displayName = "@attribution_WinterConvergence_2021",
	imagePathIcon = "LyShineUI\\Images\\Icons\\WorldMap\\worldmap_event_npc.png",
}
attributionIdToAttributionData[1166502387] = {
	displayName = "@attribution_WinterConvergence_2022",
	imagePathIcon = "LyShineUI\\Images\\Icons\\WorldMap\\worldmap_event_npc.png",
}
attributionIdToAttributionData[1189133312] = {
	displayName = "@attribution_SummerMedleyfaire_2022",
	imagePathIcon = "LyShineUI\\Images\\Icons\\WorldMap\\worldmap_summerevent_npc.png",
}
attributionIdToAttributionData[837258390] = {
	displayName = "@attribution_SummerMedleyfaire_2023",
	imagePathIcon = "LyShineUI\\Images\\Icons\\WorldMap\\worldmap_summerevent_npc.png",
}
attributionIdToAttributionData[4279388762] = {
	displayName = "@attribution_NightveilHallow_2022",
	imagePathIcon = "LyShineUI\\Images\\Icons\\WorldMap\\worldmap_nightveil_centerpiece.png",
}
attributionIdToAttributionData[2283109068] = {
	displayName = "@attribution_NightveilHallow_2023",
	imagePathIcon = "LyShineUI\\Images\\Icons\\WorldMap\\worldmap_nightveil_centerpiece.png",
}
attributionIdToAttributionData[801173683] = {
	displayName = "@attribution_TurkeyTerror_2022",
	imagePathIcon = "LyShineUI\\Images\\Icons\\WorldMap\\icon_event_turkey.png",
}
attributionIdToAttributionData[1489485861] = {
	displayName = "@attribution_TurkeyTerror_2023",
	imagePathIcon = "LyShineUI\\Images\\Icons\\WorldMap\\icon_event_turkey.png",
}
attributionIdToAttributionData[2925014118] = {
	displayName = "@attribution_Season_1",
	imagePathIcon = "LyShineUI\\Images\\Icons\\WorldMap\\icon_event_seasons.png",
}
attributionIdToAttributionData[928087516] = {
	displayName = "@attribution_Season_2",
	imagePathIcon = "LyShineUI\\Images\\Icons\\WorldMap\\icon_event_seasons.png",
}
attributionIdToAttributionData[1079397706] = {
	displayName = "@attribution_Season_3",
	imagePathIcon = "LyShineUI\\Images\\Icons\\WorldMap\\icon_event_seasons.png",
}
attributionIdToAttributionData[3727874281] = {
	displayName = "@attribution_Season_4",
	imagePathIcon = "LyShineUI\\Images\\Icons\\WorldMap\\icon_event_seasons.png",
}
attributionIdToAttributionData[1084308676] = {
	displayName = "@attribution_SpringtideBloom_2023",
	imagePathIcon = "LyShineUI\\Images\\Icons\\WorldMap\\worldmap_springtide.png",
}
attributionIdToAttributionData[655127655] = {
	displayName = "@attribution_RabbitSeason_2023",
	imagePathIcon = "LyShineUI\\Images\\Icons\\WorldMap\\icon_event_rabbit.png",
}
function AttributionHelper:GetAttributionData(attributionId)
	return attributionIdToAttributionData[attributionId] or { displayName = "", imagePathIcon = "" }
end
return AttributionHelper
