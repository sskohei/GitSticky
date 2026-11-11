use crate::{
    auth::Credentials,
    crypto::{decrypt_sync, SyncKeys},
    errors::{AppError, Result},
    models::Settings,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use reqwest::{Client, Method, Response, StatusCode, Url};
use serde::Serialize;
#[cfg(test)]
use serde_json::json;
use serde_json::Value;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::State;
fn client() -> Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("StickyNote/0.1.0")
        .build()
        .map_err(|_| AppError::new("network", "通信を準備できません。"))
}
pub(crate) fn url(settings: &Settings, parts: &[&str]) -> Result<Url> {
    if settings.repository_owner.is_empty()
        || settings.repository_name.is_empty()
        || settings.branch_name.is_empty()
        || settings.repository_owner.contains('/')
        || settings.repository_name.contains('/')
    {
        return Err(AppError::new(
            "validation",
            "owner、repository、branchを確認してください。",
        ));
    }
    let mut u = Url::parse("https://api.github.com/repos/").expect("static URL");
    {
        let mut p = u
            .path_segments_mut()
            .map_err(|_| AppError::new("validation", "接続先が不正です。"))?;
        p.pop_if_empty()
            .push(&settings.repository_owner)
            .push(&settings.repository_name);
        for part in parts {
            p.push(part);
        }
    }
    Ok(u)
}
fn status_error(
    status: StatusCode,
    headers: &reqwest::header::HeaderMap,
    missing: &'static str,
) -> AppError {
    let rate = status == StatusCode::TOO_MANY_REQUESTS
        || (status == StatusCode::FORBIDDEN
            && headers
                .get("x-ratelimit-remaining")
                .and_then(|v| v.to_str().ok())
                == Some("0"))
        || headers.contains_key("retry-after");
    if rate {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let after = headers
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .map(|v| now + v);
        let reset = headers
            .get("x-ratelimit-reset")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok());
        return AppError {
            code: "rate_limit",
            message: "GitHub APIの利用制限です。解除後に再試行してください。".into(),
            retry_at: Some(after.unwrap_or(0).max(reset.unwrap_or(0)).max(now + 60) * 1000),
        };
    }
    let (code, msg) = match status.as_u16() {
        401 => (
            "auth",
            "認証が無効、または期限切れです。GitHubで接続し直してください。",
        ),
        403 => (
            "permission",
            "権限が不足しています。Contents: Read and writeを確認してください。",
        ),
        404 => (
            missing,
            "対象が見つかりません。接続先とGitHub Appに許可したリポジトリを確認してください。",
        ),
        409 | 422 => (
            "conflict",
            "GitHub側の変更と競合しました。再取得して再判定します。",
        ),
        _ => (
            "network",
            "GitHubとの通信に失敗しました。ローカルのメモは保持されています。",
        ),
    };
    AppError::new(code, msg)
}
pub(crate) async fn request(
    credentials: &Credentials,
    method: Method,
    url: Url,
    body: Option<Value>,
    missing: &'static str,
) -> Result<Response> {
    #[cfg(test)]
    let url = if let Some(base) = &credentials.api_base {
        base.join(url.path().trim_start_matches('/'))
            .map(|mut target| {
                target.set_query(url.query());
                target
            })
            .map_err(|_| AppError::new("network", "Invalid test URL"))?
    } else {
        url
    };
    let mut token = credentials.access_token(None).await?;
    let mut refreshed = false;
    let c = client()?;
    let is_get = method == Method::GET;
    for attempt in 0..3 {
        let mut r = c
            .request(method.clone(), url.clone())
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2026-03-10")
            .bearer_auth(token.as_str());
        if let Some(ref b) = body {
            r = r.json(b);
        }
        match r.send().await {
            Ok(response) => {
                if response.status().is_success() {
                    return Ok(response);
                }
                if response.status() == StatusCode::UNAUTHORIZED && !refreshed {
                    token = credentials.access_token(Some(token.as_str())).await?;
                    refreshed = true;
                    continue;
                }
                if response.status() == StatusCode::UNAUTHORIZED {
                    credentials.reject(&token).await;
                }
                if is_get && response.status().is_server_error() && attempt < 2 {
                    tokio::time::sleep(Duration::from_millis(400 * (1 << attempt))).await;
                    continue;
                }
                return Err(status_error(response.status(), response.headers(), missing));
            }
            Err(_) => {
                if is_get && attempt < 2 {
                    tokio::time::sleep(Duration::from_millis(400 * (1 << attempt))).await;
                    continue;
                }
                return Err(AppError::new(
                    "network",
                    "ネットワークに接続できません。ローカルの変更は保持されています。",
                ));
            }
        }
    }
    Err(AppError::new(
        "network",
        "通信を再試行しましたが失敗しました。",
    ))
}
async fn bounded_bytes(mut response: Response) -> Result<Vec<u8>> {
    const LIMIT: usize = 30_000_000;
    if response.content_length().is_some_and(|n| n > LIMIT as u64) {
        return Err(AppError::new("validation", "GitHubの応答が大きすぎます。"));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| AppError::new("network", "GitHubの応答を読み取れません。"))?
    {
        if chunk.len() > LIMIT - bytes.len() {
            return Err(AppError::new("validation", "GitHubの応答が大きすぎます。"));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}
pub(crate) async fn value(r: Response) -> Result<Value> {
    serde_json::from_slice(&bounded_bytes(r).await?)
        .map_err(|_| AppError::new("network", "GitHubの応答を読み取れません。"))
}
fn require_private(repo: &Value) -> Result<()> {
    if repo["private"].as_bool() != Some(true) {
        return Err(AppError::new(
            "permission",
            "安全のため、非公開リポジトリだけに同期できます。GitHubで公開範囲を確認してください。",
        ));
    }
    Ok(())
}
pub(crate) async fn private_repository(
    settings: &Settings,
    credentials: &Credentials,
) -> Result<Value> {
    let repo = value(
        request(
            credentials,
            Method::GET,
            url(settings, &[])?,
            None,
            "repository",
        )
        .await?,
    )
    .await?;
    require_private(&repo)?;
    Ok(repo)
}
#[tauri::command]
pub async fn github_test(settings: Settings, credentials: State<'_, Credentials>) -> Result<()> {
    let repo = private_repository(&settings, &credentials).await?;
    if repo.pointer("/permissions/push").and_then(Value::as_bool) == Some(false) {
        return Err(AppError::new(
            "permission",
            "リポジトリの書き込み権限がありません。",
        ));
    }
    request(
        &credentials,
        Method::GET,
        url(&settings, &["branches", &settings.branch_name])?,
        None,
        "branch",
    )
    .await?;
    Ok(())
}
#[derive(Serialize)]
pub struct RemoteFile {
    pub id: String,
    pub sha: String,
}
#[tauri::command]
pub async fn github_list(
    settings: Settings,
    credentials: State<'_, Credentials>,
    keys: State<'_, SyncKeys>,
) -> Result<Vec<RemoteFile>> {
    crate::repository::list(&settings, &credentials, &keys).await
}
async fn list_files(settings: &Settings, credentials: &Credentials) -> Result<Vec<RemoteFile>> {
    Ok(crate::repository::raw_snapshot(settings, credentials)
        .await?
        .files
        .into_iter()
        .map(|(id, sha)| RemoteFile { id, sha })
        .collect())
}

pub(crate) fn managed_id(path: &str) -> Option<String> {
    let id = path.strip_prefix("notes/")?.strip_suffix(".md")?;
    if id.len() != 36 || uuid::Uuid::parse_str(id).is_err() {
        return None;
    }
    Some(id.to_string())
}
#[tauri::command]
pub async fn github_read(
    settings: Settings,
    id: String,
    sha: String,
    credentials: State<'_, Credentials>,
    keys: State<'_, SyncKeys>,
) -> Result<ReadNote> {
    note_url(&settings, &id)?;
    crate::repository::read(&settings, &id, &sha, &credentials, &keys).await
}

pub(crate) async fn read_blob(
    settings: &Settings,
    sha: &str,
    credentials: &Credentials,
) -> Result<String> {
    if !sha.chars().all(|c| c.is_ascii_hexdigit()) || sha.len() != 40 {
        return Err(AppError::new("validation", "SHAが不正です。"));
    }
    let v = value(
        request(
            credentials,
            Method::GET,
            url(settings, &["git", "blobs", sha])?,
            None,
            "conflict",
        )
        .await?,
    )
    .await?;
    if v["encoding"] != "base64" || v["size"].as_u64().unwrap_or(u64::MAX) > 9_000_000 {
        return Err(AppError::new(
            "locked",
            "メモのエンコードまたはサイズが不正です。",
        ));
    }
    let content = v["content"]
        .as_str()
        .ok_or_else(|| AppError::new("locked", "本文がありません。"))?;
    let bytes = STANDARD
        .decode(content.replace(['\n', '\r'], ""))
        .map_err(|_| AppError::new("locked", "本文を復号できません。"))?;
    let content = String::from_utf8(bytes)
        .map_err(|_| AppError::new("locked", "メモがUTF-8ではありません。"))?;
    Ok(content)
}
pub async fn validate_sync_key(
    settings: &Settings,
    key: &crate::crypto::NoteCipher,
    credentials: &Credentials,
    require_encrypted: bool,
) -> Result<bool> {
    private_repository(settings, credentials).await?;
    let mut has_legacy = false;
    for file in list_files(settings, credentials).await? {
        let content = read_blob(settings, &file.sha, credentials).await?;
        let (_, encrypted) = decrypt_sync(key, settings, &file.id, &content)?;
        has_legacy |= !encrypted;
        if require_encrypted && !encrypted {
            return Err(AppError::new(
                "locked",
                "平文メモが残っているため、暗号化の移行を完了できません。",
            ));
        }
    }
    Ok(has_legacy)
}
#[derive(Serialize)]
pub struct ReadNote {
    pub content: String,
    pub encrypted: bool,
}
pub(crate) fn note_url(s: &Settings, id: &str) -> Result<Url> {
    if id.len() != 36 || uuid::Uuid::parse_str(id).is_err() {
        return Err(AppError::new("validation", "メモIDが不正です。"));
    }
    url(s, &["contents", "notes", &format!("{id}.md")])
}

#[derive(Serialize)]
pub struct RepositoryOption {
    pub id: u64,
    pub owner: String,
    pub name: String,
    pub default_branch: String,
    pub private: bool,
}
fn api_url(path: &str, page: u32) -> Result<Url> {
    let mut u = Url::parse(&format!("https://api.github.com/{path}"))
        .map_err(|_| AppError::new("validation", "接続先が不正です。"))?;
    u.query_pairs_mut()
        .append_pair("per_page", "100")
        .append_pair("page", &page.to_string());
    Ok(u)
}
fn repo_option(v: &Value) -> Result<RepositoryOption> {
    Ok(RepositoryOption {
        id: v["id"]
            .as_u64()
            .ok_or_else(|| AppError::new("network", "リポジトリ一覧の応答が不正です。"))?,
        owner: v
            .pointer("/owner/login")
            .and_then(Value::as_str)
            .ok_or_else(|| AppError::new("network", "所有者を取得できません。"))?
            .into(),
        name: v["name"]
            .as_str()
            .ok_or_else(|| AppError::new("network", "リポジトリ名を取得できません。"))?
            .into(),
        default_branch: v["default_branch"].as_str().unwrap_or("main").into(),
        private: v["private"].as_bool() == Some(true),
    })
}
#[tauri::command]
pub async fn github_repositories(
    credentials: State<'_, Credentials>,
) -> Result<Vec<RepositoryOption>> {
    let modern = credentials.status(false).await?.mode.as_deref() == Some("oauth");
    let mut repositories = Vec::new();
    if modern {
        let mut installations = Vec::new();
        for page in 1..=100 {
            let v = value(
                request(
                    &credentials,
                    Method::GET,
                    api_url("user/installations", page)?,
                    None,
                    "permission",
                )
                .await?,
            )
            .await?;
            let rows = v["installations"]
                .as_array()
                .ok_or_else(|| AppError::new("network", "GitHub Appの認可先を取得できません。"))?;
            for row in rows {
                installations.push(
                    row["id"]
                        .as_u64()
                        .ok_or_else(|| AppError::new("network", "認可先の応答が不正です。"))?,
                );
            }
            if rows.len() < 100 {
                break;
            }
            if page == 100 {
                return Err(AppError::new(
                    "validation",
                    "認可先が多すぎるため一覧を取得できません。",
                ));
            }
        }
        for installation in installations {
            for page in 1..=100 {
                let v = value(
                    request(
                        &credentials,
                        Method::GET,
                        api_url(
                            &format!("user/installations/{installation}/repositories"),
                            page,
                        )?,
                        None,
                        "permission",
                    )
                    .await?,
                )
                .await?;
                let rows = v["repositories"].as_array().ok_or_else(|| {
                    AppError::new("network", "許可されたリポジトリを取得できません。")
                })?;
                for row in rows {
                    repositories.push(repo_option(row)?);
                }
                if rows.len() < 100 {
                    break;
                }
                if page == 100 {
                    return Err(AppError::new(
                        "validation",
                        "リポジトリが多すぎるため一覧を取得できません。",
                    ));
                }
            }
        }
    } else {
        for page in 1..=100 {
            let v = value(
                request(
                    &credentials,
                    Method::GET,
                    api_url("user/repos", page)?,
                    None,
                    "permission",
                )
                .await?,
            )
            .await?;
            let rows = v
                .as_array()
                .ok_or_else(|| AppError::new("network", "リポジトリ一覧を取得できません。"))?;
            for row in rows {
                repositories.push(repo_option(row)?);
            }
            if rows.len() < 100 {
                break;
            }
            if page == 100 {
                return Err(AppError::new(
                    "validation",
                    "リポジトリが多すぎるため一覧を取得できません。",
                ));
            }
        }
    }
    repositories.retain(|repo| repo.private);
    repositories.sort_by(|a, b| {
        a.owner
            .to_lowercase()
            .cmp(&b.owner.to_lowercase())
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    repositories.dedup_by_key(|r| r.id);
    Ok(repositories)
}
#[tauri::command]
pub async fn github_branches(
    settings: Settings,
    credentials: State<'_, Credentials>,
) -> Result<Vec<String>> {
    let mut branches = Vec::new();
    for page in 1..=100 {
        let mut u = url(&settings, &["branches"])?;
        u.query_pairs_mut()
            .append_pair("per_page", "100")
            .append_pair("page", &page.to_string());
        let v = value(request(&credentials, Method::GET, u, None, "repository").await?).await?;
        let rows = v
            .as_array()
            .ok_or_else(|| AppError::new("network", "ブランチ一覧を取得できません。"))?;
        for row in rows {
            branches.push(
                row["name"]
                    .as_str()
                    .ok_or_else(|| AppError::new("network", "ブランチ名を取得できません。"))?
                    .into(),
            );
        }
        if rows.len() < 100 {
            return Ok(branches);
        }
        if page == 100 {
            return Err(AppError::new(
                "validation",
                "ブランチが多すぎるため一覧を取得できません。",
            ));
        }
    }
    Ok(branches)
}
#[tauri::command]
pub async fn github_write(
    settings: Settings,
    id: String,
    content: String,
    sha: Option<String>,
    credentials: State<'_, Credentials>,
    keys: State<'_, SyncKeys>,
) -> Result<String> {
    crate::repository::write(
        &settings,
        &id,
        Some(&content),
        sha.as_deref(),
        &credentials,
        &keys,
    )
    .await?
    .ok_or_else(crate::sync_security::integrity_error)
}
#[tauri::command]
pub async fn github_remove(
    settings: Settings,
    id: String,
    sha: String,
    credentials: State<'_, Credentials>,
    keys: State<'_, SyncKeys>,
) -> Result<()> {
    crate::repository::write(&settings, &id, None, Some(&sha), &credentials, &keys).await?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_and_unknown_visibility_are_rejected() {
        assert!(require_private(&json!({"private":true})).is_ok());
        for repo in [
            json!({"private":false}),
            json!({}),
            json!({"private":"true"}),
        ] {
            assert!(require_private(&repo).is_err());
        }
    }
    #[test]
    fn errors_are_typed() {
        for (status, code) in [
            (401, "auth"),
            (403, "permission"),
            (404, "repository"),
            (409, "conflict"),
            (422, "conflict"),
            (429, "rate_limit"),
        ] {
            assert_eq!(
                status_error(
                    StatusCode::from_u16(status).unwrap(),
                    &Default::default(),
                    "repository"
                )
                .code,
                code
            );
        }
    }
    #[test]
    fn rate_limit_reset_is_respected() {
        let mut h = reqwest::header::HeaderMap::new();
        h.insert("x-ratelimit-reset", "9999999999".parse().unwrap());
        assert_eq!(
            status_error(StatusCode::TOO_MANY_REQUESTS, &h, "repository").retry_at,
            Some(9999999999000)
        );
    }
    #[test]
    fn only_uuid_notes_are_managed() {
        assert!(managed_id("README.md").is_none());
        assert!(managed_id("notes/test.md").is_none());
        assert!(managed_id("notes/../README.md").is_none());
        assert!(managed_id("notes/550e8400-e29b-41d4-a716-446655440000.md").is_some());
    }
}
