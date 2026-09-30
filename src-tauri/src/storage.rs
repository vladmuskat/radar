//! Local persistence. Passwords use Argon2id; all queries use bound parameters.
use crate::{
    ports::{
        ArchiveItem, ArchiveOutcome, ArchivePage, HistoryItem, HistoryPage, IdentityProvider,
        Repository, User,
    },
    settings::Settings,
};
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use base64::Engine as _;
use radar_core::{Notice, Snapshot};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

pub struct Store(Connection);
/// Creates an Argon2id password hash with a unique random salt.
fn hash(password: &str) -> Result<String, String> {
    let salt =
        SaltString::encode_b64(uuid::Uuid::new_v4().as_bytes()).map_err(|e| e.to_string())?;
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| e.to_string())
}
/// Verifies a plaintext password without exposing the stored hash.
fn verifies(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash).is_ok_and(|h| {
        Argon2::default()
            .verify_password(password.as_bytes(), &h)
            .is_ok()
    })
}
impl Store {
    #[cfg(test)]
    pub fn fail_training_writes(&self, fail: bool) {
        self.0.execute_batch(if fail {
            "CREATE TEMP TRIGGER fail_training BEFORE INSERT ON trainings BEGIN SELECT RAISE(FAIL, 'injected SQLite failure'); END;"
        } else { "DROP TRIGGER fail_training;" }).unwrap();
    }
    /// Opens SQLite, creates its schema and seeds demo accounts when absent.
    pub fn open(path: &Path) -> Result<Self, String> {
        let db = Connection::open(path).map_err(|e| e.to_string())?;
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;
            CREATE TABLE IF NOT EXISTS users(id INTEGER PRIMARY KEY,login TEXT UNIQUE NOT NULL,name TEXT NOT NULL,role TEXT NOT NULL,password TEXT NOT NULL,settings TEXT NOT NULL DEFAULT '{}');
            CREATE TABLE IF NOT EXISTS trainings(id TEXT PRIMARY KEY,user_id INTEGER NOT NULL REFERENCES users(id),created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),snapshot TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS archives(id INTEGER PRIMARY KEY,user_id INTEGER NOT NULL REFERENCES users(id),session_id TEXT NOT NULL,event_id INTEGER NOT NULL,created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')),event TEXT NOT NULL,png BLOB NOT NULL,UNIQUE(session_id,event_id));
            PRAGMA user_version=1;").map_err(|e|e.to_string())?;
        for (login, name, role) in [
            ("admin", "Администратор", "administrator"),
            ("operator", "Оператор", "operator"),
        ] {
            let exists: bool = db
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM users WHERE login=?1)",
                    [login],
                    |r| r.get(0),
                )
                .map_err(|e| e.to_string())?;
            if !exists {
                db.execute(
                    "INSERT INTO users(login,name,role,password) VALUES(?1,?2,?3,?4)",
                    params![login, name, role, hash(login)?],
                )
                .map_err(|e| e.to_string())?;
            }
        }
        Ok(Self(db))
    }
    /// Authenticates one local account and returns its public profile.
    pub fn login(&self, login: &str, password: &str) -> Result<User, String> {
        let row = self
            .0
            .query_row(
                "SELECT id,login,name,role,password FROM users WHERE login=?1",
                [login],
                |r| {
                    Ok((
                        User {
                            id: r.get(0)?,
                            login: r.get(1)?,
                            name: r.get(2)?,
                            role: r.get(3)?,
                        },
                        r.get::<_, String>(4)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| e.to_string())?;
        match row {
            Some((user, h)) if verifies(password, &h) => Ok(user),
            _ => Err("Неверный логин или пароль".into()),
        }
    }
    /// Replaces a password after checking the old value and basic length rules.
    pub fn change_password(&self, id: i64, old: &str, new: &str) -> Result<(), String> {
        if !(8..=128).contains(&new.chars().count()) {
            return Err("Новый пароль: от 8 до 128 символов".into());
        }
        let h: String = self
            .0
            .query_row("SELECT password FROM users WHERE id=?1", [id], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        if !verifies(old, &h) {
            return Err("Текущий пароль неверен".into());
        }
        self.0
            .execute(
                "UPDATE users SET password=?1 WHERE id=?2",
                params![hash(new)?, id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    /// Idempotently persists a completed snapshot under its session UUID.
    pub fn save_training(&self, user: i64, snapshot: &Snapshot) -> Result<(), String> {
        self.0
            .execute(
                "INSERT OR IGNORE INTO trainings(id,user_id,snapshot) VALUES(?1,?2,?3)",
                params![
                    snapshot.session_id,
                    user,
                    serde_json::to_string(snapshot).map_err(|e| e.to_string())?
                ],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    /// Loads one history page plus totals calculated across every user session.
    pub fn history(&self, user: i64, offset: u32) -> Result<HistoryPage, String> {
        let (total, correct): (i64, i64) = self.0.query_row(
            "SELECT count(*),coalesce(sum(json_extract(snapshot,'$.statistics.correct')),0) FROM trainings WHERE user_id=?1",
            [user], |r| Ok((r.get(0)?,r.get(1)?))).map_err(|e| e.to_string())?;
        let mut stmt=self.0.prepare("SELECT created_at,snapshot FROM trainings WHERE user_id=?1 ORDER BY rowid DESC LIMIT 100 OFFSET ?2").map_err(|e|e.to_string())?;
        let rows = stmt
            .query_map(params![user, offset], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;
        let mut out = vec![];
        for row in rows {
            let (date, s) = row.map_err(|e| e.to_string())?;
            let snapshot: Snapshot = serde_json::from_str(&s).map_err(|e| e.to_string())?;
            out.push(HistoryItem { date, snapshot });
        }
        Ok(HistoryPage {
            items: out,
            total,
            correct,
            offset,
            limit: 100,
        })
    }
    /// Lists local user profiles without password hashes.
    pub fn users(&self) -> Result<Vec<User>, String> {
        let mut stmt = self
            .0
            .prepare("SELECT id,login,name,role FROM users ORDER BY id")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| {
                Ok(User {
                    id: r.get(0)?,
                    login: r.get(1)?,
                    name: r.get(2)?,
                    role: r.get(3)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }
    /// Validates and updates one local account role.
    pub fn set_role(&self, id: i64, role: &str) -> Result<(), String> {
        if !["operator", "administrator"].contains(&role) {
            return Err("Неизвестная роль".into());
        }
        self.0
            .execute("UPDATE users SET role=?1 WHERE id=?2", params![role, id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    /// Loads validated settings for a user or defaults before authentication.
    pub fn settings(&self, user: Option<i64>) -> Result<Settings, String> {
        let Some(id) = user else {
            return Ok(Settings::default());
        };
        let s: String = self
            .0
            .query_row("SELECT settings FROM users WHERE id=?1", [id], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        Ok(Settings::restore(&s))
    }
    /// Validates and atomically replaces the user's settings JSON.
    pub fn save_settings(&self, user: i64, settings: &Settings) -> Result<(), String> {
        settings.validate()?;
        let s = serde_json::to_string(settings).map_err(|e| e.to_string())?;
        if s.len() > 10000 {
            return Err("Настройки слишком велики".into());
        }
        self.0
            .execute("UPDATE users SET settings=?1 WHERE id=?2", params![s, user])
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    /// Stores a bounded PNG event capture with an atomic per-session quota.
    pub fn archive(
        &self,
        user: i64,
        session: &str,
        event: &Notice,
        data: &str,
    ) -> Result<ArchiveOutcome, String> {
        if data.len() > 6_000_000 {
            return Err("Размер снимка превышает 4 МБ".into());
        }
        let encoded = data
            .strip_prefix("data:image/png;base64,")
            .ok_or("Ожидается PNG")?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|e| e.to_string())?;
        if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            return Err("Некорректный PNG".into());
        }
        let transaction = self.0.unchecked_transaction().map_err(|e| e.to_string())?;
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM archives WHERE user_id=?1 AND session_id=?2 AND event_id=?3)",
            params![user, session, event.id], |r| r.get(0)).map_err(|e| e.to_string())?;
        if exists {
            return Ok(ArchiveOutcome::AlreadySaved);
        }
        let count: i64 = transaction
            .query_row(
                "SELECT count(*) FROM archives WHERE session_id=?1",
                [session],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if count >= 30 {
            return Ok(ArchiveOutcome::LimitReached);
        }
        transaction.execute("INSERT INTO archives(user_id,session_id,event_id,event,png) VALUES(?1,?2,?3,?4,?5)",params![user,session,event.id,serde_json::to_string(event).map_err(|e|e.to_string())?,bytes]).map_err(|e|e.to_string())?;
        transaction.commit().map_err(|e| e.to_string())?;
        Ok(ArchiveOutcome::Saved)
    }
    /// Checks whether one session event already has an archived image.
    pub fn has_archive(&self, user: i64, session: &str, event: u64) -> Result<bool, String> {
        self.0.query_row("SELECT EXISTS(SELECT 1 FROM archives WHERE user_id=?1 AND session_id=?2 AND event_id=?3)", params![user, session, event], |r| r.get(0)).map_err(|e| e.to_string())
    }
    /// Loads a metadata-only archive page, leaving PNG bytes in SQLite.
    pub fn archives(&self, user: i64, offset: u32) -> Result<ArchivePage, String> {
        let total: i64 = self
            .0
            .query_row(
                "SELECT count(*) FROM archives WHERE user_id=?1",
                [user],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        let mut stmt=self.0.prepare("SELECT id,created_at,event FROM archives WHERE user_id=?1 ORDER BY id DESC LIMIT 100 OFFSET ?2").map_err(|e|e.to_string())?;
        let rows = stmt
            .query_map(params![user, offset], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        let mut out = vec![];
        for row in rows {
            let (id, date, event) = row.map_err(|e| e.to_string())?;
            out.push(ArchiveItem {
                id,
                date,
                event: serde_json::from_str(&event).map_err(|e| e.to_string())?,
            });
        }
        Ok(ArchivePage {
            items: out,
            total,
            offset,
            limit: 100,
        })
    }
    /// Returns an owned archive image as a browser-ready data URL.
    pub fn archive_image(&self, user: i64, id: i64) -> Result<String, String> {
        let png: Vec<u8> = self
            .0
            .query_row(
                "SELECT png FROM archives WHERE id=?1 AND user_id=?2",
                params![id, user],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        Ok(format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(png)
        ))
    }
}

impl IdentityProvider for Store {
    fn login(&self, login: &str, password: &str) -> Result<User, String> {
        Store::login(self, login, password)
    }
    fn change_password(&self, user: i64, old: &str, new: &str) -> Result<(), String> {
        Store::change_password(self, user, old, new)
    }
    fn users(&self) -> Result<Vec<User>, String> {
        Store::users(self)
    }
    fn set_role(&self, user: i64, role: &str) -> Result<(), String> {
        Store::set_role(self, user, role)
    }
}
impl Repository for Store {
    fn save_training(&self, user: i64, snapshot: &Snapshot) -> Result<(), String> {
        Store::save_training(self, user, snapshot)
    }
    fn history(&self, user: i64, offset: u32) -> Result<HistoryPage, String> {
        Store::history(self, user, offset)
    }
    fn settings(&self, user: Option<i64>) -> Result<Settings, String> {
        Store::settings(self, user)
    }
    fn save_settings(&self, user: i64, settings: &Settings) -> Result<(), String> {
        Store::save_settings(self, user, settings)
    }
    fn archive(
        &self,
        user: i64,
        session: &str,
        event: &Notice,
        data: &str,
    ) -> Result<ArchiveOutcome, String> {
        Store::archive(self, user, session, event, data)
    }
    fn has_archive(&self, user: i64, session: &str, event: u64) -> Result<bool, String> {
        Store::has_archive(self, user, session, event)
    }
    fn archives(&self, user: i64, offset: u32) -> Result<ArchivePage, String> {
        Store::archives(self, user, offset)
    }
    fn archive_image(&self, user: i64, id: i64) -> Result<String, String> {
        Store::archive_image(self, user, id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a1XkAAAAASUVORK5CYII=";
    #[test]
    fn archive_quota_is_explicit_and_duplicate_at_quota_stays_idempotent() {
        let s = Store::open(Path::new(":memory:")).unwrap();
        let mut event = Notice {
            id: 1,
            target_id: 1,
            zone_name: "test".into(),
            time_ms: 50,
            position: radar_core::Point { x: 0., y: 0. },
            speed_mps: 30.,
        };
        for id in 1..=30 {
            event.id = id;
            assert_eq!(
                s.archive(1, "quota", &event, PNG).unwrap(),
                ArchiveOutcome::Saved
            );
        }
        assert_eq!(
            s.archive(1, "quota", &event, PNG).unwrap(),
            ArchiveOutcome::AlreadySaved
        );
        event.id = 31;
        assert_eq!(
            s.archive(1, "quota", &event, PNG).unwrap(),
            ArchiveOutcome::LimitReached
        );
        assert_eq!(s.archives(1, 0).unwrap().total, 30);
        assert_eq!(
            s.archive(1, "next-session", &event, PNG).unwrap(),
            ArchiveOutcome::Saved
        );
    }
    #[test]
    fn settings_reject_invalid_write_without_overwriting_and_restore_old_records() {
        let s = Store::open(Path::new(":memory:")).unwrap();
        let valid = Settings {
            sound: true,
            ..Settings::default()
        };
        s.save_settings(1, &valid).unwrap();
        assert!(s
            .save_settings(
                1,
                &Settings {
                    volume: 200,
                    ..valid.clone()
                }
            )
            .is_err());
        assert_eq!(s.settings(Some(1)).unwrap(), valid);
        s.0.execute(
            "UPDATE users SET settings=?1 WHERE id=1",
            [r#"{"sound":"yes","volume":-10}"#],
        )
        .unwrap();
        assert_eq!(s.settings(Some(1)).unwrap(), Settings::default());
        s.0.execute(
            "UPDATE users SET settings=?1 WHERE id=1",
            [r#"{"sound":true}"#],
        )
        .unwrap();
        assert_eq!(s.settings(Some(1)).unwrap(), valid);
    }
    #[test]
    fn history_pages_and_totals_cover_all_sessions() {
        let s = Store::open(Path::new(":memory:")).unwrap();
        for i in 0..105 {
            let mut snapshot =
                radar_core::Engine::new(radar_core::Config::default(), format!("history-{i}"))
                    .unwrap()
                    .snapshot();
            snapshot.statistics.correct = 2;
            s.save_training(1, &snapshot).unwrap();
        }
        let first = s.history(1, 0).unwrap();
        let second = s.history(1, 100).unwrap();
        assert_eq!(first.total, 105);
        assert_eq!(first.correct, 210);
        assert_eq!(first.items.len(), 100);
        assert_eq!(second.items.len(), 5);
        assert_eq!(second.items[4].snapshot.session_id, "history-0");
        assert_eq!(s.history(2, 0).unwrap().total, 0);
    }
    #[test]
    fn auth_persistence_and_roles() {
        let s = Store::open(Path::new(":memory:")).unwrap();
        let u = s.login("operator", "operator").unwrap();
        assert!(s.login("operator", "bad").is_err());
        s.change_password(u.id, "operator", "new-password").unwrap();
        assert!(s.login("operator", "operator").is_err());
        assert!(s.login("operator", "new-password").is_ok());
        assert!(s.set_role(u.id, "root").is_err());
        let mut e = radar_core::Engine::new(radar_core::Config::default(), "a".into()).unwrap();
        e.start().unwrap();
        e.finish();
        s.save_training(u.id, &e.snapshot()).unwrap();
        s.save_training(u.id, &e.snapshot()).unwrap();
        assert_eq!(s.history(u.id, 0).unwrap().items.len(), 1);
        assert_eq!(s.history(999, 0).unwrap().total, 0);
    }
}
