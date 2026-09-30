CREATE TABLE `idx_token` (
	`id` INTEGER PRIMARY KEY AUTOINCREMENT,
	`text` TEXT NOT NULL,
	`crc` INTEGER NOT NULL,
	`crc_lower` INTEGER NOT NULL
);
--> statement-breakpoint
CREATE TABLE `idx_entry` (
	`id` INTEGER PRIMARY KEY AUTOINCREMENT,
	`pak_id` INTEGER NOT NULL,
	`name_id` INTEGER NOT NULL,
	`format` INTEGER NOT NULL,
	`size` INTEGER NOT NULL,
	CONSTRAINT `fk_idx_entry_pak_id_idx_token_id_fk` FOREIGN KEY (`pak_id`) REFERENCES `idx_token`(`id`),
	CONSTRAINT `fk_idx_entry_name_id_idx_token_id_fk` FOREIGN KEY (`name_id`) REFERENCES `idx_token`(`id`)
);
--> statement-breakpoint
CREATE TABLE `idx_fact` (
	`id` INTEGER PRIMARY KEY AUTOINCREMENT,
	`entry_id` INTEGER NOT NULL,
	`field_id` INTEGER NOT NULL,
	`value_id` INTEGER NOT NULL,
	`kind` INTEGER NOT NULL,
	`loc_a` INTEGER NOT NULL,
	`loc_b` INTEGER NOT NULL,
	CONSTRAINT `fk_idx_fact_entry_id_idx_entry_id_fk` FOREIGN KEY (`entry_id`) REFERENCES `idx_entry`(`id`),
	CONSTRAINT `fk_idx_fact_field_id_idx_token_id_fk` FOREIGN KEY (`field_id`) REFERENCES `idx_token`(`id`),
	CONSTRAINT `fk_idx_fact_value_id_idx_token_id_fk` FOREIGN KEY (`value_id`) REFERENCES `idx_token`(`id`)
);
--> statement-breakpoint
CREATE TABLE `idx_pak` (
	`id` INTEGER PRIMARY KEY AUTOINCREMENT,
	`name_id` INTEGER NOT NULL,
	`size` INTEGER NOT NULL,
	`mtime` INTEGER NOT NULL,
	`complete` INTEGER NOT NULL,
	CONSTRAINT `fk_idx_pak_name_id_idx_token_id_fk` FOREIGN KEY (`name_id`) REFERENCES `idx_token`(`id`)
);
--> statement-breakpoint
CREATE INDEX `token_crc_idx` ON `idx_token`(`crc`);
--> statement-breakpoint
CREATE INDEX `token_crc_lower_idx` ON `idx_token`(`crc_lower`);
--> statement-breakpoint
CREATE INDEX `fact_value_idx` ON `idx_fact`(`value_id`);
--> statement-breakpoint
CREATE INDEX `fact_field_idx` ON `idx_fact`(`field_id`);