CREATE TABLE tasks(
  id INTEGER PRIMARY KEY,
  title BLOB NOT NULL,
  notes BLOB,
  result BLOB,
  status TEXT NOT NULL CHECK(status IN ('inbox','today','in_progress','completed','cancelled')),
  planned_date TEXT,
  date_kind TEXT NOT NULL DEFAULT 'none' CHECK(date_kind IN ('none','exact','month_tbd','uncertain')),
  due_at INTEGER,
  priority TEXT NOT NULL DEFAULT 'medium' CHECK(priority IN ('high','medium','low')),
  project BLOB,
  source TEXT NOT NULL DEFAULT 'manual',
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  completed_at INTEGER
);
CREATE INDEX tasks_status_date ON tasks(status, planned_date, updated_at DESC);
CREATE INDEX tasks_due ON tasks(due_at) WHERE due_at IS NOT NULL;

CREATE TABLE audit_events(
  id INTEGER PRIMARY KEY,
  entity_type TEXT NOT NULL,
  entity_id TEXT NOT NULL,
  action TEXT NOT NULL,
  before_json TEXT,
  after_json TEXT,
  reason BLOB,
  created_at INTEGER NOT NULL
);

CREATE TABLE secure_settings(key TEXT PRIMARY KEY, value BLOB NOT NULL);

ALTER TABLE workdays ADD COLUMN target_seconds INTEGER NOT NULL DEFAULT 32400;
ALTER TABLE workdays ADD COLUMN revised_at INTEGER;
ALTER TABLE workdays ADD COLUMN revision_reason BLOB;

INSERT INTO tasks(title,status,date_kind,priority,source,created_at,updated_at)
SELECT title,'inbox','none','medium','capture',created_at,created_at FROM captures;

INSERT INTO schema_migrations VALUES(2, unixepoch());
