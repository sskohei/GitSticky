//! The encrypted inventory authenticates additions, replacements and deletions.
//! A device-local high-water mark detects replay of a previously observed state.
use crate::{
    crypto::{connection, NoteCipher},
    errors::{AppError, Result},
    models::Settings,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use zeroize::{Zeroize, Zeroizing};

pub const MANIFEST_PATH: &str = ".gitsticky-security.json";
pub const MAX_NOTES: usize = 10_000;
#[derive(Clone, Debug, Serialize, Deserialize, Zeroize)]
pub struct Checkpoint {
    pub generation: u64,
    pub sha: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inventory {
    pub generation: u64,
    pub notes: BTreeMap<String, String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    #[serde(rename = "schemaVersion")]
    version: u32,
    format: String,
    data: String,
}
pub fn integrity_error() -> AppError {
    AppError::new("locked", "同期先の巻き戻し、無断変更、または削除を検出しました。ローカルのメモを保持して同期を中断しました。")
}
pub fn verify_checkpoint(known: Option<&Checkpoint>, remote: Option<&Checkpoint>) -> Result<()> {
    match (known, remote) {
        (Some(_), None) => Err(integrity_error()),
        (Some(k), Some(r))
            if r.generation < k.generation || (r.generation == k.generation && r.sha != k.sha) =>
        {
            Err(integrity_error())
        }
        _ => Ok(()),
    }
}
pub fn valid_sha(sha: &str) -> bool {
    sha.len() == 40 && sha.bytes().all(|c| c.is_ascii_hexdigit())
}
impl Inventory {
    pub fn validate(&self, files: &BTreeMap<String, String>) -> Result<()> {
        if self.generation == 0
            || self.notes.len() > MAX_NOTES
            || &self.notes != files
            || self.notes.iter().any(|(id, sha)| {
                id.len() != 36 || uuid::Uuid::parse_str(id).is_err() || !valid_sha(sha)
            })
        {
            return Err(integrity_error());
        }
        Ok(())
    }
    pub fn seal(&self, key: &NoteCipher, settings: &Settings) -> Result<String> {
        let raw = Zeroizing::new(serde_json::to_vec(self).map_err(|_| integrity_error())?);
        serde_json::to_string(&Envelope {
            version: 1,
            format: "gitsticky-inventory-aes256gcm-v1".into(),
            data: key.seal(
                &raw,
                &format!("GitSticky:inventory:v1:{}", connection(settings)),
            )?,
        })
        .map_err(|_| integrity_error())
    }
    pub fn open(content: &str, key: &NoteCipher, settings: &Settings) -> Result<Self> {
        if content.len() > 2_000_000 {
            return Err(integrity_error());
        }
        let envelope: Envelope = serde_json::from_str(content).map_err(|_| integrity_error())?;
        if envelope.version != 1 || envelope.format != "gitsticky-inventory-aes256gcm-v1" {
            return Err(integrity_error());
        }
        let raw = key.open(
            &envelope.data,
            &format!("GitSticky:inventory:v1:{}", connection(settings)),
        )?;
        serde_json::from_slice(&raw).map_err(|_| integrity_error())
    }
    pub fn advance(&mut self) -> Result<()> {
        self.generation = self.generation.checked_add(1).ok_or_else(integrity_error)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_replay_forks_missing_manifest_and_inventory_tampering() {
        let checkpoint = Checkpoint {
            generation: 3,
            sha: "a".repeat(40),
        };
        assert!(verify_checkpoint(Some(&checkpoint), None).is_err());
        assert!(verify_checkpoint(
            Some(&checkpoint),
            Some(&Checkpoint {
                generation: 2,
                sha: "b".repeat(40)
            })
        )
        .is_err());
        assert!(verify_checkpoint(
            Some(&checkpoint),
            Some(&Checkpoint {
                generation: 3,
                sha: "b".repeat(40)
            })
        )
        .is_err());
        assert!(verify_checkpoint(Some(&checkpoint), Some(&checkpoint)).is_ok());
        assert!(verify_checkpoint(
            Some(&checkpoint),
            Some(&Checkpoint {
                generation: 4,
                sha: "b".repeat(40)
            })
        )
        .is_ok());
        let id = uuid::Uuid::new_v4().to_string();
        let inventory = Inventory {
            generation: 3,
            notes: BTreeMap::from([(id.clone(), "a".repeat(40))]),
        };
        assert!(inventory.validate(&inventory.notes).is_ok());
        assert!(inventory.validate(&BTreeMap::new()).is_err());
        assert!(inventory
            .validate(&BTreeMap::from([(id, "b".repeat(40))]))
            .is_err());
    }
    #[test]
    fn authenticates_inventory_key_connection_and_generation() {
        let key = NoteCipher::random();
        let settings = Settings::default();
        let inventory = Inventory {
            generation: 1,
            notes: BTreeMap::new(),
        };
        let encrypted = inventory.seal(&key, &settings).unwrap();
        assert_eq!(
            Inventory::open(&encrypted, &key, &settings)
                .unwrap()
                .generation,
            1
        );
        assert!(Inventory::open(&encrypted, &NoteCipher::random(), &settings).is_err());
        assert!(Inventory::open(
            &encrypted,
            &key,
            &Settings {
                branch_name: "other".into(),
                ..settings
            }
        )
        .is_err());
    }
}
