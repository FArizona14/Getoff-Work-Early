CREATE TABLE reports(
  id INTEGER PRIMARY KEY,
  report_type TEXT NOT NULL CHECK(report_type IN ('daily','weekly')),
  range_start TEXT NOT NULL,
  range_end TEXT NOT NULL,
  template TEXT NOT NULL CHECK(template IN ('simple','detailed','results')),
  title BLOB NOT NULL,
  auto_body BLOB NOT NULL,
  edited_body BLOB NOT NULL,
  generated_by TEXT NOT NULL CHECK(generated_by IN ('local','ai')),
  generation_reason TEXT NOT NULL CHECK(generation_reason IN ('manual','automatic','catchup')),
  source_cutoff INTEGER NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE INDEX reports_period ON reports(report_type,range_start DESC,range_end DESC,updated_at DESC);
CREATE UNIQUE INDEX reports_one_automatic_period
  ON reports(report_type,range_start,range_end)
  WHERE generation_reason IN ('automatic','catchup');

CREATE TABLE report_versions(
  id INTEGER PRIMARY KEY,
  report_id INTEGER NOT NULL REFERENCES reports(id) ON DELETE CASCADE,
  version_kind TEXT NOT NULL CHECK(version_kind IN ('generated','edited','synced')),
  body BLOB NOT NULL,
  created_at INTEGER NOT NULL
);
CREATE INDEX report_versions_report ON report_versions(report_id,created_at DESC);

CREATE TABLE report_sources(
  id INTEGER PRIMARY KEY,
  report_id INTEGER NOT NULL REFERENCES reports(id) ON DELETE CASCADE,
  task_id INTEGER REFERENCES tasks(id) ON DELETE SET NULL,
  task_title BLOB NOT NULL,
  task_result BLOB,
  task_status TEXT NOT NULL,
  task_project BLOB,
  planned_date TEXT,
  completed_at INTEGER,
  section TEXT NOT NULL,
  added_at INTEGER NOT NULL,
  UNIQUE(report_id,task_id)
);
CREATE INDEX report_sources_report ON report_sources(report_id,id);

INSERT OR IGNORE INTO settings(key,value) VALUES('report_auto_enabled','true');
INSERT OR IGNORE INTO settings(key,value) VALUES('report_weekday','5');
INSERT OR IGNORE INTO settings(key,value) VALUES('ai_enabled','false');
INSERT OR IGNORE INTO settings(key,value) VALUES('ai_endpoint','https://api.openai.com/v1/chat/completions');
INSERT OR IGNORE INTO settings(key,value) VALUES('ai_model','gpt-4.1-mini');
INSERT INTO schema_migrations VALUES(3, unixepoch());
