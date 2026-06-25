CREATE TABLE `guid` (
	`guid` TEXT PRIMARY KEY NOT NULL,
	`path` TEXT NOT NULL
);

--> statement-breakpoint
CREATE TABLE `meta` (
	`key` TEXT PRIMARY KEY NOT NULL,
	`value` TEXT NOT NULL
);
