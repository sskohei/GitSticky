use crate::{
    auth::TokenSet,
    errors::{AppError, Result},
};
use async_trait::async_trait;
use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize)]
pub struct DeviceReply {
    pub device_code: String,
    pub user_code: String,
    pub expires_in: u64,
    pub interval: u64,
}
#[derive(Deserialize)]
pub struct TokenReply {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
    pub refresh_token_expires_in: u64,
}
impl TokenReply {
    pub fn tokens(self, client_id: &str, now: u64) -> Result<TokenSet> {
        if self.access_token.is_empty()
            || self.refresh_token.is_empty()
            || self.expires_in == 0
            || self.refresh_token_expires_in == 0
        {
            return Err(AppError::new(
                "auth",
                "認証の有効期限を確認できません。GitHubで接続し直してください。",
            ));
        }
        Ok(TokenSet {
            access_token: self.access_token,
            refresh_token: Some(self.refresh_token),
            expires_at: Some(now.saturating_add(self.expires_in)),
            refresh_expires_at: Some(now.saturating_add(self.refresh_token_expires_in)),
            client_id: client_id.into(),
            mode: "oauth".into(),
            must_reconnect: false,
        })
    }
}
pub enum PollReply {
    Pending,
    SlowDown,
    Authorized(TokenReply),
}
#[async_trait]
pub trait OAuthTransport: Send + Sync {
    async fn start(&self, client_id: &str) -> Result<DeviceReply>;
    async fn poll(&self, client_id: &str, code: &str) -> Result<PollReply>;
    async fn refresh(&self, client_id: &str, refresh_token: &str) -> Result<TokenReply>;
}
pub struct GitHubOAuth {
    client: reqwest::Client,
}
impl GitHubOAuth {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .connect_timeout(Duration::from_secs(10))
                .redirect(reqwest::redirect::Policy::none())
                .user_agent("StickyNote/0.1.0")
                .build()
                .map_err(|_| AppError::new("network", "認証通信を準備できません。"))?,
        })
    }
    async fn post(&self, path: &str, fields: &[(&str, &str)]) -> Result<serde_json::Value> {
        let response = self
            .client
            .post(format!("https://github.com/{path}"))
            .header("Accept", "application/json")
            .form(fields)
            .send()
            .await
            .map_err(|_| {
                AppError::new(
                    "network",
                    "GitHubへの接続に失敗しました。通信状態を確認して再試行してください。",
                )
            })?;
        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS
            || status == reqwest::StatusCode::FORBIDDEN
        {
            let seconds = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(60);
            return Err(AppError {
                code: "rate_limit",
                message: "認証の利用制限です。しばらく待って再試行してください。".into(),
                retry_at: Some(
                    crate::auth::now()
                        .saturating_add(seconds)
                        .saturating_mul(1000),
                ),
            });
        }
        if status.is_server_error() {
            return Err(AppError::new(
                "network",
                "GitHubの認証サーバーと通信できません。再試行してください。",
            ));
        }
        let body = response
            .bytes()
            .await
            .map_err(|_| AppError::new("network", "認証の応答を受信できません。"))?;
        if body.len() > 100_000 {
            return Err(AppError::new("auth", "認証の応答が不正です。"));
        }
        let value: serde_json::Value = serde_json::from_slice(&body)
            .map_err(|_| AppError::new("auth", "認証の応答を確認できません。"))?;
        if !status.is_success() {
            return Err(value["error"].as_str().map(oauth_error).unwrap_or_else(|| {
                AppError::new(
                    "auth",
                    "認証リクエストが拒否されました。接続し直してください。",
                )
            }));
        }
        Ok(value)
    }
}
pub fn parse_poll(value: serde_json::Value) -> Result<PollReply> {
    match value.get("error").and_then(serde_json::Value::as_str) {
        Some("authorization_pending") => Ok(PollReply::Pending),
        Some("slow_down") => Ok(PollReply::SlowDown),
        Some(code) => Err(oauth_error(code)),
        None => serde_json::from_value(value)
            .map(PollReply::Authorized)
            .map_err(|_| AppError::new("auth", "認証の応答が不正です。接続し直してください。")),
    }
}
fn oauth_error(code: &str) -> AppError {
    let message = match code {
        "access_denied" => "GitHubで接続が拒否されました。必要な場合はもう一度接続してください。",
        "expired_token" | "bad_verification_code" => {
            "接続コードの有効期限が切れました。もう一度接続してください。"
        }
        "bad_refresh_token" | "invalid_grant" => {
            "認証の有効期限が切れたか、権限が取り消されました。GitHubで接続し直してください。"
        }
        "device_flow_disabled" => {
            "GitHub AppのDevice Flowが無効です。開発者がApp設定を確認してください。"
        }
        "incorrect_client_credentials" => {
            "GitHub AppのClient IDが正しくありません。開発者が設定を確認してください。"
        }
        _ => "GitHubの認証を完了できませんでした。接続し直してください。",
    };
    AppError::new("auth", message)
}
#[async_trait]
impl OAuthTransport for GitHubOAuth {
    async fn start(&self, client_id: &str) -> Result<DeviceReply> {
        let value = self
            .post(
                "login/device/code",
                &[("client_id", client_id), ("scope", "offline_access")],
            )
            .await?;
        if let Some(e) = value["error"].as_str() {
            return Err(oauth_error(e));
        }
        serde_json::from_value(value)
            .map_err(|_| AppError::new("auth", "接続コードを取得できません。"))
    }
    async fn poll(&self, client_id: &str, code: &str) -> Result<PollReply> {
        parse_poll(
            self.post(
                "login/oauth/access_token",
                &[
                    ("client_id", client_id),
                    ("device_code", code),
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ],
            )
            .await?,
        )
    }
    async fn refresh(&self, client_id: &str, refresh_token: &str) -> Result<TokenReply> {
        let value = self
            .post(
                "login/oauth/access_token",
                &[
                    ("client_id", client_id),
                    ("refresh_token", refresh_token),
                    ("grant_type", "refresh_token"),
                ],
            )
            .await?;
        if let Some(e) = value["error"].as_str() {
            return Err(oauth_error(e));
        }
        serde_json::from_value(value).map_err(|_| {
            AppError::new(
                "auth",
                "更新した認証の応答が不正です。接続し直してください。",
            )
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pending_slow_down_denial_and_expiration() {
        assert!(matches!(
            parse_poll(serde_json::json!({"error":"authorization_pending"})).unwrap(),
            PollReply::Pending
        ));
        assert!(matches!(
            parse_poll(serde_json::json!({"error":"slow_down"})).unwrap(),
            PollReply::SlowDown
        ));
        for e in [
            "access_denied",
            "expired_token",
            "bad_verification_code",
            "bad_refresh_token",
            "device_flow_disabled",
        ] {
            assert_eq!(
                parse_poll(serde_json::json!({"error":e}))
                    .err()
                    .unwrap()
                    .code,
                "auth"
            );
        }
    }
    #[test]
    fn tokens_require_both_expirations() {
        assert!(TokenReply {
            access_token: "test".into(),
            refresh_token: "refresh".into(),
            expires_in: 0,
            refresh_token_expires_in: 100
        }
        .tokens("client", 1)
        .is_err());
    }
}
