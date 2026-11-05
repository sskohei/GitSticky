use serde::Serialize;
#[derive(Debug, Serialize)]
pub struct AppError {
    pub code: &'static str,
    pub message: String,
    pub retry_at: Option<u64>,
}
impl AppError {
    pub fn new(code: &'static str, message: &str) -> Self {
        Self {
            code,
            message: message.into(),
            retry_at: None,
        }
    }
}
impl From<sqlx::Error> for AppError {
    fn from(_: sqlx::Error) -> Self {
        Self::new(
            "database",
            "ローカル保存に失敗しました。入力を残したまま再試行してください。",
        )
    }
}
pub type Result<T> = std::result::Result<T, AppError>;
