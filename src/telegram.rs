use std::time::Duration;

use anyhow::Context;
use isahc::{AsyncReadResponseExt, Request};
use serde_json::{json, Value};
use smol_timeout::TimeoutExt;

use crate::{
    database::{Platform, Role},
    learn::learn,
    responder::respond,
    Message, CONFIG, DB,
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

pub async fn handle_telegram() {
    let telegram = TelegramBot::new(&CONFIG.telegram_config.as_ref().unwrap().telegram_token);
    let admin_uname = &CONFIG.telegram_config.as_ref().unwrap().admin_uname;
    let bot_uname = &CONFIG.telegram_config.as_ref().unwrap().bot_uname;
    let mut counter = 0;
    loop {
        log::info!("getting updates at {counter}");
        let fallible = async {
            let updates = telegram
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
                if !update["message"]["text"].is_null() {
                    let convo_id = get_convo_id(update.clone()).await?;
                    let msg = update["message"]["text"]
                        .as_str()
                        .context("cannot parse out text")?;
                    log::info!("msg = {msg}");
                    if msg.contains(&("@".to_owned() + bot_uname))
                        || update["message"]["reply_to_message"]["from"]["username"].as_str()
                            == Some(bot_uname)
                        || update["message"]["chat"]["type"].as_str() == Some("private")
                    {
                        let mut username = "";
                        let mut message = Message {
                            text: msg.replace(&("@".to_owned() + bot_uname), ""),
                            convo_id,
                        };
                        if let Some(uname) = update["message"]["from"]["username"].as_str() {
                            username = uname;
                            message.text = uname.to_owned() + ": " + &message.text;
                        };
                        // learn if the chat is from the admin & contains "#learn"
                        let resp = if username == admin_uname && message.text.contains("#learn") {
                            learn(message.clone()).await?
                        } else {
                            respond(message.clone())
                                .await
                                .context("cannot calculate response")?
                        };
                        if resp != "".to_string() {
                            // add question & response to db
                            DB.insert_msg(
                                &message,
                                Platform::Telegram,
                                Role::User,
                                json!({"lol": "todo"}),
                            )
                            .await?;
                            DB.insert_msg(
                                &Message {
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
                            telegram
                                .call_api("sendMessage", json_resp)
                                .await
                                .context("cannot send reply back to telegram")?;
                        }
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
}

async fn get_convo_id(update: Value) -> anyhow::Result<i64> {
    if update["message"]["chat"]["type"] == "private" {
        update["message"]["chat"]["id"]
            .as_i64()
            .context("chat id could not be converted to i64")
    } else {
        if !update["message"]["reply_to_message"].is_null() {
            if let Some(id) = DB
                .txt_to_id(
                    update["message"]["reply_to_message"]["text"]
                        .as_str()
                        .context("could not get reply_to_message text")?,
                )
                .await
            {
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
