-- Your SQL goes here
CREATE TABLE "posts"(
	"id" SERIAL NOT NULL PRIMARY KEY,
	"title" VARCHAR NOT NULL,
	"body" TEXT NOT NULL,
	"published" BOOL NOT NULL DEFAULT FALSE
);

