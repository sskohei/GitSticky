use crate::{
    errors::{AppError, Result},
    oauth::{OAuthTransport, PollReply},
    secure_store::SecretStore,
};
use serde::{Deserialize, Serialize};
use std::{
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager, State};
use tokio::sync::Mutex;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};
#[cfg(test)]
#[path = "auth_tests.rs"]
mod tests;
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn reconnect() -> AppError {
    AppError::new(
        "auth",
        "GitHubで接続し直してください。ローカルのメモは保持されています。",
    )
}
fn canceled() -> AppError {
    AppError::new("auth_cancelled", "接続をキャンセルしました。")
}
#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct TokenSet {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<u64>,
    pub refresh_expires_at: Option<u64>,
    pub client_id: String,
    pub mode: String,
    pub must_reconnect: bool,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct AppConfig {
    pub client_id: String,
    pub slug: String,
}
impl AppConfig {
    pub fn bundled() -> Self {
        let mut value: Self = serde_json::from_str(include_str!("../github-app.json"))
            .expect("valid public GitHub App configuration");
        if let Some(id) = option_env!("STICKYNOTE_GITHUB_CLIENT_ID") {
            value.client_id = id.into();
        }
        if let Some(slug) = option_env!("STICKYNOTE_GITHUB_APP_SLUG") {
            value.slug = slug.into();
        }
        value
    }
    pub fn configured(&self) -> bool {
        self.client_id.len() >= 8
            && self.client_id.len() < 200
            && self
                .client_id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
            && !self.slug.is_empty()
            && self.slug.len() < 100
            && self
                .slug
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-')
    }
    fn install_url(&self) -> Option<String> {
        self.configured()
            .then(|| format!("https://github.com/apps/{}/installations/new", self.slug))
    }
}
struct Pending {
    session_id: String,
    device_code: Zeroizing<String>,
    deadline: Instant,
    interval: Duration,
    next_poll: Instant,
    polling: bool,
}
#[derive(Default)]
struct AuthState {
    loaded: bool,
    tokens: Option<TokenSet>,
    unsaved: bool,
    pending: Option<Pending>,
    starting: Option<String>,
}
pub struct Credentials {
    inner: Mutex<AuthState>,
    store: Arc<dyn SecretStore>,
    transport: Arc<dyn OAuthTransport>,
    pub config: AppConfig,
}
#[derive(Serialize)]
pub struct AuthStatus {
    pub connected: bool,
    pub mode: Option<String>,
    pub configured: bool,
    pub needs_reconnect: bool,
    pub expires_at: Option<u64>,
    pub install_url: Option<String>,
    pub legacy_available: bool,
}
#[derive(Serialize)]
pub struct DeviceSession {
    pub session_id: String,
    pub user_code: String,
    pub verification_uri: &'static str,
    pub expires_at: u64,
    pub interval_ms: u64,
}
#[derive(Serialize)]
pub struct PollStatus {
    pub connected: bool,
    pub retry_after_ms: u64,
}
impl Credentials {
    pub fn new(
        config: AppConfig,
        store: Arc<dyn SecretStore>,
        transport: Arc<dyn OAuthTransport>,
    ) -> Self {
        Self {
            inner: Mutex::new(AuthState::default()),
            config,
            store,
            transport,
        }
    }
    fn load(&self, state: &mut AuthState) -> Result<()> {
        if !state.loaded {
            state.tokens = match self.store.load()? {
                Some(bytes) => Some(
                    serde_json::from_slice::<TokenSet>(&bytes)
                        .map_err(|_| crate::secure_store::storage_error())?,
                ),
                None => None,
            };
            state.loaded = true;
        }
        self.save_pending(state)
    }
    fn save_pending(&self, state: &mut AuthState) -> Result<()> {
        if state.unsaved {
            if let Some(tokens) = &state.tokens {
                let raw = Zeroizing::new(
                    serde_json::to_vec(tokens).map_err(|_| crate::secure_store::storage_error())?,
                );
                self.store.save(&raw)?;
            }
            state.unsaved = false;
        }
        Ok(())
    }
    fn commit(&self, state: &mut AuthState, tokens: TokenSet) -> Result<()> {
        state.loaded = true;
        state.tokens = Some(tokens);
        state.unsaved = true;
        self.save_pending(state)
    }
    fn usable(&self, t: &TokenSet) -> bool {
        !t.must_reconnect
            && (t.mode == "legacy_pat"
                || (t.mode == "oauth"
                    && t.client_id == self.config.client_id
                    && t.refresh_expires_at.unwrap_or(0) > now()))
    }
    pub async fn status(&self, legacy_available: bool) -> Result<AuthStatus> {
        let mut state = self.inner.lock().await;
        self.load(&mut state)?;
        let connected = state.tokens.as_ref().is_some_and(|t| self.usable(t));
        Ok(AuthStatus {
            connected,
            mode: state.tokens.as_ref().map(|t| t.mode.clone()),
            configured: self.config.configured(),
            needs_reconnect: state.tokens.is_some() && !connected,
            expires_at: state.tokens.as_ref().and_then(|t| t.expires_at),
            install_url: self.config.install_url(),
            legacy_available,
        })
    }
    pub async fn access_token(&self, rejected: Option<&str>) -> Result<Zeroizing<String>> {
        let mut state = self.inner.lock().await;
        self.load(&mut state)?;
        let tokens = state.tokens.as_ref().ok_or_else(reconnect)?;
        if !self.usable(tokens) {
            return Err(reconnect());
        }
        let force = rejected.is_some_and(|v| v == tokens.access_token);
        if tokens.mode == "legacy_pat" {
            if force {
                if let Some(t) = state.tokens.as_mut() {
                    t.must_reconnect = true;
                }
                state.unsaved = true;
                let _ = self.save_pending(&mut state);
                return Err(reconnect());
            }
            return Ok(Zeroizing::new(tokens.access_token.clone()));
        }
        if force || tokens.expires_at.unwrap_or(0) <= now().saturating_add(60) {
            let refresh = Zeroizing::new(tokens.refresh_token.clone().ok_or_else(reconnect)?);
            let result = self.transport.refresh(&tokens.client_id, &refresh).await;
            match result {
                Ok(reply) => {
                    let next = reply.tokens(&self.config.client_id, now())?;
                    self.commit(&mut state, next)?;
                }
                Err(error) => {
                    if error.code == "auth" {
                        if let Some(t) = state.tokens.as_mut() {
                            t.must_reconnect = true;
                        }
                        state.unsaved = true;
                        let _ = self.save_pending(&mut state);
                    }
                    return Err(error);
                }
            }
        }
        Ok(Zeroizing::new(
            state
                .tokens
                .as_ref()
                .ok_or_else(reconnect)?
                .access_token
                .clone(),
        ))
    }
    pub async fn begin(&self) -> Result<DeviceSession> {
        if !self.config.configured() {
            return Err(AppError::new(
                "auth_config",
                "GitHub Appが未設定です。開発者が公開Client IDとApp名を設定してください。",
            ));
        }
        let id = uuid::Uuid::new_v4().to_string();
        {
            let mut s = self.inner.lock().await;
            s.pending = None;
            s.starting = Some(id.clone());
        }
        let response = self.transport.start(&self.config.client_id).await;
        let mut s = self.inner.lock().await;
        if s.starting.as_ref() != Some(&id) {
            return Err(canceled());
        }
        s.starting = None;
        let response = response?;
        if response.device_code.is_empty()
            || response.user_code.is_empty()
            || response.expires_in == 0
            || response.expires_in > 3600
            || response.interval == 0
            || response.interval > 300
        {
            return Err(AppError::new("auth", "接続コードの応答が不正です。"));
        }
        let interval = Duration::from_secs(response.interval);
        let started = Instant::now();
        let expires = now() + response.expires_in;
        s.pending = Some(Pending {
            session_id: id.clone(),
            device_code: Zeroizing::new(response.device_code),
            deadline: started + Duration::from_secs(response.expires_in),
            interval,
            next_poll: started + interval,
            polling: false,
        });
        Ok(DeviceSession {
            session_id: id,
            user_code: response.user_code,
            verification_uri: "https://github.com/login/device",
            expires_at: expires,
            interval_ms: interval.as_millis() as u64,
        })
    }
    pub async fn cancel(&self, id: Option<&str>) {
        let mut s = self.inner.lock().await;
        if id.is_none()
            || s.starting.as_deref() == id
            || s.pending
                .as_ref()
                .is_some_and(|p| Some(p.session_id.as_str()) == id)
        {
            s.pending = None;
            s.starting = None;
        }
    }
    pub async fn poll(&self, id: &str) -> Result<PollStatus> {
        let code = {
            let mut s = self.inner.lock().await;
            let p = s
                .pending
                .as_mut()
                .filter(|p| p.session_id == id)
                .ok_or_else(canceled)?;
            if Instant::now() >= p.deadline {
                s.pending = None;
                return Err(AppError::new(
                    "auth",
                    "接続コードの期限が切れました。もう一度接続してください。",
                ));
            }
            if p.polling || Instant::now() < p.next_poll {
                return Ok(PollStatus {
                    connected: false,
                    retry_after_ms: p
                        .next_poll
                        .saturating_duration_since(Instant::now())
                        .as_millis()
                        .max(1000) as u64,
                });
            }
            p.polling = true;
            p.next_poll = Instant::now() + p.interval;
            p.device_code.clone()
        };
        let result = self.transport.poll(&self.config.client_id, &code).await;
        let mut s = self.inner.lock().await;
        let p = s
            .pending
            .as_mut()
            .filter(|p| p.session_id == id)
            .ok_or_else(canceled)?;
        p.polling = false;
        if Instant::now() >= p.deadline {
            s.pending = None;
            return Err(AppError::new("auth", "接続コードの期限が切れました。"));
        }
        match result {
            Ok(PollReply::Pending) => Ok(PollStatus {
                connected: false,
                retry_after_ms: p.interval.as_millis() as u64,
            }),
            Ok(PollReply::SlowDown) => {
                p.interval += Duration::from_secs(5);
                p.next_poll = Instant::now() + p.interval;
                Ok(PollStatus {
                    connected: false,
                    retry_after_ms: p.interval.as_millis() as u64,
                })
            }
            Ok(PollReply::Authorized(reply)) => {
                let tokens = reply.tokens(&self.config.client_id, now())?;
                s.pending = None;
                self.commit(&mut s, tokens)?;
                Ok(PollStatus {
                    connected: true,
                    retry_after_ms: 0,
                })
            }
            Err(error) => {
                if error.code != "network" && error.code != "rate_limit" {
                    s.pending = None;
                } else if let Some(at) = error.retry_at {
                    let delay = Duration::from_millis(at.saturating_sub(now() * 1000));
                    if let Some(p) = s.pending.as_mut() {
                        p.next_poll = Instant::now() + delay;
                    }
                }
                Err(error)
            }
        }
    }
    pub async fn disconnect(&self) -> Result<()> {
        let mut s = self.inner.lock().await;
        self.store.clear()?;
        *s = AuthState {
            loaded: true,
            ..Default::default()
        };
        Ok(())
    }
    pub async fn reject(&self, token: &str) {
        let mut s = self.inner.lock().await;
        if let Some(t) = s.tokens.as_mut() {
            if t.access_token == token {
                t.must_reconnect = true;
                s.unsaved = true;
                let _ = self.save_pending(&mut s);
            }
        }
    }
    pub async fn import_legacy(&self, token: Zeroizing<String>) -> Result<()> {
        let mut s = self.inner.lock().await;
        self.load(&mut s)?;
        if s.tokens.is_some() {
            return Err(AppError::new(
                "validation",
                "接続済みです。以前の認証へ変更する場合は先に切断してください。",
            ));
        }
        self.commit(
            &mut s,
            TokenSet {
                access_token: token.to_string(),
                refresh_token: None,
                expires_at: None,
                refresh_expires_at: None,
                client_id: String::new(),
                mode: "legacy_pat".into(),
                must_reconnect: false,
            },
        )
    }
}
#[tauri::command]
pub async fn auth_status(
    app: AppHandle,
    credentials: State<'_, Credentials>,
) -> Result<AuthStatus> {
    let legacy = app
        .path()
        .app_config_dir()
        .map(|p| p.join("token.stronghold").exists())
        .unwrap_or(false);
    credentials.status(legacy).await
}
#[tauri::command]
pub async fn auth_begin(credentials: State<'_, Credentials>) -> Result<DeviceSession> {
    credentials.begin().await
}
#[tauri::command]
pub async fn auth_poll(
    credentials: State<'_, Credentials>,
    session_id: String,
) -> Result<PollStatus> {
    credentials.poll(&session_id).await
}
#[tauri::command]
pub async fn auth_cancel(
    credentials: State<'_, Credentials>,
    session_id: Option<String>,
) -> Result<()> {
    credentials.cancel(session_id.as_deref()).await;
    Ok(())
}
#[tauri::command]
pub async fn auth_disconnect(credentials: State<'_, Credentials>) -> Result<()> {
    credentials.disconnect().await
}
#[tauri::command]
pub async fn auth_open_browser(
    app: AppHandle,
    credentials: State<'_, Credentials>,
    target: String,
) -> Result<()> {
    let url = match target.as_str() {
        "device" => "https://github.com/login/device".into(),
        "install" => credentials
            .config
            .install_url()
            .ok_or_else(|| AppError::new("auth_config", "GitHub Appが未設定です。"))?,
        _ => return Err(AppError::new("validation", "このURLは開けません。")),
    };
    tauri::async_runtime::spawn_blocking(move || crate::secure_store::open_github(&app, &url))
        .await
        .map_err(|_| AppError::new("validation", "ブラウザを開けません。"))?
}
#[tauri::command]
pub async fn auth_import_legacy(
    app: AppHandle,
    credentials: State<'_, Credentials>,
    passphrase: String,
) -> Result<()> {
    let token = tauri::async_runtime::spawn_blocking(move || {
        crate::legacy::unlock_storage(app, passphrase)
    })
    .await
    .map_err(|_| reconnect())??;
    credentials.import_legacy(token).await
}
