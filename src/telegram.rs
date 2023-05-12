use anyhow::Context;
use isahc::{AsyncReadResponseExt, Request};
use serde_json::{json, Value};

use crate::{
    database::{Platform, Role},
    responder::respond,
    Message, DB, TELEGRAM,
};

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

pub async fn recv_telegram() {
    let mut counter = 0;
    loop {
        log::debug!("getting updates at {counter}");
        let fallible = async {
            let updates = TELEGRAM
                .call_api(
                    "getUpdates",
                    json!({"timeout": 120, "offset": counter + 1, "allowed_updates": []}),
                )
                .await
                .context("cannot call telegram for updates")?;
            let updates: Vec<Value> = serde_json::from_value(updates)?;

            for update in updates {
                // we only support text msgs atm
                counter = counter.max(update["update_id"].as_i64().unwrap_or_default());
                if update["my_chat_member"].is_null() && update["message"].is_object() {
                    // todo: learn if the chat is from the admin!
                    let convo_id = get_convo_id(update.clone()).await.unwrap();
                    let msg = update["message"]["text"]
                        .as_str()
                        .context("cannot parse out text")?;
                    if !msg.contains("is_forum") {
                        anyhow::bail!("not in a forum")
                    }
                    if msg.contains("@GephSupportBot")
                        || update["message"]["reply_to_message"]["from"]["username"].as_str()
                            == Some("GephSupportBot")
                    {
                        let message = Message {
                            text: msg.to_owned(),
                            convo_id,
                        };
                        let resp = respond(message.clone())
                            .await
                            .context("cannot calculate response")?;

                        // add question & response to db
                        DB.add_msg(
                            message.clone(),
                            Platform::Telegram,
                            Role::User,
                            json!({"lol": "todo"}),
                        )
                        .await?;
                        DB.add_msg(
                            Message {
                                text: resp.clone(),
                                convo_id: message.convo_id,
                            },
                            Platform::Telegram,
                            Role::Assistant,
                            json!({"lol": "todo"}),
                        )
                        .await?;

                        // send response to telegram
                        let json_resp = telegram_json(
                            resp,
                            update["message"]["chat"]["id"]
                                .as_i64()
                                .context("could not get chat id")?,
                            update["message"]["message_id"]
                                .as_i64()
                                .context("could not get message_id")?,
                        );
                        TELEGRAM
                            .call_api("sendMessage", json_resp)
                            .await
                            .context("cannot send reply back to telegram")?;
                    }
                }
            }
            anyhow::Ok(())
        };
        if let Err(err) = fallible.await {
            log::warn!("error getting updates: {:?}", err)
        }
    }
}

async fn get_convo_id(update: Value) -> anyhow::Result<i64> {
    if update["chat"]["type"] == "private" {
        update["chat"]["id"]
            .as_i64()
            .context("chat_id could not be converted to i64")
    } else {
        if update["message"]["reply_to_message"].is_object() {
            if let Some(id) = DB.txt_to_id(&update["message"]["text"].to_string()).await {
                return Ok(id);
            }
        }
        Ok(rand::random())
    }
}

// puts message into correct json format for telegram bot api
fn telegram_json(msg: String, chat_id: i64, reply_to_message_id: i64) -> Value {
    json!({
        "chat_id": chat_id,
        "text": msg,
        "reply_to_message_id": reply_to_message_id,
    })
}
