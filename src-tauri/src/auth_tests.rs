use super::*;
use crate::oauth::{DeviceReply, TokenReply};
use async_trait::async_trait;
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Mutex as StdMutex,
    },
};

#[derive(Default)]
struct MemoryStore {
    value: StdMutex<Option<Vec<u8>>>,
    fail_save: AtomicBool,
    fail_clear: AtomicBool,
}
impl SecretStore for MemoryStore {
    fn load(&self) -> Result<Option<Zeroizing<Vec<u8>>>> {
        Ok(self.value.lock().unwrap().clone().map(Zeroizing::new))
    }
    fn save(&self, value: &[u8]) -> Result<()> {
        if self.fail_save.load(Ordering::SeqCst) {
            return Err(crate::secure_store::storage_error());
        }
        *self.value.lock().unwrap() = Some(value.to_vec());
        Ok(())
    }
    fn clear(&self) -> Result<()> {
        if self.fail_clear.load(Ordering::SeqCst) {
            return Err(crate::secure_store::storage_error());
        }
        *self.value.lock().unwrap() = None;
        Ok(())
    }
}
#[derive(Default)]
struct FakeOAuth {
    starts: AtomicUsize,
    polls: AtomicUsize,
    refreshes: AtomicUsize,
    replies: StdMutex<VecDeque<PollReply>>,
    refresh_error: StdMutex<Option<&'static str>>,
    poll_started: tokio::sync::Notify,
    poll_gate: StdMutex<Option<Arc<tokio::sync::Notify>>>,
}
fn reply() -> TokenReply {
    TokenReply {
        access_token: "test-new-access".into(),
        refresh_token: "test-new-refresh".into(),
        expires_in: 28800,
        refresh_token_expires_in: 15897600,
    }
}
#[async_trait]
impl OAuthTransport for FakeOAuth {
    async fn start(&self, _: &str) -> Result<DeviceReply> {
        self.starts.fetch_add(1, Ordering::SeqCst);
        Ok(DeviceReply {
            device_code: "private-device-secret".into(),
            user_code: "ABCD-EFGH".into(),
            expires_in: 900,
            interval: 5,
        })
    }
    async fn poll(&self, _: &str, _: &str) -> Result<PollReply> {
        self.polls.fetch_add(1, Ordering::SeqCst);
        let gate = self.poll_gate.lock().unwrap().clone();
        self.poll_started.notify_one();
        if let Some(gate) = gate {
            gate.notified().await;
        }
        Ok(self
            .replies
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(PollReply::Pending))
    }
    async fn refresh(&self, _: &str, _: &str) -> Result<TokenReply> {
        self.refreshes.fetch_add(1, Ordering::SeqCst);
        if let Some(code) = self.refresh_error.lock().unwrap().take() {
            return Err(AppError::new(code, "test refresh error"));
        }
        Ok(reply())
    }
}
fn config() -> AppConfig {
    AppConfig {
        client_id: "Iv1.test-client".into(),
        slug: "sticky-note-test".into(),
    }
}
fn tokens(expired: bool) -> TokenSet {
    TokenSet {
        access_token: "test-old-access".into(),
        refresh_token: Some("test-old-refresh".into()),
        expires_at: Some(if expired { now() - 1 } else { now() + 3600 }),
        refresh_expires_at: Some(now() + 100000),
        client_id: config().client_id,
        mode: "oauth".into(),
        must_reconnect: false,
    }
}
fn fixture(initial: Option<TokenSet>) -> (Arc<Credentials>, Arc<MemoryStore>, Arc<FakeOAuth>) {
    let store = Arc::new(MemoryStore::default());
    if let Some(t) = initial {
        store.save(&serde_json::to_vec(&t).unwrap()).unwrap();
    }
    let transport = Arc::new(FakeOAuth::default());
    let auth = Arc::new(Credentials::new(config(), store.clone(), transport.clone()));
    (auth, store, transport)
}
async fn ready(auth: &Credentials) {
    auth.inner.lock().await.pending.as_mut().unwrap().next_poll = Instant::now();
}
#[tokio::test]
async fn startup_restores_without_a_passphrase_or_returning_tokens() {
    let (auth, _, _) = fixture(Some(tokens(false)));
    let status = auth.status(false).await.unwrap();
    assert!(status.connected);
    let json = serde_json::to_string(&status).unwrap();
    assert!(!json.contains("test-old-access"));
    assert!(!json.contains("refresh_token"));
    assert_eq!(&*auth.access_token(None).await.unwrap(), "test-old-access");
}
#[tokio::test]
async fn initial_device_response_does_not_expose_the_device_secret() {
    let (auth, _, _) = fixture(None);
    let session = auth.begin().await.unwrap();
    let json = serde_json::to_string(&session).unwrap();
    assert!(json.contains("ABCD-EFGH"));
    assert!(!json.contains("private-device-secret"));
    assert!(!json.contains("device_code"));
    assert_eq!(session.verification_uri, "https://github.com/login/device");
}
#[tokio::test]
async fn polling_enforces_interval_and_adds_five_seconds_on_slow_down() {
    let (auth, _, http) = fixture(None);
    http.replies.lock().unwrap().push_back(PollReply::SlowDown);
    let session = auth.begin().await.unwrap();
    assert!(!auth.poll(&session.session_id).await.unwrap().connected);
    assert_eq!(http.polls.load(Ordering::SeqCst), 0);
    ready(&auth).await;
    assert_eq!(
        auth.poll(&session.session_id).await.unwrap().retry_after_ms,
        10000
    );
    auth.poll(&session.session_id).await.unwrap();
    assert_eq!(http.polls.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn successful_device_login_is_saved_and_restored() {
    let (auth, store, http) = fixture(None);
    http.replies
        .lock()
        .unwrap()
        .push_back(PollReply::Authorized(reply()));
    let session = auth.begin().await.unwrap();
    ready(&auth).await;
    assert!(auth.poll(&session.session_id).await.unwrap().connected);
    let restored = Credentials::new(config(), store, http);
    assert!(restored.status(false).await.unwrap().connected);
    assert_eq!(
        &*restored.access_token(None).await.unwrap(),
        "test-new-access"
    );
}
#[tokio::test]
async fn canceling_an_inflight_poll_cannot_connect_afterward() {
    let (auth, store, http) = fixture(None);
    let gate = Arc::new(tokio::sync::Notify::new());
    *http.poll_gate.lock().unwrap() = Some(gate.clone());
    http.replies
        .lock()
        .unwrap()
        .push_back(PollReply::Authorized(reply()));
    let session = auth.begin().await.unwrap();
    ready(&auth).await;
    let worker = auth.clone();
    let id = session.session_id.clone();
    let task = tokio::spawn(async move { worker.poll(&id).await });
    http.poll_started.notified().await;
    auth.cancel(Some(&session.session_id)).await;
    gate.notify_one();
    assert_eq!(task.await.unwrap().err().unwrap().code, "auth_cancelled");
    assert!(store.load().unwrap().is_none());
}
#[tokio::test]
async fn cancellation_of_an_old_session_does_not_cancel_a_new_one() {
    let (auth, _, _) = fixture(None);
    let old = auth.begin().await.unwrap();
    let current = auth.begin().await.unwrap();
    auth.cancel(Some(&old.session_id)).await;
    assert_eq!(
        auth.inner.lock().await.pending.as_ref().unwrap().session_id,
        current.session_id
    );
    assert_eq!(
        auth.poll(&old.session_id).await.err().unwrap().code,
        "auth_cancelled"
    );
}
#[tokio::test]
async fn expired_device_codes_are_not_sent() {
    let (auth, _, http) = fixture(None);
    let session = auth.begin().await.unwrap();
    auth.inner.lock().await.pending.as_mut().unwrap().deadline =
        Instant::now() - Duration::from_secs(1);
    assert_eq!(
        auth.poll(&session.session_id).await.err().unwrap().code,
        "auth"
    );
    assert_eq!(http.polls.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn refresh_rotates_both_tokens_and_persists_before_return() {
    let (auth, store, http) = fixture(Some(tokens(true)));
    assert_eq!(&*auth.access_token(None).await.unwrap(), "test-new-access");
    assert_eq!(http.refreshes.load(Ordering::SeqCst), 1);
    let saved: TokenSet = serde_json::from_slice(&store.load().unwrap().unwrap()).unwrap();
    assert_eq!(saved.refresh_token.as_deref(), Some("test-new-refresh"));
}
#[tokio::test]
async fn simultaneous_requests_refresh_once() {
    let (auth, _, http) = fixture(Some(tokens(true)));
    let (a, b, c) = tokio::join!(
        auth.access_token(None),
        auth.access_token(None),
        auth.access_token(None)
    );
    assert!(a.is_ok() && b.is_ok() && c.is_ok());
    assert_eq!(http.refreshes.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn concurrent_401_responses_do_not_rotate_the_new_refresh_token_twice() {
    let (auth, _, http) = fixture(Some(tokens(false)));
    let (a, b) = tokio::join!(
        auth.access_token(Some("test-old-access")),
        auth.access_token(Some("test-old-access"))
    );
    assert!(a.is_ok() && b.is_ok());
    assert_eq!(http.refreshes.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn network_failures_keep_credentials_for_retry() {
    let (auth, store, http) = fixture(Some(tokens(true)));
    *http.refresh_error.lock().unwrap() = Some("network");
    assert_eq!(auth.access_token(None).await.err().unwrap().code, "network");
    assert!(auth.status(false).await.unwrap().connected);
    assert!(store.load().unwrap().is_some());
    assert!(auth.access_token(None).await.is_ok());
}
#[tokio::test]
async fn expired_refresh_token_requires_reconnection_without_erasing_credentials() {
    let mut t = tokens(true);
    t.refresh_expires_at = Some(now() - 1);
    let (auth, store, http) = fixture(Some(t));
    assert!(auth.status(false).await.unwrap().needs_reconnect);
    assert_eq!(auth.access_token(None).await.err().unwrap().code, "auth");
    assert_eq!(http.refreshes.load(Ordering::SeqCst), 0);
    assert!(store.load().unwrap().is_some());
}
#[tokio::test]
async fn revoked_tokens_remain_marked_after_restart() {
    let (auth, store, http) = fixture(Some(tokens(true)));
    *http.refresh_error.lock().unwrap() = Some("auth");
    assert!(auth.access_token(None).await.is_err());
    let restored = Credentials::new(config(), store, http);
    assert!(restored.status(false).await.unwrap().needs_reconnect);
}
#[tokio::test]
async fn save_failure_retains_rotated_credentials_in_memory_for_a_safe_retry() {
    let (auth, store, http) = fixture(Some(tokens(true)));
    store.fail_save.store(true, Ordering::SeqCst);
    assert_eq!(
        auth.access_token(None).await.err().unwrap().code,
        "secure_storage"
    );
    store.fail_save.store(false, Ordering::SeqCst);
    assert_eq!(&*auth.access_token(None).await.unwrap(), "test-new-access");
    assert_eq!(http.refreshes.load(Ordering::SeqCst), 1);
    let saved: TokenSet = serde_json::from_slice(&store.load().unwrap().unwrap()).unwrap();
    assert_eq!(saved.refresh_token.as_deref(), Some("test-new-refresh"));
}
#[tokio::test]
async fn device_login_save_failure_can_be_retried_without_reusing_the_device_code() {
    let (auth, store, http) = fixture(None);
    http.replies
        .lock()
        .unwrap()
        .push_back(PollReply::Authorized(reply()));
    let session = auth.begin().await.unwrap();
    ready(&auth).await;
    store.fail_save.store(true, Ordering::SeqCst);
    assert_eq!(
        auth.poll(&session.session_id).await.err().unwrap().code,
        "secure_storage"
    );
    store.fail_save.store(false, Ordering::SeqCst);
    assert!(auth.status(false).await.unwrap().connected);
    assert_eq!(http.polls.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn disconnect_clears_only_authentication_and_cancels_pending_login() {
    let (auth, store, _) = fixture(Some(tokens(false)));
    auth.begin().await.unwrap();
    auth.disconnect().await.unwrap();
    assert!(!auth.status(false).await.unwrap().connected);
    assert!(store.load().unwrap().is_none());
    assert!(auth.inner.lock().await.pending.is_none());
}
#[tokio::test]
async fn failed_disconnect_does_not_claim_to_have_deleted_authentication() {
    let (auth, store, _) = fixture(Some(tokens(false)));
    store.fail_clear.store(true, Ordering::SeqCst);
    assert!(auth.disconnect().await.is_err());
    assert!(auth.status(false).await.unwrap().connected);
}
#[tokio::test]
async fn a_different_app_configuration_never_uses_old_tokens() {
    let (_, store, http) = fixture(Some(tokens(false)));
    let mut app = config();
    app.client_id = "Iv1.another-client".into();
    let auth = Credentials::new(app, store, http);
    assert!(auth.status(false).await.unwrap().needs_reconnect);
    assert_eq!(auth.access_token(None).await.err().unwrap().code, "auth");
}
#[tokio::test]
async fn unconfigured_builds_do_not_make_fake_login_requests() {
    let store = Arc::new(MemoryStore::default());
    let http = Arc::new(FakeOAuth::default());
    let auth = Credentials::new(
        AppConfig {
            client_id: String::new(),
            slug: String::new(),
        },
        store,
        http.clone(),
    );
    assert_eq!(auth.begin().await.err().unwrap().code, "auth_config");
    assert_eq!(http.starts.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn legacy_import_is_one_time_and_does_not_need_a_passphrase_after_restart() {
    let (auth, store, http) = fixture(None);
    auth.import_legacy(Zeroizing::new("test-legacy-token".into()))
        .await
        .unwrap();
    let restored = Credentials::new(config(), store, http);
    assert_eq!(
        &*restored.access_token(None).await.unwrap(),
        "test-legacy-token"
    );
    assert!(restored
        .import_legacy(Zeroizing::new("other-token".into()))
        .await
        .is_err());
}
