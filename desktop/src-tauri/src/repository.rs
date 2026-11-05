use crate::crypto::Crypto;
use chrono::{Local, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::path::Path;

pub struct Store {
    pub db: Connection,
    crypto: Crypto,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub day: String,
    pub started_at: Option<i64>,
    pub ended_at: Option<i64>,
    pub status: String,
    pub server_now: i64,
    pub capture_count: i64,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capture {
    id: i64,
    title: String,
    created_at: i64,
}
fn db_error(_: rusqlite::Error) -> String {
    "数据库操作失败；请检查文件占用或磁盘空间".into()
}
impl Store {
    pub fn open(dir: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(dir).map_err(|_| "数据目录创建失败")?;
        let path = dir.join("app.db");
        let crypto = Crypto::open(dir, path.exists())?;
        let mut db = Connection::open(&path).map_err(db_error)?;
        db.busy_timeout(std::time::Duration::from_secs(3))
            .map_err(db_error)?;
        db.pragma_update(None, "foreign_keys", "ON")
            .map_err(db_error)?;
        let version: i64 = db
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(db_error)?;
        if version > 1 {
            return Err("数据库来自较新版本，请升级应用".into());
        }
        if version < 1 {
            db.backup(
                rusqlite::DatabaseName::Main,
                dir.join("before-migration-v1.db"),
                None,
            )
            .map_err(db_error)?;
            let tx = db.transaction().map_err(db_error)?;
            tx.execute_batch(include_str!("../migrations/001_initial.sql"))
                .map_err(db_error)?;
            tx.pragma_update(None, "user_version", 1)
                .map_err(db_error)?;
            tx.commit().map_err(db_error)?;
        }
        db.pragma_update(None, "journal_mode", "WAL")
            .map_err(db_error)?;
        Ok(Self { db, crypto })
    }
    pub fn snapshot(&self) -> Result<Snapshot, String> {
        self.snapshot_at(
            Utc::now().timestamp(),
            &Local::now().format("%Y-%m-%d").to_string(),
        )
    }
    fn snapshot_at(&self, now: i64, today: &str) -> Result<Snapshot, String> {
        let row:Option<(String,i64,Option<i64>)>=self.db.query_row("SELECT day,started_at,ended_at FROM workdays WHERE ended_at IS NULL OR day=?1 ORDER BY started_at LIMIT 1",[today],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(db_error)?;
        let (day, started_at, ended_at) =
            row.map(|(d, s, e)| (d, Some(s), e))
                .unwrap_or((today.into(), None, None));
        let status = match (started_at, ended_at) {
            (_, Some(_)) => "finished",
            (Some(s), None) if now - s >= 9 * 3600 => "overtime",
            (Some(_), None) => "working",
            _ => "idle",
        }
        .into();
        Ok(Snapshot {
            day,
            started_at,
            ended_at,
            status,
            server_now: now,
            capture_count: self
                .db
                .query_row("SELECT count(*) FROM captures", [], |r| r.get(0))
                .map_err(db_error)?,
        })
    }
    pub fn clock(&mut self, action: &str) -> Result<(), String> {
        self.clock_at(
            action,
            Utc::now().timestamp(),
            &Local::now().format("%Y-%m-%d").to_string(),
        )
    }
    fn clock_at(&mut self, action: &str, now: i64, today: &str) -> Result<(), String> {
        let tx = self.db.transaction().map_err(db_error)?;
        let active: Option<(String, i64)> = tx
            .query_row(
                "SELECT day,started_at FROM workdays WHERE ended_at IS NULL LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(db_error)?;
        match action {
            "start" => {
                if active.is_some() {
                    return Err("已有进行中的打卡".into());
                }
                let exists: bool = tx
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM workdays WHERE day=?1)",
                        [today],
                        |r| r.get(0),
                    )
                    .map_err(db_error)?;
                if exists {
                    return Err("当日已下班，不能重新打卡".into());
                }
                tx.execute(
                    "INSERT INTO workdays(day,started_at) VALUES(?1,?2)",
                    params![today, now],
                )
                .map_err(db_error)?;
            }
            "finish" => {
                let (day, start) = active.ok_or("当前没有进行中的打卡")?;
                if now < start {
                    return Err("系统时间早于上班时间，请校正系统时钟".into());
                }
                tx.execute(
                    "UPDATE workdays SET ended_at=?1 WHERE day=?2 AND ended_at IS NULL",
                    params![now, day],
                )
                .map_err(db_error)?;
            }
            _ => return Err("无效打卡操作".into()),
        };
        tx.commit().map_err(db_error)
    }
    pub fn add(&self, title: &str) -> Result<(), String> {
        let title = title.trim();
        if title.is_empty() || title.chars().count() > 2000 {
            return Err("请输入 1–2000 字的内容".into());
        }
        let encrypted = self.crypto.encrypt(title)?;
        self.db
            .execute(
                "INSERT INTO captures(title,created_at) VALUES(?1,?2)",
                params![encrypted, Utc::now().timestamp()],
            )
            .map_err(db_error)?;
        Ok(())
    }
    pub fn captures(&self) -> Result<Vec<Capture>, String> {
        let mut stmt = self
            .db
            .prepare("SELECT id,title,created_at FROM captures ORDER BY id DESC LIMIT 50")
            .map_err(db_error)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, Vec<u8>>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })
            .map_err(db_error)?;
        rows.map(|row| {
            let (id, title, created_at) = row.map_err(db_error)?;
            Ok(Capture {
                id,
                title: self.crypto.decrypt(&title)?,
                created_at,
            })
        })
        .collect()
    }
    pub fn setting(&self, key: &str) -> Option<String> {
        self.db
            .query_row("SELECT value FROM settings WHERE key=?1", [key], |r| {
                r.get(0)
            })
            .optional()
            .ok()
            .flatten()
    }
    pub fn set(&self, key: &str, value: &str) -> Result<(), String> {
        self.db.execute("INSERT INTO settings VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",params![key,value]).map_err(db_error)?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn store() -> (Store, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!("eow-test-{}", rand::random::<u64>()));
        (Store::open(&path).unwrap(), path)
    }
    #[test]
    fn clock_restart_midnight_and_lock() {
        let (mut s, p) = store();
        s.clock_at("start", 100, "2026-11-05").unwrap();
        assert!(s.clock_at("start", 200, "2026-11-05").is_err());
        assert_eq!(
            s.snapshot_at(32500, "2026-11-06").unwrap().status,
            "overtime"
        );
        drop(s);
        let mut s = Store::open(&p).unwrap();
        assert_eq!(s.snapshot_at(400, "2026-11-06").unwrap().day, "2026-11-05");
        assert!(s.clock_at("finish", 99, "2026-11-06").is_err());
        s.clock_at("finish", 500, "2026-11-06").unwrap();
        assert!(s.clock_at("start", 600, "2026-11-05").is_err());
        s.clock_at("start", 600, "2026-11-06").unwrap();
        drop(s);
        std::fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn migration_and_ciphertext() {
        let (s, p) = store();
        s.add("客户保密事项").unwrap();
        assert_eq!(s.captures().unwrap()[0].title, "客户保密事项");
        let raw: Vec<u8> =
            s.db.query_row("SELECT title FROM captures", [], |r| r.get(0))
                .unwrap();
        assert!(!raw.windows("客户".len()).any(|w| w == "客户".as_bytes()));
        assert!(s.add("").is_err());
        drop(s);
        let s = Store::open(&p).unwrap();
        assert_eq!(s.captures().unwrap().len(), 1);
        assert!(p.join("before-migration-v1.db").exists());
        drop(s);
        std::fs::remove_dir_all(p).unwrap();
    }
}
