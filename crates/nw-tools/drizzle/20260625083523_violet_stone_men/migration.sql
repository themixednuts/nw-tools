CREATE TABLE `toc` (
	`path` TEXT PRIMARY KEY NOT NULL,
	`pak` TEXT NOT NULL,
	`entry` INTEGER NOT NULL
);

--> statement-breakpoint
CREATE TABLE `guid` (
	`guid` TEXT PRIMARY KEY NOT NULL,
	`path` TEXT NOT NULL
);
