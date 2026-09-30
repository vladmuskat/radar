#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod logging;
mod ports;
mod settings;
mod storage;

use ports::{ArchiveOutcome, IdentityProvider, Repository, User};
use radar_core::{Config, Engine, Notice, Snapshot, Status, STEP_MS};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use settings::Settings;
use std::{
    rc::Rc,
    sync::mpsc,
    time::{Duration, Instant},
};
use storage::Store;
use tauri::{ipc::Channel, Emitter, Manager, State};

#[derive(Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum Command {
    Bootstrap,
    Login {
        login: String,
        password: String,
    },
    Logout,
    PrepareClose,
    ValidateConfig {
        config: Config,
    },
    Create {
        config: Config,
    },
    Start {
        session_id: String,
    },
    Pause {
        session_id: String,
    },
    PauseForTransition {
        session_id: String,
    },
    Resume {
        session_id: String,
    },
    Identify {
        session_id: String,
        target_id: u64,
    },
    Finish {
        session_id: String,
    },
    Reset {
        session_id: String,
    },
    History {
        #[serde(default)]
        offset: u32,
    },
    ChangePassword {
        old_password: String,
        new_password: String,
    },
    Users,
    SetRole {
        user_id: i64,
        role: String,
    },
    SaveSettings {
        settings: Settings,
    },
    ReserveArchive {
        session_id: String,
        event_id: u64,
    },
    CancelArchive {
        session_id: String,
        event_id: u64,
    },
    ArchiveImage {
        session_id: String,
        event_id: u64,
        data_url: String,
    },
    Archives {
        #[serde(default)]
        offset: u32,
    },
    ArchiveImageData {
        id: i64,
    },
}
impl Command {
    /// Returns an allowlisted log name without serializing secret command data.
    fn name(&self) -> &'static str {
        match self {
            Self::Bootstrap => "bootstrap",
            Self::Login { .. } => "login",
            Self::Logout => "logout",
            Self::PrepareClose => "prepareClose",
            Self::ValidateConfig { .. } => "validateConfig",
            Self::Create { .. } => "create",
            Self::Start { .. } => "start",
            Self::Pause { .. } => "pause",
            Self::PauseForTransition { .. } => "pauseForTransition",
            Self::Resume { .. } => "resume",
            Self::Identify { .. } => "identify",
            Self::Finish { .. } => "finish",
            Self::Reset { .. } => "reset",
            Self::History { .. } => "history",
            Self::ChangePassword { .. } => "changePassword",
            Self::Users => "users",
            Self::SetRole { .. } => "setRole",
            Self::SaveSettings { .. } => "saveSettings",
            Self::ReserveArchive { .. } => "reserveArchive",
            Self::CancelArchive { .. } => "cancelArchive",
            Self::ArchiveImage { .. } => "archiveImage",
            Self::Archives { .. } => "archives",
            Self::ArchiveImageData { .. } => "archiveImageData",
        }
    }
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Packet {
    #[serde(flatten)]
    snapshot: Snapshot,
    save_error: Option<String>,
}
enum Message {
    Request(Command, String, mpsc::Sender<Result<Value, String>>),
    Subscribe(Channel<Packet>, mpsc::Sender<Result<Packet, String>>),
}
struct Service(mpsc::Sender<Message>);

/// Only an acknowledged durable save allows the process to exit.
#[tauri::command]
async fn close_application(
    app: tauri::AppHandle,
    service: State<'_, Service>,
) -> Result<(), String> {
    request(Command::PrepareClose, None, service).await?;
    tracing::info!(event = "application_exit_requested");
    app.exit(0);
    Ok(())
}

#[tauri::command]
async fn request(
    command: Command,
    request_id: Option<String>,
    service: State<'_, Service>,
) -> Result<Value, String> {
    let tx = service.0.clone();
    let request_id = request_id
        .and_then(|id| uuid::Uuid::parse_str(&id).ok())
        .unwrap_or_else(uuid::Uuid::new_v4)
        .to_string();
    tauri::async_runtime::spawn_blocking(move || {
        let (reply, rx) = mpsc::channel();
        tx.send(Message::Request(command, request_id.clone(), reply))
            .map_err(|_| {
                tracing::error!(%request_id, event="worker_unavailable");
                "Движок недоступен"
            })?;
        rx.recv_timeout(Duration::from_secs(15)).map_err(|_| {
            tracing::error!(%request_id, event="command_reply_unavailable");
            "Истекло время ожидания команды"
        })?
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn subscribe(
    channel: Channel<Packet>,
    service: State<'_, Service>,
) -> Result<Packet, String> {
    let tx = service.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let (reply, rx) = mpsc::channel();
        tx.send(Message::Subscribe(channel, reply))
            .map_err(|_| "Движок недоступен")?;
        rx.recv_timeout(Duration::from_secs(15))
            .map_err(|_| "Нет ответа движка")?
    })
    .await
    .map_err(|e| e.to_string())?
}

struct Worker {
    engine: Engine,
    store: Rc<dyn Repository>,
    identity: Rc<dyn IdentityProvider>,
    user: Option<User>,
    channel: Option<Channel<Packet>>,
    saved: bool,
    save_error: Option<String>,
    pending_archive: Option<(String, Notice)>,
}
impl Worker {
    /// Assembles the application service from replaceable domain ports.
    fn new(engine: Engine, store: Rc<dyn Repository>, identity: Rc<dyn IdentityProvider>) -> Self {
        Self {
            engine,
            store,
            identity,
            user: None,
            channel: None,
            saved: false,
            save_error: None,
            pending_archive: None,
        }
    }
    /// Blocks session transitions while captured bytes still need persistence.
    fn require_archive_saved(&self) -> Result<(), String> {
        if self.pending_archive.is_some() {
            return Err("Дождитесь сохранения снимка перед сменой сеанса".into());
        }
        Ok(())
    }
    /// Combines the safe domain snapshot with a recoverable save error.
    fn packet(&self) -> Packet {
        Packet {
            snapshot: self.engine.snapshot(),
            save_error: self.save_error.clone(),
        }
    }
    /// Returns the authenticated profile or rejects protected operations.
    fn require_user(&self) -> Result<&User, String> {
        self.user
            .as_ref()
            .ok_or("Необходимо войти в систему".into())
    }
    /// Rejects commands carrying an obsolete or foreign session UUID.
    fn require_session(&self, id: &str) -> Result<(), String> {
        self.require_user()?;
        if self.engine.snapshot().session_id != id {
            return Err("Команда относится к предыдущей тренировке".into());
        }
        Ok(())
    }
    /// Durably saves a finished result once and retains failures for retry.
    fn save_finished(&mut self) -> Result<(), String> {
        if self.engine.status == Status::Finished && !self.saved {
            if let Some(u) = &self.user {
                if let Err(error) = self.store.save_training(u.id, &self.engine.snapshot()) {
                    tracing::error!(event="training_save_failed", error=%error);
                    self.save_error = Some(format!("Результат не сохранён: {error}"));
                    return Err(error);
                }
                self.saved = true;
                tracing::info!(event="training_saved", session_id=%self.engine.snapshot().session_id);
                self.save_error = None;
            }
        }
        Ok(())
    }
    /// Adds structured timing and transition logs around command dispatch.
    fn handle(&mut self, command: Command) -> Result<Value, String> {
        let name = command.name();
        let span = tracing::info_span!("command", command=name, session_id=%self.engine.snapshot().session_id);
        let _entered = span.enter();
        let started = Instant::now();
        tracing::debug!(event = "command_started");
        let before = self.engine.status;
        let result = self.handle_inner(command);
        let elapsed_ms = started.elapsed().as_millis() as u64;
        match &result {
            Ok(_) => {
                tracing::info!(event="command_completed", elapsed_ms, status=?self.engine.status)
            }
            Err(error) => tracing::warn!(event="command_failed", elapsed_ms, error=%error),
        }
        if before != self.engine.status {
            tracing::info!(event="session_transition", from=?before, to=?self.engine.status, new_session_id=%self.engine.snapshot().session_id);
        }
        result
    }
    /// Enforces authorization and executes the concrete application command.
    fn handle_inner(&mut self, command: Command) -> Result<Value, String> {
        match command {
            Command::Bootstrap => {
                return Ok(
                    json!({"user":self.user,"config":self.engine.config,"settings":self.store.settings(self.user.as_ref().map(|u|u.id))?}),
                )
            }
            Command::Login { login, password } => {
                if self.user.is_some() {
                    return Err("Сначала завершите текущую сессию входа".into());
                }
                self.user = Some(self.identity.login(&login, &password)?);
                return Ok(json!(self.user));
            }
            Command::Logout => {
                self.require_user()?;
                self.require_archive_saved()?;
                if matches!(self.engine.status, Status::Running | Status::Paused) {
                    self.engine.finish();
                }
                self.save_finished()?;
                self.user = None;
                self.channel = None;
                self.engine = Engine::new(Config::default(), uuid::Uuid::new_v4().to_string())?;
                self.saved = false;
                self.save_error = None;
                return Ok(Value::Null);
            }
            Command::PrepareClose => {
                self.require_archive_saved()?;
                if matches!(self.engine.status, Status::Running | Status::Paused) {
                    self.engine.finish();
                }
                self.save_finished()?;
                return Ok(Value::Null);
            }
            Command::ValidateConfig { config } => {
                self.require_user()?;
                config.validate()?;
                return Ok(Value::Null);
            }
            Command::Create { config } => {
                self.require_user()?;
                self.require_archive_saved()?;
                if matches!(self.engine.status, Status::Running | Status::Paused) {
                    return Err("Сначала завершите текущую тренировку".into());
                }
                self.save_finished()?;
                self.engine = Engine::new(config, uuid::Uuid::new_v4().to_string())?;
                self.saved = false;
                self.save_error = None;
            }
            Command::Start { session_id } => {
                self.require_session(&session_id)?;
                self.engine.start()?;
            }
            Command::Pause { session_id } => {
                self.require_session(&session_id)?;
                self.engine.pause()?;
            }
            Command::PauseForTransition { session_id } => {
                self.require_session(&session_id)?;
                // Natural completion may race the UI's request to leave the session.
                if self.engine.status == Status::Running {
                    self.engine.pause()?;
                }
            }
            Command::Resume { session_id } => {
                self.require_session(&session_id)?;
                self.engine.resume()?;
            }
            Command::Identify {
                session_id,
                target_id,
            } => {
                self.require_session(&session_id)?;
                self.engine.identify(target_id)?;
            }
            Command::Finish { session_id } => {
                self.require_session(&session_id)?;
                if self.engine.status == Status::Ready {
                    return Err("Тренировка ещё не началась".into());
                }
                self.engine.finish();
                self.save_finished()?;
            }
            Command::Reset { session_id } => {
                self.require_session(&session_id)?;
                self.require_archive_saved()?;
                if matches!(self.engine.status, Status::Running | Status::Paused) {
                    self.engine.finish();
                }
                self.save_finished()?;
                self.engine =
                    Engine::new(self.engine.config.clone(), uuid::Uuid::new_v4().to_string())?;
                self.saved = false;
                self.save_error = None;
            }
            Command::History { offset } => {
                return Ok(json!(self
                    .store
                    .history(self.require_user()?.id, offset)?))
            }
            Command::ChangePassword {
                old_password,
                new_password,
            } => {
                self.identity.change_password(
                    self.require_user()?.id,
                    &old_password,
                    &new_password,
                )?;
                return Ok(Value::Null);
            }
            Command::Users => {
                if self.require_user()?.role != "administrator" {
                    return Err("Требуются права администратора".into());
                }
                return Ok(json!(self.identity.users()?));
            }
            Command::SetRole { user_id, role } => {
                let u = self.require_user()?;
                if u.role != "administrator" {
                    return Err("Требуются права администратора".into());
                }
                if u.id == user_id {
                    return Err("Собственную роль менять нельзя".into());
                }
                self.identity.set_role(user_id, &role)?;
                return Ok(json!(self.identity.users()?));
            }
            Command::SaveSettings { settings } => {
                settings.validate()?;
                self.store
                    .save_settings(self.require_user()?.id, &settings)?;
                return Ok(Value::Null);
            }
            Command::ReserveArchive {
                session_id,
                event_id,
            } => {
                self.require_session(&session_id)?;
                self.require_archive_saved()?;
                let event = self
                    .engine
                    .snapshot()
                    .notifications
                    .into_iter()
                    .find(|n| n.id == event_id)
                    .ok_or("Событие уже недоступно")?;
                self.pending_archive = Some((session_id, event));
                return Ok(Value::Null);
            }
            Command::CancelArchive {
                session_id,
                event_id,
            } => {
                self.require_session(&session_id)?;
                if self
                    .pending_archive
                    .as_ref()
                    .is_some_and(|(id, n)| id == &session_id && n.id == event_id)
                {
                    self.pending_archive = None;
                }
                return Ok(Value::Null);
            }
            Command::ArchiveImage {
                session_id,
                event_id,
                data_url,
            } => {
                self.require_session(&session_id)?;
                if self
                    .store
                    .has_archive(self.require_user()?.id, &session_id, event_id)?
                {
                    if self
                        .pending_archive
                        .as_ref()
                        .is_some_and(|(id, n)| id == &session_id && n.id == event_id)
                    {
                        self.pending_archive = None;
                    }
                    return Ok(json!(ArchiveOutcome::AlreadySaved));
                }
                let (_, event) = self
                    .pending_archive
                    .as_ref()
                    .filter(|(id, n)| id == &session_id && n.id == event_id)
                    .ok_or("Сначала зарезервируйте событие снимка")?;
                let outcome =
                    self.store
                        .archive(self.require_user()?.id, &session_id, event, &data_url)?;
                if outcome == ArchiveOutcome::LimitReached {
                    tracing::info!(event="archive_limit_reached", session_id=%session_id);
                }
                self.pending_archive = None;
                return Ok(json!(outcome));
            }
            Command::Archives { offset } => {
                return Ok(json!(self
                    .store
                    .archives(self.require_user()?.id, offset)?))
            }
            Command::ArchiveImageData { id } => {
                return Ok(json!(self
                    .store
                    .archive_image(self.require_user()?.id, id)?))
            }
        }
        Ok(json!(self.packet()))
    }
    /// Publishes the latest packet and pauses if the WebView channel disappears.
    fn publish(&mut self) {
        if let Some(ch) = &self.channel {
            if ch.send(self.packet()).is_err() {
                tracing::warn!(event = "snapshot_channel_disconnected");
                self.channel = None;
                if self.engine.status == Status::Running {
                    let _ = self.engine.pause();
                }
            }
        }
    }
    /// Owns the engine loop, serializes commands and schedules fixed-time ticks.
    fn run(mut self, rx: mpsc::Receiver<Message>) {
        let step = Duration::from_millis(STEP_MS);
        let mut deadline = Instant::now() + step;
        loop {
            match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(Message::Request(cmd, request_id, reply)) => {
                    let was = self.engine.status;
                    let result =
                        tracing::info_span!("ipc", %request_id).in_scope(|| self.handle(cmd));
                    if was != self.engine.status {
                        deadline = Instant::now() + step;
                    }
                    let _ = reply.send(result);
                    self.publish();
                }
                Ok(Message::Subscribe(channel, reply)) => {
                    tracing::info!(
                        event = "snapshot_subscription",
                        authenticated = self.user.is_some()
                    );
                    if self.user.is_none() {
                        let _ = reply.send(Err("Необходимо войти".into()));
                        continue;
                    }
                    // A reloaded window must explicitly resume its training.
                    if self.engine.status == Status::Running {
                        let _ = self.engine.pause();
                    }
                    // A fresh WebView has no old capture bytes. Do not leave an orphan reservation.
                    self.pending_archive = None;
                    self.channel = Some(channel);
                    let _ = reply.send(Ok(self.packet()));
                    self.publish();
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    tracing::warn!(event = "worker_stopped");
                    break;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            if Instant::now() >= deadline {
                // No catch-up after OS sleep or expensive operations: bounded real-time work.
                let changed = self.engine.status == Status::Running;
                self.engine.tick();
                if changed && self.engine.status == Status::Finished {
                    tracing::info!(event="training_finished_by_timer", session_id=%self.engine.snapshot().session_id);
                }
                deadline = Instant::now() + step;
                if changed {
                    if let Err(e) = self.save_finished() {
                        self.save_error = Some(format!("Результат не сохранён: {e}"));
                    }
                }
                if changed {
                    self.publish();
                }
            }
        }
    }
}

/// Builds the Tauri application and wires logging, storage, worker and IPC.
fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let log_dir = app.path().app_log_dir()?;
            app.manage(logging::init(&log_dir).map_err(|e| std::io::Error::other(e.to_string()))?);
            let path = app.path().app_data_dir()?;
            std::fs::create_dir_all(&path)?;
            let store = Store::open(&path.join("radar.sqlite3")).map_err(|error| {
                tracing::error!(event="database_open_failed", %error);
                std::io::Error::other(error)
            })?;
            let engine = Engine::new(Config::default(), uuid::Uuid::new_v4().to_string())
                .map_err(std::io::Error::other)?;
            let (tx, rx) = mpsc::channel();
            app.manage(Service(tx));
            std::thread::Builder::new()
                .name("radar-simulation".into())
                .spawn(move || {
                    // One SQLite connection, shared only inside its owning worker thread.
                    let store = Rc::new(store);
                    Worker::new(engine, store.clone(), store).run(rx)
                })?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                tracing::info!(event = "window_close_requested");
                let _ = window.emit("radar-close-request", ());
            }
        })
        .invoke_handler(tauri::generate_handler![
            request,
            subscribe,
            close_application,
            logging::frontend_log
        ])
        .build(tauri::generate_context!())
        .expect("Не удалось запустить РАДАР")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                if let Some(logging) = app.try_state::<logging::Logging>() {
                    logging.flush();
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestIdentity;
    impl IdentityProvider for TestIdentity {
        fn login(&self, _: &str, _: &str) -> Result<User, String> {
            // External identity mapped to an existing local application profile.
            Ok(User {
                id: 2,
                login: "external-operator".into(),
                name: "External".into(),
                role: "operator".into(),
            })
        }
        fn change_password(&self, _: i64, _: &str, _: &str) -> Result<(), String> {
            Err("Managed by identity provider".into())
        }
        fn users(&self) -> Result<Vec<User>, String> {
            Ok(vec![])
        }
        fn set_role(&self, _: i64, _: &str) -> Result<(), String> {
            Err("Managed by identity provider".into())
        }
    }

    #[test]
    fn identity_can_be_replaced_independently_of_repository_and_engine() {
        let (mut w, store) = worker_with_store();
        w.identity = Rc::new(TestIdentity);
        assert!(store
            .login("external-operator", "external-credential")
            .is_err());
        w.handle(Command::Login {
            login: "external-operator".into(),
            password: "external-credential".into(),
        })
        .unwrap();
        assert!(w.handle(Command::Users).is_err()); // Local authorization still enforced.
        w.handle(Command::Start {
            session_id: "initial".into(),
        })
        .unwrap();
        w.handle(Command::Finish {
            session_id: "initial".into(),
        })
        .unwrap();
        assert_eq!(store.history(2, 0).unwrap().total, 1);
        assert_eq!(store.history(1, 0).unwrap().total, 0);
        assert!(w
            .handle(Command::ChangePassword {
                old_password: "old".into(),
                new_password: "new-password".into()
            })
            .is_err());
    }

    fn worker() -> Worker {
        worker_with_store().0
    }
    fn worker_with_store() -> (Worker, Rc<Store>) {
        let store = Rc::new(Store::open(std::path::Path::new(":memory:")).unwrap());
        (
            Worker::new(
                Engine::new(Config::default(), "initial".into()).unwrap(),
                store.clone(),
                store.clone(),
            ),
            store,
        )
    }

    #[test]
    fn commands_enforce_auth_roles_and_session_boundaries() {
        let mut w = worker();
        assert!(w.handle(Command::History { offset: 0 }).is_err());
        assert!(w
            .handle(Command::Start {
                session_id: "initial".into()
            })
            .is_err());
        w.handle(Command::Login {
            login: "operator".into(),
            password: "operator".into(),
        })
        .unwrap();
        assert!(w.handle(Command::Users).is_err());
        assert!(w
            .handle(Command::SetRole {
                user_id: 1,
                role: "operator".into()
            })
            .is_err());
        let created = w
            .handle(Command::Create {
                config: Config::default(),
            })
            .unwrap();
        let id = created["sessionId"].as_str().unwrap().to_string();
        w.handle(Command::Start {
            session_id: id.clone(),
        })
        .unwrap();
        w.engine.tick();
        w.handle(Command::Pause {
            session_id: id.clone(),
        })
        .unwrap();
        let frozen = w.engine.time_ms;
        for _ in 0..100 {
            w.engine.tick();
        }
        assert_eq!(w.engine.time_ms, frozen);
        w.handle(Command::Resume {
            session_id: id.clone(),
        })
        .unwrap();
        let reset = w
            .handle(Command::Reset {
                session_id: id.clone(),
            })
            .unwrap();
        assert_ne!(reset["sessionId"], id);
        assert!(w.handle(Command::Pause { session_id: id }).is_err());
        assert_eq!(
            w.handle(Command::History { offset: 0 }).unwrap()["items"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        w.handle(Command::Logout).unwrap();
        assert!(w.handle(Command::History { offset: 0 }).is_err());
    }

    #[test]
    fn failed_save_blocks_every_destructive_transition_until_retry() {
        let (mut w, store) = worker_with_store();
        w.handle(Command::Login {
            login: "operator".into(),
            password: "operator".into(),
        })
        .unwrap();
        w.handle(Command::Start {
            session_id: "initial".into(),
        })
        .unwrap();
        store.fail_training_writes(true);
        assert!(w
            .handle(Command::Finish {
                session_id: "initial".into()
            })
            .is_err());
        for command in [
            Command::Create {
                config: Config::default(),
            },
            Command::Reset {
                session_id: "initial".into(),
            },
            Command::Logout,
            Command::PrepareClose,
        ] {
            assert!(w.handle(command).is_err());
            assert_eq!(w.engine.snapshot().session_id, "initial");
            assert!(w.user.is_some());
            assert!(w.save_error.is_some());
        }
        store.fail_training_writes(false);
        w.handle(Command::PrepareClose).unwrap();
        assert!(w.save_error.is_none());
        assert_eq!(
            w.store
                .history(w.user.as_ref().unwrap().id, 0)
                .unwrap()
                .total,
            1
        );
        w.handle(Command::Logout).unwrap();
        assert!(w.user.is_none());
    }

    #[test]
    fn pending_archive_blocks_transition_and_retries_without_losing_event() {
        let mut w = worker();
        w.handle(Command::Login {
            login: "operator".into(),
            password: "operator".into(),
        })
        .unwrap();
        w.handle(Command::Start {
            session_id: "initial".into(),
        })
        .unwrap();
        for _ in 0..400 {
            w.engine.tick();
        }
        let event = w.engine.snapshot().notifications.last().unwrap().clone();
        w.handle(Command::ReserveArchive {
            session_id: "initial".into(),
            event_id: event.id,
        })
        .unwrap();
        w.engine.finish();
        for command in [
            Command::Reset {
                session_id: "initial".into(),
            },
            Command::Create {
                config: Config::default(),
            },
            Command::Logout,
            Command::PrepareClose,
        ] {
            assert!(w.handle(command).is_err());
        }
        assert!(w
            .handle(Command::ArchiveImage {
                session_id: "initial".into(),
                event_id: event.id,
                data_url: "invalid".into()
            })
            .is_err());
        assert!(w.pending_archive.is_some());
        let png = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+a1XkAAAAASUVORK5CYII=";
        for _ in 0..2 {
            w.handle(Command::ArchiveImage {
                session_id: "initial".into(),
                event_id: event.id,
                data_url: png.into(),
            })
            .unwrap();
        }
        assert!(w.pending_archive.is_none());
        // Re-reserving an already archived event must not leave a stuck reservation.
        w.handle(Command::ReserveArchive {
            session_id: "initial".into(),
            event_id: event.id,
        })
        .unwrap();
        w.handle(Command::ArchiveImage {
            session_id: "initial".into(),
            event_id: event.id,
            data_url: png.into(),
        })
        .unwrap();
        assert!(w.pending_archive.is_none());
        w.handle(Command::Reset {
            session_id: "initial".into(),
        })
        .unwrap();
        assert_eq!(
            w.store
                .archives(w.user.as_ref().unwrap().id, 0)
                .unwrap()
                .total,
            1
        );
        assert_eq!(w.store.archives(999, 0).unwrap().total, 0);
    }

    #[test]
    fn normal_close_finishes_active_training_and_guest_close_is_safe() {
        let mut w = worker();
        w.handle(Command::PrepareClose).unwrap();
        w.handle(Command::Login {
            login: "operator".into(),
            password: "operator".into(),
        })
        .unwrap();
        w.handle(Command::Start {
            session_id: "initial".into(),
        })
        .unwrap();
        w.engine.tick();
        w.handle(Command::PrepareClose).unwrap();
        assert_eq!(w.engine.status, Status::Finished);
        assert!(w.saved);
    }

    #[test]
    fn command_contract_accepts_typescript_camel_case() {
        let command: Command =
            serde_json::from_value(json!({"type":"identify","sessionId":"abc","targetId":4}))
                .unwrap();
        assert!(matches!(command, Command::Identify { target_id: 4, .. }));
        let command: Command = serde_json::from_value(
            json!({"type":"changePassword","oldPassword":"old","newPassword":"new"}),
        )
        .unwrap();
        assert!(matches!(command, Command::ChangePassword { .. }));
    }
}
