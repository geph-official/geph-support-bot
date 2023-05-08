use anyhow::Context;
use isahc::{AsyncReadResponseExt, Request};
use serde_json::Value;

/// A client of the Telegram bot API.
pub struct TelegramBot {
    token: String,
    client: isahc::HttpClient,
}

impl TelegramBot {
    /// Creates a new TelegramBot.
    pub fn new(token: &str) -> Self {
        Self {
            token: token.into(),
            client: isahc::HttpClientBuilder::new()
                .max_connections(4)
                .build()
                .unwrap(),
        }
    }

    /// Calls a Telegram API.
    pub async fn call_api(&self, method: &str, args: Value) -> anyhow::Result<Value> {
        let raw_res: Value = self
            .client
            .send_async(
                Request::post(format!(
                    "https://api.telegram.org/bot{}/{method}",
                    self.token
                ))
                .header("Content-Type", "application/json")
                .body(serde_json::to_vec(&args)?)?,
            )
            .await?
            .json()
            .await?;
        if raw_res["ok"].as_bool().unwrap_or(false) {
            Ok(raw_res["result"].clone())
        } else {
            anyhow::bail!(
                "telegram failed with error code {}",
                raw_res["error_code"]
                    .as_i64()
                    .context("could not parse error code as integer")?
            )
        }
    }
}
