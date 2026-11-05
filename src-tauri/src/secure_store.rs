use crate::errors::{AppError, Result};
use tauri::{AppHandle, Manager};
use zeroize::Zeroizing;

pub trait SecretStore: Send + Sync {
    fn load(&self) -> Result<Option<Zeroizing<Vec<u8>>>>;
    fn save(&self, plaintext: &[u8]) -> Result<()>;
    fn clear(&self) -> Result<()>;
}
pub fn storage_error() -> AppError {
    AppError::new(
        "secure_storage",
        "端末の認証情報を読み書きできません。メモは保持されています。再試行してください。",
    )
}
pub struct NativeStore(pub AppHandle);

#[cfg(windows)]
mod desktop {
    use super::*;
    use std::{fs, io::Write, path::Path};
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{
            CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
        },
        Storage::FileSystem::{MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH},
    };
    fn wide(path: &Path) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        path.as_os_str().encode_wide().chain(Some(0)).collect()
    }
    pub fn protect(bytes: &[u8], decrypt: bool) -> Result<Zeroizing<Vec<u8>>> {
        if bytes.len() > 1_000_000 {
            return Err(storage_error());
        }
        let input = CRYPT_INTEGER_BLOB {
            cbData: bytes.len() as u32,
            pbData: bytes.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };
        let success = unsafe {
            if decrypt {
                CryptUnprotectData(
                    &input,
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    std::ptr::null(),
                    std::ptr::null(),
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut output,
                )
            } else {
                CryptProtectData(
                    &input,
                    std::ptr::null(),
                    std::ptr::null(),
                    std::ptr::null(),
                    std::ptr::null(),
                    CRYPTPROTECT_UI_FORBIDDEN,
                    &mut output,
                )
            }
        };
        if success == 0 {
            return Err(storage_error());
        }
        let value = unsafe {
            Zeroizing::new(
                std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec(),
            )
        };
        unsafe {
            if decrypt {
                std::ptr::write_bytes(output.pbData, 0, output.cbData as usize);
            }
            LocalFree(output.pbData as _);
        }
        Ok(value)
    }
    pub fn save_file(path: &Path, plaintext: &[u8]) -> Result<()> {
        let encrypted = protect(plaintext, false)?;
        let tmp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| {
            let mut f = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&tmp)
                .map_err(|_| storage_error())?;
            f.write_all(&encrypted).map_err(|_| storage_error())?;
            f.sync_all().map_err(|_| storage_error())?;
            drop(f);
            if unsafe {
                MoveFileExW(
                    wide(&tmp).as_ptr(),
                    wide(path).as_ptr(),
                    MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
                )
            } == 0
            {
                return Err(storage_error());
            }
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(tmp);
        }
        result
    }
    impl NativeStore {
        fn path(&self) -> Result<std::path::PathBuf> {
            let dir = self
                .0
                .path()
                .app_config_dir()
                .map_err(|_| storage_error())?;
            fs::create_dir_all(&dir).map_err(|_| storage_error())?;
            Ok(dir.join("github-auth.dpapi"))
        }
    }
    impl SecretStore for NativeStore {
        fn load(&self) -> Result<Option<Zeroizing<Vec<u8>>>> {
            match fs::read(self.path()?) {
                Ok(bytes) => protect(&bytes, true).map(Some),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(_) => Err(storage_error()),
            }
        }
        fn save(&self, value: &[u8]) -> Result<()> {
            save_file(&self.path()?, value)
        }
        fn clear(&self) -> Result<()> {
            match fs::remove_file(self.path()?) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(_) => Err(storage_error()),
            }
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn dpapi_round_trip_and_tamper_rejection() {
            let secret = b"test-oauth-refresh-token";
            let encrypted = protect(secret, false).unwrap();
            assert!(!encrypted.windows(secret.len()).any(|w| w == secret));
            assert_eq!(&*protect(&encrypted, true).unwrap(), secret);
            let mut damaged = encrypted.to_vec();
            damaged[0] ^= 1;
            assert!(protect(&damaged, true).is_err());
        }
        #[test]
        fn atomic_replacement_keeps_only_ciphertext() {
            let path =
                std::env::temp_dir().join(format!("sticky-auth-test-{}.bin", uuid::Uuid::new_v4()));
            save_file(&path, b"first").unwrap();
            save_file(&path, b"second").unwrap();
            assert_eq!(
                &*protect(&fs::read(&path).unwrap(), true).unwrap(),
                b"second"
            );
            fs::remove_file(path).unwrap();
        }
    }
}

#[cfg(target_os = "android")]
pub struct AndroidBridge(pub tauri::plugin::PluginHandle<tauri::Wry>);
#[cfg(target_os = "android")]
impl SecretStore for NativeStore {
    fn load(&self) -> Result<Option<Zeroizing<Vec<u8>>>> {
        let value: serde_json::Value = self
            .0
            .state::<AndroidBridge>()
            .0
            .run_mobile_plugin("read", serde_json::json!({}))
            .map_err(|_| storage_error())?;
        Ok(value["value"]
            .as_str()
            .map(|v| Zeroizing::new(v.as_bytes().to_vec())))
    }
    fn save(&self, bytes: &[u8]) -> Result<()> {
        let value = std::str::from_utf8(bytes).map_err(|_| storage_error())?;
        self.0
            .state::<AndroidBridge>()
            .0
            .run_mobile_plugin::<serde_json::Value>("write", serde_json::json!({"value":value}))
            .map_err(|_| storage_error())?;
        Ok(())
    }
    fn clear(&self) -> Result<()> {
        self.0
            .state::<AndroidBridge>()
            .0
            .run_mobile_plugin::<serde_json::Value>("clear", serde_json::json!({}))
            .map_err(|_| storage_error())?;
        Ok(())
    }
}
#[cfg(not(any(windows, target_os = "android")))]
impl SecretStore for NativeStore {
    fn load(&self) -> Result<Option<Zeroizing<Vec<u8>>>> {
        Err(storage_error())
    }
    fn save(&self, _: &[u8]) -> Result<()> {
        Err(storage_error())
    }
    fn clear(&self) -> Result<()> {
        Err(storage_error())
    }
}

pub fn open_github(app: &AppHandle, url: &str) -> Result<()> {
    let parsed =
        reqwest::Url::parse(url).map_err(|_| AppError::new("validation", "URLが不正です。"))?;
    if parsed.scheme() != "https" || parsed.host_str() != Some("github.com") {
        return Err(AppError::new("validation", "GitHub以外のURLは開けません。"));
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};
        let value: Vec<u16> = url.encode_utf16().chain(Some(0)).collect();
        if unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                std::ptr::null(),
                value.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            )
        } as isize
            <= 32
        {
            return Err(AppError::new(
                "validation",
                "ブラウザを開けません。表示されたURLを手動で開いてください。",
            ));
        }
        let _ = app;
        Ok(())
    }
    #[cfg(target_os = "android")]
    {
        app.state::<AndroidBridge>()
            .0
            .run_mobile_plugin::<serde_json::Value>("openGithub", serde_json::json!({"url":url}))
            .map_err(|_| AppError::new("validation", "ブラウザを開けません。"))?;
        Ok(())
    }
    #[cfg(not(any(windows, target_os = "android")))]
    {
        let _ = app;
        Err(AppError::new("validation", "このOSには対応していません。"))
    }
}
