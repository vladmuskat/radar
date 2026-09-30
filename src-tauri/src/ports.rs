//! Application-owned contracts. No SQL, HTTP or Tauri types cross these boundaries.
use crate::settings::Settings;
use radar_core::{Notice, Snapshot};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct User {
    pub id: i64,
    pub login: String,
    pub name: String,
    pub role: String,
}
#[derive(Serialize)]
pub struct HistoryItem {
    pub date: String,
    pub snapshot: Snapshot,
}
#[derive(Serialize)]
pub struct HistoryPage {
    pub items: Vec<HistoryItem>,
    pub total: i64,
    pub correct: i64,
    pub offset: u32,
    pub limit: u32,
}
#[derive(Serialize)]
pub struct ArchiveItem {
    pub id: i64,
    pub date: String,
    pub event: Notice,
}
#[derive(Serialize)]
pub struct ArchivePage {
    pub items: Vec<ArchiveItem>,
    pub total: i64,
    pub offset: u32,
    pub limit: u32,
}
#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ArchiveOutcome {
    Saved,
    AlreadySaved,
    LimitReached,
}

/// All user IDs are application profile IDs, not external provider subjects.
pub trait IdentityProvider {
    fn login(&self, login: &str, password: &str) -> Result<User, String>;
    fn change_password(&self, user: i64, old: &str, new: &str) -> Result<(), String>;
    fn users(&self) -> Result<Vec<User>, String>;
    fn set_role(&self, user: i64, role: &str) -> Result<(), String>;
}
/// Saving a training must be idempotent by session ID; archive quotas are atomic.
/// Successful writes mean durable completion, not merely an enqueued remote request.
pub trait Repository {
    fn save_training(&self, user: i64, snapshot: &Snapshot) -> Result<(), String>;
    fn history(&self, user: i64, offset: u32) -> Result<HistoryPage, String>;
    fn settings(&self, user: Option<i64>) -> Result<Settings, String>;
    fn save_settings(&self, user: i64, settings: &Settings) -> Result<(), String>;
    fn archive(
        &self,
        user: i64,
        session: &str,
        event: &Notice,
        data: &str,
    ) -> Result<ArchiveOutcome, String>;
    fn has_archive(&self, user: i64, session: &str, event: u64) -> Result<bool, String>;
    fn archives(&self, user: i64, offset: u32) -> Result<ArchivePage, String>;
    fn archive_image(&self, user: i64, id: i64) -> Result<String, String>;
}
