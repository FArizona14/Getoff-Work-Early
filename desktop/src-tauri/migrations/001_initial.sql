CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY, applied_at INTEGER NOT NULL);
CREATE TABLE workdays(day TEXT PRIMARY KEY, started_at INTEGER NOT NULL, ended_at INTEGER, CHECK(ended_at IS NULL OR ended_at >= started_at));
CREATE TABLE captures(id INTEGER PRIMARY KEY, title BLOB NOT NULL, created_at INTEGER NOT NULL);
CREATE TABLE settings(key TEXT PRIMARY KEY, value TEXT NOT NULL);
INSERT INTO schema_migrations VALUES(1, unixepoch());
