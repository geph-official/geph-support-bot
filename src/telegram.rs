use std::time::Duration;

use anyhow::Context;
use async_trait::async_trait;
use isahc::{AsyncReadResponseExt, HttpClient, Request};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use smol::Task;
use smol_timeout::TimeoutExt;

use crate::{
    platform::{Platform, PlatformMsg},
    TelegramConfig,
};

pub struct Telegram {
    token: String,
    client: isahc::HttpClient,
    _task: Task<()>,
    recv_msgs: smol::channel::Receiver<PlatformMsg>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramThread {
    pub chat_id: i64,
    pub user_id: i64,
}

impl Telegram {
    pub fn new(config: &TelegramConfig) -> Self {
        let config = config.clone();
        let client = isahc::HttpClientBuilder::new()
            .max_connections(4)
            .build()
            .unwrap();
        let (send_msgs, recv_msgs) = smol::channel::unbounded();
        let token = config.telegram_token.clone();
        let http_client = client.clone();

        let _task = smol::spawn(async move {
            let mut counter = 0;
            loop {
                log::info!("getting updates at {counter}");
                let fallible = async {
                    let updates = call_api(
                        "getUpdates",
                        json!({"timeout": 120, "offset": counter + 1, "allowed_updates": []}),
                        &http_client,
                        &config.telegram_token,
                    )
                    .await
                    .context("cannot call telegram for updates")?;
                    let updates: Vec<Value> = serde_json::from_value(updates)?;
                    for update in updates {
                        // we only support text msgs atm
                        counter = counter.max(update["update_id"].as_i64().unwrap_or_default());
                        if !update["message"]["text"].is_null() {
                            let msg = update["message"]["text"]
                                .as_str()
                                .context("cannot parse out text")?;
                            log::info!("msg = {msg}");
                            // check if the msg is for us
                            if msg.contains(&("@".to_owned() + &config.bot_uname))
                                || update["message"]["reply_to_message"]["from"]["username"]
                                    .as_str()
                                    == Some(&config.bot_uname)
                                || update["message"]["chat"]["type"].as_str() == Some("private")
                            {
                                // send into channel
                                let chat_id = update["message"]["chat"]["id"]
                                    .as_i64()
                                    .context("telegram: could not get chat_id")?;
                                let user_id = update["message"]["from"]["id"]
                                    .as_i64()
                                    .context("telegram: could not get sender id")?;
                                let from =
                                    serde_json::to_string(&TelegramThread { chat_id, user_id })?;
                                let msg_id = update["message"]["message_id"]
                                    .as_i64()
                                    .context("could not get message_id")?;
                                let mut text =
                                    msg.replace(&("@".to_owned() + &config.bot_uname), "");
                                if let Some(uname) = update["message"]["from"]["username"].as_str()
                                {
                                    text = format!("From {uname}: \n{text}");
                                };
                                send_msgs
                                    .send(PlatformMsg {
                                        text,
                                        from,
                                        msg_id: msg_id.to_string(),
                                    })
                                    .await?;
                            }
                        }
                    }
                    anyhow::Ok(())
                };
                match fallible.timeout(Duration::from_secs(300)).await {
                    Some(x) => {
                        if let Err(err) = x {
                            log::error!("error getting updates: {:?}", err)
                        }
                    }
                    None => log::error!("timed out getting telegram updates!"),
                }
            }
        });
        Self {
            token,
            client,
            _task,
            recv_msgs,
        }
    }
}

async fn call_api(
    method: &str,
    args: Value,
    http_client: &HttpClient,
    token: &str,
) -> anyhow::Result<Value> {
    let raw_res: Value = http_client
        .send_async(
            Request::post(format!("https://api.telegram.org/bot{}/{method}", token))
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

#[async_trait]
impl Platform for Telegram {
    async fn send_msg(
        &self,
        msg: String,
        to: String,
        in_reply_to: Option<String>,
    ) -> anyhow::Result<()> {
        let TelegramThread {
            chat_id,
            user_id: _,
        } = serde_json::from_str(&to)?;
        let json = match in_reply_to {
            Some(id) => json!({
                "chat_id": chat_id,
                "text": msg,
                "reply_to_message_id": id,
            }),
            None => json!({
                "chat_id": chat_id,
                "text": msg,
            }),
        };
        call_api("sendMessage", json, &self.client, &self.token).await?;
        Ok(())
    }

    async fn recv_msg(&self) -> PlatformMsg {
        self.recv_msgs.recv().await.unwrap()
    }
}
