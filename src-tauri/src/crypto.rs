use crate::{
    errors::{AppError, Result},
    models::Settings,
    secure_store::SecretStore,
};
use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};
use tauri::State;
use tokio::sync::Mutex;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

// Local JSON can expand control characters well beyond the Markdown byte size.
const MAX_PLAINTEXT: usize = 64_000_000;
const MAX_SYNC_PLAINTEXT: usize = 5_000_000;
#[derive(Clone)]
pub struct NoteCipher(Zeroizing<[u8; 32]>);
fn error() -> AppError {
    AppError::new("locked", "メモを復号できません。暗号鍵を確認してください。")
}
impl NoteCipher {
    pub fn new(key: [u8; 32]) -> Self {
        Self(Zeroizing::new(key))
    }
    pub fn random() -> Self {
        let mut key = [0; 32];
        OsRng.fill_bytes(&mut key);
        Self::new(key)
    }
    pub fn from_encoded(key: &str) -> Result<Self> {
        if key.trim().len() != 44 {
            return Err(error());
        }
        let bytes = Zeroizing::new(STANDARD.decode(key.trim()).map_err(|_| error())?);
        Ok(Self::new(bytes.as_slice().try_into().map_err(|_| error())?))
    }
    pub fn encoded(&self) -> Zeroizing<String> {
        Zeroizing::new(STANDARD.encode(self.0.as_slice()))
    }
    pub fn seal(&self, plaintext: &[u8], context: &str) -> Result<String> {
        if plaintext.len() > MAX_PLAINTEXT {
            return Err(AppError::new("validation", "メモが大きすぎます。"));
        }
        let cipher = Aes256Gcm::new_from_slice(self.0.as_slice()).map_err(|_| error())?;
        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut nonce);
        let encrypted = cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: plaintext,
                    aad: context.as_bytes(),
                },
            )
            .map_err(|_| error())?;
        let mut bytes = nonce.to_vec();
        bytes.extend(encrypted);
        Ok(STANDARD.encode(bytes))
    }
    pub fn open(&self, value: &str, context: &str) -> Result<Zeroizing<Vec<u8>>> {
        if value.len() > (MAX_PLAINTEXT + 28).div_ceil(3) * 4 {
            return Err(error());
        }
        let bytes = STANDARD.decode(value).map_err(|_| error())?;
        if bytes.len() < 28 {
            return Err(error());
        }
        let cipher = Aes256Gcm::new_from_slice(self.0.as_slice()).map_err(|_| error())?;
        cipher
            .decrypt(
                Nonce::from_slice(&bytes[..12]),
                Payload {
                    msg: &bytes[12..],
                    aad: context.as_bytes(),
                },
            )
            .map(Zeroizing::new)
            .map_err(|_| error())
    }
}

pub fn connection(settings: &Settings) -> String {
    serde_json::json!([
        settings.repository_owner.to_lowercase(),
        settings.repository_name.to_lowercase(),
        settings.branch_name
    ])
    .to_string()
}
#[derive(Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
struct SyncKeyRecord {
    key: String,
    legacy_allowed: bool,
}
#[derive(Default, Serialize, Deserialize)]
#[serde(transparent)]
struct KeyMap(BTreeMap<String, SyncKeyRecord>);
pub struct SyncKeys {
    store: Arc<dyn SecretStore>,
    lock: Mutex<()>,
}
impl SyncKeys {
    pub fn new(store: Arc<dyn SecretStore>) -> Self {
        Self {
            store,
            lock: Mutex::new(()),
        }
    }
    fn load(&self) -> Result<KeyMap> {
        match self.store.load()? {
            Some(bytes) => serde_json::from_slice(&bytes).map_err(|_| error()),
            None => Ok(KeyMap::default()),
        }
    }
    pub async fn key(&self, settings: &Settings) -> Result<NoteCipher> {
        let _guard = self.lock.lock().await;
        let keys = self.load()?;
        let key = keys.0.get(&connection(settings)).ok_or_else(|| {
            AppError::new(
                "locked",
                "設定画面で、この同期先の暗号鍵を登録してください。他の端末と同じ鍵が必要です。",
            )
        })?;
        NoteCipher::from_encoded(&key.key)
    }
    pub async fn decrypt(
        &self,
        settings: &Settings,
        id: &str,
        content: &str,
    ) -> Result<(String, bool)> {
        let _guard = self.lock.lock().await;
        let keys = self.load()?;
        let record = keys.0.get(&connection(settings)).ok_or_else(error)?;
        let key = NoteCipher::from_encoded(&record.key)?;
        let result = decrypt_sync(&key, settings, id, content)?;
        if !result.1 && !record.legacy_allowed {
            return Err(AppError::new(
                "locked",
                "暗号化済みの同期先で平文メモを検出しました。安全のため同期を中断します。",
            ));
        }
        Ok(result)
    }
    async fn finish_migration(&self, settings: &Settings) -> Result<()> {
        let _guard = self.lock.lock().await;
        let mut keys = self.load()?;
        let record = keys.0.get_mut(&connection(settings)).ok_or_else(error)?;
        record.legacy_allowed = false;
        let raw = Zeroizing::new(serde_json::to_vec(&keys).map_err(|_| error())?);
        self.store.save(&raw)
    }
    async fn legacy_allowed(&self, settings: &Settings) -> Result<bool> {
        let _guard = self.lock.lock().await;
        let keys = self.load()?;
        Ok(keys
            .0
            .get(&connection(settings))
            .ok_or_else(error)?
            .legacy_allowed)
    }
    async fn save(
        &self,
        settings: &Settings,
        value: Zeroizing<String>,
        legacy_allowed: bool,
    ) -> Result<()> {
        if settings.repository_owner.is_empty()
            || settings.repository_name.is_empty()
            || settings.branch_name.is_empty()
        {
            return Err(error());
        }
        let key = NoteCipher::from_encoded(&value)?;
        let _guard = self.lock.lock().await;
        let mut keys = self.load()?;
        let cid = connection(settings);
        let encoded = key.encoded();
        match keys.0.get_mut(&cid) {
            Some(record) if record.key == encoded.as_str() => {
                // Re-registering the same key must never reopen a completed migration.
                record.legacy_allowed &= legacy_allowed;
            }
            _ => {
                keys.0.insert(
                    cid,
                    SyncKeyRecord {
                        key: encoded.to_string(),
                        legacy_allowed,
                    },
                );
            }
        }
        let raw = Zeroizing::new(serde_json::to_vec(&keys).map_err(|_| error())?);
        self.store.save(&raw)
    }
}
#[derive(Serialize)]
pub struct SecurityStatus {
    sync_ready: bool,
}
#[tauri::command]
pub async fn security_status(
    settings: Settings,
    keys: State<'_, SyncKeys>,
) -> Result<SecurityStatus> {
    let _guard = keys.lock.lock().await;
    let map = keys.load()?;
    let sync_ready = match map.0.get(&connection(&settings)) {
        Some(key) => {
            NoteCipher::from_encoded(&key.key)?;
            true
        }
        None => false,
    };
    Ok(SecurityStatus { sync_ready })
}
#[tauri::command]
pub fn security_generate_key() -> String {
    NoteCipher::random().encoded().to_string()
}
#[tauri::command]
pub async fn security_set_sync_key(
    settings: Settings,
    key: String,
    keys: State<'_, SyncKeys>,
    credentials: State<'_, crate::auth::Credentials>,
) -> Result<()> {
    let key = Zeroizing::new(key);
    let candidate = NoteCipher::from_encoded(&key)?;
    let legacy_allowed =
        crate::github::validate_sync_key(&settings, &candidate, &credentials, false).await?;
    keys.save(&settings, key, legacy_allowed).await
}

#[tauri::command]
pub async fn security_finish_migration(
    settings: Settings,
    keys: State<'_, SyncKeys>,
    credentials: State<'_, crate::auth::Credentials>,
) -> Result<()> {
    if !keys.legacy_allowed(&settings).await? {
        return Ok(());
    }
    let key = keys.key(&settings).await?;
    crate::github::validate_sync_key(&settings, &key, &credentials, true).await?;
    keys.finish_migration(&settings).await
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    #[serde(rename = "schemaVersion")]
    version: u32,
    format: String,
    data: String,
}
fn sync_context(settings: &Settings, id: &str) -> String {
    format!("GitSticky:sync:v1:{}:{id}", connection(settings))
}
pub fn encrypt_sync(
    key: &NoteCipher,
    settings: &Settings,
    id: &str,
    content: &str,
) -> Result<String> {
    if content.len() > MAX_SYNC_PLAINTEXT {
        return Err(AppError::new(
            "validation",
            "同期するメモは5MB以下にしてください。",
        ));
    }
    serde_json::to_string(&Envelope {
        version: 3,
        format: "gitsticky-aes256gcm-v1".into(),
        data: key.seal(content.as_bytes(), &sync_context(settings, id))?,
    })
    .map_err(|_| error())
}
pub fn decrypt_sync(
    key: &NoteCipher,
    settings: &Settings,
    id: &str,
    content: &str,
) -> Result<(String, bool)> {
    // Existing Markdown is read only to migrate it; every subsequent write is encrypted.
    if content.trim_start_matches('\u{feff}').starts_with("---") {
        return Ok((content.into(), false));
    }
    let envelope: Envelope = serde_json::from_str(content).map_err(|_| error())?;
    if envelope.version != 3 || envelope.format != "gitsticky-aes256gcm-v1" {
        return Err(error());
    }
    let raw = key.open(&envelope.data, &sync_context(settings, id))?;
    if raw.len() > MAX_SYNC_PLAINTEXT {
        return Err(error());
    }
    let text = String::from_utf8(raw.to_vec()).map_err(|_| error())?;
    Ok((text, true))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Mutex as StdMutex,
    };
    #[derive(Default)]
    struct Store {
        value: StdMutex<Option<Vec<u8>>>,
        fail: AtomicBool,
    }
    impl SecretStore for Store {
        fn load(&self) -> Result<Option<Zeroizing<Vec<u8>>>> {
            Ok(self.value.lock().unwrap().clone().map(Zeroizing::new))
        }
        fn save(&self, value: &[u8]) -> Result<()> {
            if self.fail.load(Ordering::SeqCst) {
                return Err(error());
            }
            *self.value.lock().unwrap() = Some(value.to_vec());
            Ok(())
        }
        fn clear(&self) -> Result<()> {
            *self.value.lock().unwrap() = None;
            Ok(())
        }
    }
    #[tokio::test]
    async fn keys_persist_independently_and_completed_migration_rejects_plaintext_downgrades() {
        let store = Arc::new(Store::default());
        let keys = SyncKeys::new(store.clone());
        let settings = Settings {
            repository_owner: "owner".into(),
            repository_name: "notes".into(),
            ..Default::default()
        };
        assert!(keys.key(&settings).await.is_err());
        let key = NoteCipher::random();
        store.fail.store(true, Ordering::SeqCst);
        assert!(keys.save(&settings, key.encoded(), true).await.is_err());
        assert!(keys.key(&settings).await.is_err());
        store.fail.store(false, Ordering::SeqCst);
        keys.save(&settings, key.encoded(), true).await.unwrap();
        let encrypted = encrypt_sync(&key, &settings, "id", "---\nlegacy").unwrap();
        assert!(keys.decrypt(&settings, "id", "---\nlegacy").await.is_ok());
        keys.finish_migration(&settings).await.unwrap();
        let restored = SyncKeys::new(store.clone());
        assert!(!restored.legacy_allowed(&settings).await.unwrap());
        assert!(restored.decrypt(&settings, "id", &encrypted).await.is_ok());
        assert!(restored
            .decrypt(&settings, "id", "---\nlegacy")
            .await
            .is_err());
        assert!(restored
            .save(&settings, Zeroizing::new("invalid".into()), true)
            .await
            .is_err());
        restored.save(&settings, key.encoded(), true).await.unwrap();
        assert!(!restored.legacy_allowed(&settings).await.unwrap());
        let other = Settings {
            repository_name: "other".into(),
            ..settings
        };
        assert!(restored.key(&other).await.is_err());
    }
    #[test]
    fn authenticated_encryption_rejects_tampering_wrong_key_and_swapped_note() {
        let key = NoteCipher::random();
        let encrypted = key.seal("秘密の本文".as_bytes(), "note-1").unwrap();
        assert_eq!(
            &*key.open(&encrypted, "note-1").unwrap(),
            "秘密の本文".as_bytes()
        );
        assert!(key.open(&encrypted, "note-2").is_err());
        assert!(NoteCipher::random().open(&encrypted, "note-1").is_err());
        let mut bytes = STANDARD.decode(&encrypted).unwrap();
        bytes[12] ^= 1;
        assert!(key.open(&STANDARD.encode(bytes), "note-1").is_err());
        assert_ne!(
            encrypted,
            key.seal("秘密の本文".as_bytes(), "note-1").unwrap()
        );
    }
    #[test]
    fn sync_round_trip_between_devices_binds_repository_branch_and_note() {
        let key = NoteCipher::random();
        let device = NoteCipher::from_encoded(&key.encoded()).unwrap();
        let settings = Settings {
            repository_owner: "Owner".into(),
            repository_name: "Notes".into(),
            ..Default::default()
        };
        let content = "---\ntitle: 秘密\n---\n本文";
        let encrypted = encrypt_sync(&key, &settings, "note-1", content).unwrap();
        assert!(!encrypted.contains("秘密"));
        assert_eq!(
            decrypt_sync(&device, &settings, "note-1", &encrypted).unwrap(),
            (content.into(), true)
        );
        assert!(decrypt_sync(&device, &settings, "note-2", &encrypted).is_err());
        let other = Settings {
            branch_name: "other".into(),
            ..settings.clone()
        };
        assert!(decrypt_sync(&device, &other, "note-1", &encrypted).is_err());
        assert_eq!(
            decrypt_sync(&device, &settings, "note-1", content)
                .unwrap()
                .1,
            false
        );
        assert!(decrypt_sync(&device, &settings, "note-1", "{broken").is_err());
    }
}
