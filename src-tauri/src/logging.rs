//! Local diagnostics only: never serialize commands, credentials, snapshots or PNGs.
use serde::Deserialize;
use std::{path::Path, sync::Mutex, time::Instant};
use tracing_appender::{
    non_blocking::WorkerGuard,
    rolling::{RollingFileAppender, Rotation},
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub struct Logging {
    guard: Mutex<Option<WorkerGuard>>,
    rate: Mutex<(Instant, u32)>,
    dropped: tracing_appender::non_blocking::ErrorCounter,
}
impl Logging {
    /// Emits shutdown diagnostics and drains the non-blocking file writer.
    pub fn flush(&self) {
        tracing::info!(
            event = "logging_shutdown",
            dropped_lines = self.dropped.dropped_lines()
        );
        if let Ok(mut guard) = self.guard.lock() {
            drop(guard.take());
        }
    }
    /// Applies a small per-second rate limit to untrusted frontend events.
    fn admit(&self) -> bool {
        let Ok(mut rate) = self.rate.lock() else {
            return false;
        };
        if rate.0.elapsed().as_secs() >= 1 {
            *rate = (Instant::now(), 0);
        }
        rate.1 += 1;
        rate.1 <= 20
    }
}

/// Initializes stderr and mandatory rolling JSONL file logging.
pub fn init(path: &Path) -> Result<Logging, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(path)?;
    let appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("radar")
        .filename_suffix("jsonl")
        .max_log_files(7)
        .build(path)?;
    let (writer, guard) = tracing_appender::non_blocking::NonBlockingBuilder::default()
        .buffered_lines_limit(4096)
        .lossy(true)
        .finish(appender);
    let dropped = writer.error_counter();
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("warn,radar_trainer=info"));
    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .with_ansi(false),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_ansi(false)
                .with_writer(writer),
        )
        .try_init()?;
    // Do not log panic payloads: they may contain user input. Keep source location.
    std::panic::set_hook(Box::new(|info| {
        if let Some(location) = info.location() {
            tracing::error!(
                event = "rust_panic",
                file = location.file(),
                line = location.line()
            );
        } else {
            tracing::error!(event = "rust_panic");
        }
    }));
    tracing::info!(event = "application_started", version = env!("CARGO_PKG_VERSION"), log_dir = %path.display());
    Ok(Logging {
        guard: Mutex::new(Some(guard)),
        rate: Mutex::new((Instant::now(), 0)),
        dropped,
    })
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum FrontendEvent {
    UnhandledError,
    UnhandledRejection,
    ReactRenderFailed,
    IpcFailed,
    MapFailed,
    CaptureFailed,
    AudioFailed,
}
#[derive(Deserialize, Debug)]
pub enum ErrorKind {
    Error,
    TypeError,
    RangeError,
    ReferenceError,
    SyntaxError,
    DOMException,
    Unknown,
}

#[derive(Deserialize, serde::Serialize)]
pub struct Frame {
    script: String,
    line: u32,
    column: u32,
}
impl Frame {
    /// Accepts only bounded local JS/TS filenames without URL or message data.
    fn safe(&self) -> bool {
        self.script.len() <= 100
            && self
                .script
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
            && [".js", ".ts", ".tsx"]
                .iter()
                .any(|ext| self.script.ends_with(ext))
    }
}

/// Enum-only metadata prevents credentials/PNG/error payloads reaching files from JS.
#[tauri::command]
pub fn frontend_log(
    event: FrontendEvent,
    kind: ErrorKind,
    request_id: Option<String>,
    frames: Vec<Frame>,
    logging: tauri::State<'_, Logging>,
) {
    if !logging.admit() {
        return;
    }
    let request_id = request_id
        .and_then(|id| uuid::Uuid::parse_str(&id).ok())
        .map(|id| id.to_string());
    let frames: Vec<_> = frames.into_iter().filter(Frame::safe).take(8).collect();
    tracing::warn!(target: "radar_trainer::frontend", ?event, ?kind, ?request_id, frames=%serde_json::json!(frames), "Frontend failure");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frontend_contract_rejects_arbitrary_payloads() {
        assert!(serde_json::from_str::<FrontendEvent>("\"password=secret\"").is_err());
        assert!(serde_json::from_str::<ErrorKind>("\"data:image/png;base64,secret\"").is_err());
        assert!(serde_json::from_str::<FrontendEvent>("\"ipc_failed\"").is_ok());
    }
    #[test]
    fn rotated_file_is_json_and_guard_flushes() {
        let directory =
            std::env::temp_dir().join(format!("radar-log-test-{}", uuid::Uuid::new_v4()));
        let appender = RollingFileAppender::builder()
            .rotation(Rotation::DAILY)
            .filename_prefix("radar")
            .filename_suffix("jsonl")
            .max_log_files(7)
            .build(&directory)
            .unwrap();
        let (writer, guard) = tracing_appender::non_blocking(appender);
        let subscriber = tracing_subscriber::fmt()
            .json()
            .with_writer(writer)
            .finish();
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(
                event = "test_event",
                request_id = "test-request",
                elapsed_ms = 12
            )
        });
        drop(guard);
        let file = std::fs::read_dir(&directory)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let text = std::fs::read_to_string(&file).unwrap();
        let value: serde_json::Value = serde_json::from_str(text.trim()).unwrap();
        assert_eq!(value["fields"]["event"], "test_event");
        assert_eq!(value["fields"]["elapsed_ms"], 12);
        std::fs::remove_file(file).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}
