use super::indexed::{
    number_expression, optional_field, required_field, required_row, string_expression,
};
use super::*;

pub(super) fn augmentation(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
    shape: &NativeManagerShape,
) -> GoNativeManagerAugmentation {
    match shape {
        NativeManagerShape::OneTableCampSkin(_) => camp_skin(unit, manager),
        NativeManagerShape::OneTableStoreCategory(_) => store_category(unit, manager),
        NativeManagerShape::OneTableStoreProduct(_) => store_product(unit, manager),
        NativeManagerShape::OneTableRewardTrackItem(_) => reward_track_item(unit, manager),
        _ => panic!(
            "manager {} reached commerce Go native dispatch with {shape:?}",
            manager.manager_name
        ),
    }
}

fn camp_skin(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let row = required_row(unit, manager, "CampSkinData");
    let row_field = go_direct_row_field_name("CampSkinData");
    let id = required_field(&row, "CampSkinID");
    let item = required_field(&row, "ItemID");
    let achievement = required_field(&row, "RequiredAchievementID");
    let entitlement = required_field(&row, "IsEntitlement");
    let enabled = required_field(&row, "IsEnabled");
    let id_expr = string_expression(id, "source.Row");
    let item_expr = string_expression(item, "source.Row");
    let achievement_expr = string_expression(achievement, "source.Row");
    let entitlement_expr = bool_expression(entitlement, "source.Row");
    let enabled_expr = bool_expression(enabled, "source.Row");
    let manager_type = go_method_name(&manager.manager_class_name);

    GoNativeManagerAugmentation {
        declarations: r#"
type CampSkinSettings struct { EnableCampSkins bool; EnableAllCampSkins bool }

type CampSkinData struct {
	ID string
	IDCRC gametypes.CRC32
	ItemID string
	RequiredAchievementID string
	IsEntitlement bool
	IsEnabled bool
}
"#
        .to_owned(),
        fields: "\tcampSkins []CampSkinData\n\tcampSkinsByID map[gametypes.CRC32]int\n".to_owned(),
        field_values: "\t\tcampSkinsByID: make(map[gametypes.CRC32]int),\n".to_owned(),
        initializers: format!(
            r#"	for index := range manager.{row_field}.entries {{
		source := &manager.{row_field}.entries[index]
		if !({enabled_expr}) {{ continue }}
		key := strings.TrimSpace({id_expr})
		itemID := strings.TrimSpace({item_expr})
		id := gametypes.CRC32(crc32Lowercase(key))
		if key == "" || itemID == "" || id == 0 || gametypes.CRC32(crc32Lowercase(itemID)) == 0 {{ continue }}
		if _, exists := manager.campSkinsByID[id]; exists {{ continue }}
		manager.campSkinsByID[id] = len(manager.campSkins)
		manager.campSkins = append(manager.campSkins, CampSkinData{{ID: key, IDCRC: id, ItemID: itemID, RequiredAchievementID: strings.TrimSpace({achievement_expr}), IsEntitlement: {entitlement_expr}, IsEnabled: {enabled_expr}}})
	}}
"#
        ),
        methods: format!(
            r#"func (manager *{manager_type}) CampSkinDataFromID(id gametypes.CRC32) *CampSkinData {{ index, ok := manager.campSkinsByID[id]; if !ok {{ return nil }}; return rowCopy(manager.campSkins[index]) }}
func (manager *{manager_type}) CampSkinData(key string) *CampSkinData {{ data := manager.CampSkinDataFromID(gametypes.CRC32(crc32Lowercase(key))); if data == nil || !strings.EqualFold(data.ID, strings.TrimSpace(key)) {{ return nil }}; return data }}
func (manager *{manager_type}) CampSkinDataByKey(key string) *CampSkinData {{ return manager.CampSkinData(key) }}
func (manager *{manager_type}) CampSkinIDs() iter.Seq[string] {{ return func(yield func(string) bool) {{ for index := range manager.campSkins {{ if !yield(manager.campSkins[index].ID) {{ return }} }} }} }}
func (manager *{manager_type}) CampSkins() iter.Seq[CampSkinData] {{ return rowValues(manager.campSkins) }}
func (manager *{manager_type}) Rows() iter.Seq[CampSkinData] {{ return manager.CampSkins() }}

"#
        ),
    }
}

fn store_category(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let row = required_row(unit, manager, "StoreCategoryProperties");
    let row_field = go_direct_row_field_name("StoreCategoryProperties");
    let manager_type = go_method_name(&manager.manager_class_name);
    let id = string_expression(required_field(&row, "StoreCategory"), "source.Row");
    let order = number_expression(required_field(&row, "CategoryOrder"), "source.Row");
    let text = string_expression(required_field(&row, "CategoryText"), "source.Row");
    let portrait = string_expression(required_field(&row, "PortraitImage"), "source.Row");
    let landscape = string_expression(required_field(&row, "LandscapeImage"), "source.Row");
    let square = string_expression(required_field(&row, "SquareImage"), "source.Row");
    let children = optional_field(&row, "ChildCategoryList")
        .map(|field| string_expression(field, "source.Row"))
        .unwrap_or_else(|| "\"\"".to_owned());
    let products = optional_field(&row, "StoreProductTypeList")
        .map(|field| string_expression(field, "source.Row"))
        .unwrap_or_else(|| "\"\"".to_owned());

    GoNativeManagerAugmentation {
        declarations: r#"
type GameStoreTab string

const (
	GameStoreTabFeaturedDeals GameStoreTab = "FeaturedDeals"
	GameStoreTabConsumables GameStoreTab = "Consumables"
	GameStoreTabEmotes GameStoreTab = "Emotes"
	GameStoreTabArmorSkins GameStoreTab = "ArmorSkins"
	GameStoreTabWeaponSkins GameStoreTab = "WeaponSkins"
	GameStoreTabHousingItems GameStoreTab = "HousingItems"
	GameStoreTabToolSkins GameStoreTab = "ToolSkins"
	GameStoreTabCampSkins GameStoreTab = "CampSkins"
	GameStoreTabGuildCrestsAndColors GameStoreTab = "GuildCrestsAndColors"
	GameStoreTabBundles GameStoreTab = "Bundles"
	GameStoreTabServices GameStoreTab = "Services"
	GameStoreTabLoadouts GameStoreTab = "Loadouts"
	GameStoreTabMounts GameStoreTab = "Mounts"
	GameStoreTabMountAttachments GameStoreTab = "MountAttachments"
	GameStoreTabMountTrailVFX GameStoreTab = "MountTrailVFX"
	GameStoreTabMountSummonVFX GameStoreTab = "MountSummonVFX"
	GameStoreTabEffects GameStoreTab = "Effects"
	GameStoreTabMountEffects GameStoreTab = "MountEffects"
	GameStoreTabTimedEvent GameStoreTab = "TimedEvent"
	GameStoreTabExpansion2023 GameStoreTab = "Expansion2023"
	GameStoreTabDyes GameStoreTab = "Dyes"
	GameStoreTabBoosters GameStoreTab = "Boosters"
	GameStoreTabPermits GameStoreTab = "Permits"
	GameStoreTabSpecial1 GameStoreTab = "Special1"
	GameStoreTabSpecial2 GameStoreTab = "Special2"
	GameStoreTabSearchResults GameStoreTab = "SearchResults"
	GameStoreTabSkinsAndEffects GameStoreTab = "SkinsAndEffects"
	GameStoreTabFurniture GameStoreTab = "Furniture"
	GameStoreTabHousePets GameStoreTab = "HousePets"
	GameStoreTabTokens GameStoreTab = "Tokens"
	GameStoreTabAugments GameStoreTab = "Augments"
	GameStoreTabMountDyes GameStoreTab = "MountDyes"
)

func parseGameStoreTab(value string) (GameStoreTab, bool) {
	switch GameStoreTab(strings.TrimSpace(value)) {
	case GameStoreTabFeaturedDeals, GameStoreTabConsumables, GameStoreTabEmotes, GameStoreTabArmorSkins, GameStoreTabWeaponSkins, GameStoreTabHousingItems, GameStoreTabToolSkins, GameStoreTabCampSkins, GameStoreTabGuildCrestsAndColors, GameStoreTabBundles, GameStoreTabServices, GameStoreTabLoadouts, GameStoreTabMounts, GameStoreTabMountAttachments, GameStoreTabMountTrailVFX, GameStoreTabMountSummonVFX, GameStoreTabEffects, GameStoreTabMountEffects, GameStoreTabTimedEvent, GameStoreTabExpansion2023, GameStoreTabDyes, GameStoreTabBoosters, GameStoreTabPermits, GameStoreTabSpecial1, GameStoreTabSpecial2, GameStoreTabSearchResults, GameStoreTabSkinsAndEffects, GameStoreTabFurniture, GameStoreTabHousePets, GameStoreTabTokens, GameStoreTabAugments, GameStoreTabMountDyes:
		return GameStoreTab(strings.TrimSpace(value)), true
	default:
		return "", false
	}
}

type StoreCategoryProperties struct {
	CategoryID gametypes.CRC32
	CategoryName string
	Tab GameStoreTab
	CategoryOrder uint32
	CategoryText string
	PortraitImage string
	LandscapeImage string
	SquareImage string
	ChildCategories []gametypes.CRC32
	ProductTypes []gametypes.StoreProductType
}

type InvalidStoreCategoryProductType struct { CategoryID gametypes.CRC32; CategoryName string; ProductType string }
"#
        .to_owned(),
        fields: "\tstoreCategories []StoreCategoryProperties\n\tstoreCategoriesByID map[gametypes.CRC32]int\n\tstoreCategoryByOrder map[uint32]gametypes.CRC32\n\tstoreTabByProductType map[gametypes.StoreProductType]GameStoreTab\n\tinvalidStoreCategoryProductTypes []InvalidStoreCategoryProductType\n".to_owned(),
        field_values: "\t\tstoreCategoriesByID: make(map[gametypes.CRC32]int),\n\t\tstoreCategoryByOrder: make(map[uint32]gametypes.CRC32),\n\t\tstoreTabByProductType: make(map[gametypes.StoreProductType]GameStoreTab),\n".to_owned(),
        initializers: format!(
            r#"	for index := range manager.{row_field}.entries {{
		source := &manager.{row_field}.entries[index]
		name := strings.TrimSpace({id})
		id := gametypes.CRC32(crc32Lowercase(name))
		if name == "" || id == 0 {{ continue }}
		if _, exists := manager.storeCategoriesByID[id]; exists {{ continue }}
		tab, validTab := parseGameStoreTab(name)
		if !validTab {{ continue }}
		categoryOrder, validOrder := exactUint32({order})
		if !validOrder {{ continue }}
		data := StoreCategoryProperties{{CategoryID: id, CategoryName: name, Tab: tab, CategoryOrder: categoryOrder, CategoryText: strings.TrimSpace({text}), PortraitImage: strings.TrimSpace({portrait}), LandscapeImage: strings.TrimSpace({landscape}), SquareImage: strings.TrimSpace({square})}}
		for _, child := range splitDesignerList({children}) {{ childID := gametypes.CRC32(crc32Lowercase(child)); if childID != 0 {{ data.ChildCategories = append(data.ChildCategories, childID) }} }}
		for _, productText := range splitDesignerList({products}) {{
			productType, err := parseStoreProductType(productText)
			if err != nil {{ manager.invalidStoreCategoryProductTypes = append(manager.invalidStoreCategoryProductTypes, InvalidStoreCategoryProductType{{CategoryID: id, CategoryName: name, ProductType: productText}}); continue }}
			data.ProductTypes = append(data.ProductTypes, productType)
			manager.storeTabByProductType[productType] = tab
		}}
		manager.storeCategoriesByID[id] = len(manager.storeCategories)
		if categoryOrder > 0 {{ manager.storeCategoryByOrder[categoryOrder] = id }}
		manager.storeCategories = append(manager.storeCategories, data)
	}}
"#
        ),
        methods: format!(
            r#"func (manager *{manager_type}) StoreCategoryPropertiesFromID(id gametypes.CRC32) *StoreCategoryProperties {{ index, ok := manager.storeCategoriesByID[id]; if !ok {{ return nil }}; return rowCopy(manager.storeCategories[index]) }}
func (manager *{manager_type}) StoreCategoryProperties(name string) *StoreCategoryProperties {{ return manager.StoreCategoryPropertiesFromID(gametypes.CRC32(crc32Lowercase(name))) }}
func (manager *{manager_type}) StoreCategoryPropertiesByName(name string) *StoreCategoryProperties {{ return manager.StoreCategoryProperties(name) }}
func (manager *{manager_type}) StoreCategoryPropertiesByIndex(order uint32) *StoreCategoryProperties {{ id, ok := manager.storeCategoryByOrder[order]; if !ok {{ return nil }}; return manager.StoreCategoryPropertiesFromID(id) }}
func (manager *{manager_type}) StoreTabForProductType(productType gametypes.StoreProductType) (GameStoreTab, bool) {{ tab, ok := manager.storeTabByProductType[productType]; return tab, ok }}
func (manager *{manager_type}) InvalidProductTypes() iter.Seq[InvalidStoreCategoryProductType] {{ return slices.Values(manager.invalidStoreCategoryProductTypes) }}
func (manager *{manager_type}) Categories() iter.Seq[StoreCategoryProperties] {{ return rowValues(manager.storeCategories) }}
func (manager *{manager_type}) Rows() iter.Seq[StoreCategoryProperties] {{ return manager.Categories() }}
func (manager *{manager_type}) NumCategories() int {{ return len(manager.storeCategoryByOrder) }}

"#
        ),
    }
}

fn store_product(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let row = required_row(unit, manager, "StoreProductData");
    let row_field = go_direct_row_field_name("StoreProductData");
    let manager_type = go_method_name(&manager.manager_class_name);
    let id = string_expression(required_field(&row, "UniqueTagID"), "source.Row");
    let enabled = bool_expression(required_field(&row, "IsEnabled"), "source.Row");
    let string = |column: &str| string_expression(required_field(&row, column), "source.Row");
    let display = string("DisplayName");
    let description = string("Description");
    let portrait = string("PortraitImage");
    let landscape = string("LandscapeImage");
    let square = string("SquareImage");
    let thumbnail = string("ThumbnailImage");
    let type_description = string("TypeDescription");
    let product_type = string("StoreProductType");

    GoNativeManagerAugmentation {
        declarations: r#"
type StoreProductData struct {
	ProductID gametypes.CRC32
	UniqueTagID string
	IsEnabled bool
	DisplayName string
	Description string
	PortraitImage string
	LandscapeImage string
	SquareImage string
	ThumbnailImage string
	TypeDescription string
	ProductType *gametypes.StoreProductType
	ProductTypeText string
}
type InvalidStoreProductType struct { ProductID gametypes.CRC32; UniqueTagID string; ProductType string }
"#
        .to_owned(),
        fields: "\tstoreProducts []StoreProductData\n\tstoreProductsByID map[gametypes.CRC32]int\n\tinvalidStoreProductTypes []InvalidStoreProductType\n".to_owned(),
        field_values: "\t\tstoreProductsByID: make(map[gametypes.CRC32]int),\n".to_owned(),
        initializers: format!(
            r#"	for index := range manager.{row_field}.entries {{
		source := &manager.{row_field}.entries[index]
		key := strings.TrimSpace({id})
		id := gametypes.CRC32(crc32Lowercase(key))
		if key == "" || id == 0 {{ continue }}
		productTypeText := strings.TrimSpace({product_type})
		var productType *gametypes.StoreProductType
		if typed, err := parseStoreProductType(productTypeText); err == nil {{ productType = &typed }} else {{ manager.invalidStoreProductTypes = append(manager.invalidStoreProductTypes, InvalidStoreProductType{{ProductID: id, UniqueTagID: key, ProductType: productTypeText}}) }}
		data := StoreProductData{{ProductID: id, UniqueTagID: key, IsEnabled: {enabled}, DisplayName: strings.TrimSpace({display}), Description: strings.TrimSpace({description}), PortraitImage: strings.TrimSpace({portrait}), LandscapeImage: strings.TrimSpace({landscape}), SquareImage: strings.TrimSpace({square}), ThumbnailImage: strings.TrimSpace({thumbnail}), TypeDescription: strings.TrimSpace({type_description}), ProductType: productType, ProductTypeText: productTypeText}}
		if previous, exists := manager.storeProductsByID[id]; exists {{ manager.storeProducts[previous] = data; continue }}
		manager.storeProductsByID[id] = len(manager.storeProducts)
		manager.storeProducts = append(manager.storeProducts, data)
	}}
"#
        ),
        methods: format!(
            r#"func (manager *{manager_type}) StoreProductDataFromID(id gametypes.CRC32) *StoreProductData {{ index, ok := manager.storeProductsByID[id]; if !ok {{ return nil }}; return rowCopy(manager.storeProducts[index]) }}
func (manager *{manager_type}) StoreProductDataByTag(tag string) *StoreProductData {{ return manager.StoreProductDataFromID(gametypes.CRC32(crc32Lowercase(tag))) }}
func (manager *{manager_type}) StoreProductData(tag string) *StoreProductData {{ return manager.StoreProductDataByTag(tag) }}
func (manager *{manager_type}) Products() iter.Seq[StoreProductData] {{ return rowValues(manager.storeProducts) }}
func (manager *{manager_type}) Rows() iter.Seq[StoreProductData] {{ return manager.Products() }}
func (manager *{manager_type}) InvalidProductTypes() iter.Seq[InvalidStoreProductType] {{ return slices.Values(manager.invalidStoreProductTypes) }}

"#
        ),
    }
}

fn reward_track_item(
    unit: &GameDataCompileUnit,
    manager: &DirectManagerSurface,
) -> GoNativeManagerAugmentation {
    let row = required_row(unit, manager, "RewardTrackItemData");
    let row_field = go_direct_row_field_name("RewardTrackItemData");
    let manager_type = go_method_name(&manager.manager_class_name);
    let string = |column: &str| string_expression(required_field(&row, column), "source.Row");
    let reward = string("RewardID");
    let entitlement = string("Entitlement");
    let event = string("GameEvent");
    let item = string("Item");
    let name = string("Name");
    let description = string("Description");
    let progression = string("CategoricalProgressionId");
    let icon = string("IconPath");
    let hi_res_icon = string("HiResIconPath");
    let quantity = number_expression(required_field(&row, "Quantity"), "source.Row");
    let cost = number_expression(
        required_field(&row, "BuyCategoricalProgressionCost"),
        "source.Row",
    );
    let roll = bool_expression(required_field(&row, "RollOnPresent"), "source.Row");
    let use_level = optional_field(&row, "UseLevelGS")
        .map(|field| optional_bool_pointer_expression(field, "source.Row"))
        .unwrap_or_else(|| "nil".to_owned());

    GoNativeManagerAugmentation {
        declarations: r#"
type RewardTrackItemPayloadKind uint8
const (
	RewardTrackItemPayloadItem RewardTrackItemPayloadKind = iota + 1
	RewardTrackItemPayloadGameEvent
	RewardTrackItemPayloadEntitlement
)
type RewardTrackItemPayload struct { Kind RewardTrackItemPayloadKind; Value string }
type RewardTrackItemData struct {
	RewardID string
	RewardIDCRC gametypes.CRC32
	Payload RewardTrackItemPayload
	Entitlement string
	GameEvent string
	Item string
	Name string
	Description string
	Quantity uint32
	RollOnPresent bool
	CategoricalProgressionID string
	CategoricalProgressionIDCRC gametypes.CRC32
	BuyCategoricalProgressionCost uint32
	IconPath string
	HiResIconPath string
	UseLevelGS *bool
}
"#
        .to_owned(),
        fields: "\trewardTrackItems []RewardTrackItemData\n\trewardTrackItemsByID map[gametypes.CRC32]int\n".to_owned(),
        field_values: "\t\trewardTrackItemsByID: make(map[gametypes.CRC32]int),\n".to_owned(),
        initializers: format!(
            r#"	for index := range manager.{row_field}.entries {{
		source := &manager.{row_field}.entries[index]
		key := strings.TrimSpace({reward})
		id := gametypes.CRC32(crc32Lowercase(key))
		if key == "" || id == 0 {{ continue }}
		entitlement := strings.TrimSpace({entitlement})
		gameEvent := strings.TrimSpace({event})
		item := strings.TrimSpace({item})
		payload := RewardTrackItemPayload{{}}
		switch {{ case entitlement != "": payload = RewardTrackItemPayload{{Kind: RewardTrackItemPayloadEntitlement, Value: entitlement}}; case gameEvent != "": payload = RewardTrackItemPayload{{Kind: RewardTrackItemPayloadGameEvent, Value: gameEvent}}; case item != "": payload = RewardTrackItemPayload{{Kind: RewardTrackItemPayloadItem, Value: item}}; default: continue }}
		progression := strings.TrimSpace({progression})
		progressionID := gametypes.CRC32(crc32Lowercase(progression))
		if progression == "" || progressionID == 0 {{ continue }}
		quantity, quantityOK := exactUint32({quantity}); if !quantityOK || quantity == 0 {{ continue }}
		cost, costOK := exactUint32({cost}); if !costOK {{ continue }}
		if _, exists := manager.rewardTrackItemsByID[id]; exists {{ continue }}
		manager.rewardTrackItemsByID[id] = len(manager.rewardTrackItems)
		manager.rewardTrackItems = append(manager.rewardTrackItems, RewardTrackItemData{{RewardID: key, RewardIDCRC: id, Payload: payload, Entitlement: entitlement, GameEvent: gameEvent, Item: item, Name: strings.TrimSpace({name}), Description: strings.TrimSpace({description}), Quantity: quantity, RollOnPresent: {roll}, CategoricalProgressionID: progression, CategoricalProgressionIDCRC: progressionID, BuyCategoricalProgressionCost: cost, IconPath: strings.TrimSpace({icon}), HiResIconPath: strings.TrimSpace({hi_res_icon}), UseLevelGS: {use_level}}})
	}}
"#
        ),
        methods: format!(
            r#"func (manager *{manager_type}) RewardTrackItemFromID(id gametypes.CRC32) *RewardTrackItemData {{ index, ok := manager.rewardTrackItemsByID[id]; if !ok {{ return nil }}; return rowCopy(manager.rewardTrackItems[index]) }}
func (manager *{manager_type}) RewardTrackItem(key string) *RewardTrackItemData {{ return manager.RewardTrackItemFromID(gametypes.CRC32(crc32Lowercase(key))) }}
func (manager *{manager_type}) RewardTrackItemByKey(key string) *RewardTrackItemData {{ return manager.RewardTrackItem(key) }}
func (manager *{manager_type}) RewardTrackItems() iter.Seq[RewardTrackItemData] {{ return rowValues(manager.rewardTrackItems) }}
func (manager *{manager_type}) Rows() iter.Seq[RewardTrackItemData] {{ return manager.RewardTrackItems() }}

"#
        ),
    }
}
