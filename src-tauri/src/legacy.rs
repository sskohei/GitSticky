use crate::errors::{AppError, Result};
use argon2::Argon2;
use std::{fs, path::PathBuf};
use tauri::{AppHandle, Manager};
use tauri_plugin_stronghold::stronghold::Stronghold;
use zeroize::Zeroizing;
fn err() -> AppError {
    AppError::new(
        "locked",
        "保管庫を開けません。パスフレーズと保存ファイルを確認してください。",
    )
}
fn paths(app: &AppHandle) -> Result<(PathBuf, PathBuf)> {
    let dir = app.path().app_config_dir().map_err(|_| err())?;
    fs::create_dir_all(&dir).map_err(|_| err())?;
    Ok((dir.join("token.stronghold"), dir.join("vault.salt")))
}
fn open(app: &AppHandle, passphrase: &str) -> Result<Stronghold> {
    if passphrase.chars().count() < 8 {
        return Err(AppError::new(
            "validation",
            "パスフレーズは8文字以上にしてください。",
        ));
    }
    let (path, salt_path) = paths(app)?;
    if !path.exists() || !salt_path.exists() {
        return Err(err());
    }
    let salt = fs::read(salt_path).map_err(|_| err())?;
    if salt.len() != 32 {
        return Err(err());
    }
    let mut key = Zeroizing::new(vec![0u8; 32]);
    Argon2::default()
        .hash_password_into(passphrase.as_bytes(), &salt, &mut key)
        .map_err(|_| err())?;
    Stronghold::new(path, key.to_vec()).map_err(|_| err())
}
pub fn unlock_storage(app: AppHandle, passphrase: String) -> Result<Zeroizing<String>> {
    let passphrase = Zeroizing::new(passphrase);
    let vault = open(&app, &passphrase)?;
    let client = if vault.get_client(b"github").is_ok() {
        vault.get_client(b"github").map_err(|_| err())?
    } else {
        // Existing snapshots must never be replaced on a load failure.
        if paths(&app)?.0.exists() {
            vault.load_client(b"github").map_err(|_| err())?;
            vault.get_client(b"github").map_err(|_| err())?
        } else {
            return Err(err());
        }
    };
    let raw = Zeroizing::new(
        client
            .store()
            .get(b"pat")
            .map_err(|_| err())?
            .ok_or_else(err)?,
    );
    let text = String::from_utf8(raw.to_vec()).map_err(|_| err())?;
    Ok(Zeroizing::new(text))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_reopens_and_rejects_a_wrong_key() {
        let dir =
            std::env::temp_dir().join(format!("stickynote-vault-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("test.stronghold");
        let mut key = vec![0u8; 32];
        Argon2::default()
            .hash_password_into(b"test passphrase", b"random-test-salt", &mut key)
            .unwrap();
        {
            let vault = Stronghold::new(&path, key.clone()).unwrap();
            let client = vault.create_client(b"github").unwrap();
            client
                .store()
                .insert(b"pat".to_vec(), b"test-token-only".to_vec(), None)
                .unwrap();
            vault.write_client(b"github").unwrap();
            vault.save().unwrap();
        }
        let bytes = fs::read(&path).unwrap();
        assert!(!bytes.windows(15).any(|w| w == b"test-token-only"));
        {
            let vault = Stronghold::new(&path, key).unwrap();
            vault.load_client(b"github").unwrap();
            let client = vault.get_client(b"github").unwrap();
            assert_eq!(
                client.store().get(b"pat").unwrap().unwrap(),
                b"test-token-only"
            );
        }
        assert!(Stronghold::new(&path, vec![9u8; 32]).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
}
