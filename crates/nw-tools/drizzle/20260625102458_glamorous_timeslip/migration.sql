CREATE TABLE `catalog` (
	`asset_id` TEXT PRIMARY KEY NOT NULL,
	`path` TEXT NOT NULL,
	`asset_type` TEXT NOT NULL,
	`size` INTEGER NOT NULL
);
