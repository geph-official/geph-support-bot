use std::time::Instant;

use acidjson::AcidJson;
use anyhow::Context;
use isahc::{AsyncReadResponseExt, Request, RequestExt};
use once_cell::sync::Lazy;
use serde_json::{json, Value};

use crate::{CONFIG, RECV_UPDATES, TELEGRAM};

pub async fn respond_loop() {
    let mut recv = RECV_UPDATES.clone();
    loop {
        let fallible = async {
            let incoming = recv.recv().await?;
            if incoming["message"].is_object() {
                let message = incoming["message"]["text"]
                    .as_str()
                    .context("cannot parse out text")?;
                if message.contains("@GephSupportBot")
                    || incoming["message"]["reply_to_message"]["from"]["username"].as_str()
                        == Some("GephSupportBot")
                {
                    let raw_json = serde_json::to_string(&incoming["message"])?;
                    let response = respond_once(&raw_json.replace("@GephSupportBot", "")).await?;
                    log::debug!("LLM response: {response}");

                    let response: Value = serde_json::from_str(&response)
                        .context("bot did not provide valid JSON")?;

                    TELEGRAM.call_api("sendMessage", response).await?;
                }
            }
            anyhow::Ok(())
        };
        if let Err(err) = fallible.await {
            log::warn!("failed: {:?}", err)
        }
    }
}

static LEARN_DB: Lazy<AcidJson<Vec<String>>> = Lazy::new(|| {
    AcidJson::open_or_else(&CONFIG.learn_db, Vec::new).expect("could not open learning DB")
});

async fn respond_once(msg: &str) -> anyhow::Result<String> {
    if !msg.contains("is_forum") {
        anyhow::bail!("not in a forum")
    }

    static MESSAGES: Lazy<smol::lock::Mutex<Vec<Value>>> = Lazy::new(|| {
        vec![json!({"role": "system", "content": include_str!("initial-prompt.txt")})].into()
    });

    log::debug!("waiting in queue...");
    let mut messages = MESSAGES.lock().await;
    log::debug!("thinking...");

    while messages
        .iter()
        .fold(0, |len, a| len + a["content"].as_str().unwrap().len())
        > 10000
    {
        log::debug!("trimming history");
        messages.remove(1);
    }

    messages.push(json!({"role": "user", "content": msg}));
    let req = json!({
        "model": "gpt-4",
        "messages": messages.clone(),
        // "max_tokens": 300
    });
    let mut resp: Value = Request::post("https://api.openai.com/v1/chat/completions")
        .header("Content-Type", "application/json")
        .header("Authorization", "Bearer ".to_string() + &CONFIG.openai_key)
        .body(serde_json::to_vec(&req)?)?
        .send_async()
        .await?
        .json()
        .await?;

    let resp = &mut resp["choices"][0]["message"];
    if resp["role"].is_string() {
        let mut toret = resp["content"]
            .as_str()
            .context("no content for response")?
            .to_string();

        // fix a common issue with dumber LLMs
        if !msg.contains("is_topic_message") {
            toret = toret.replace("message_thread_id", "dummy");
            resp["content"] = toret.clone().into();
        }

        messages.push(resp.clone());
        Ok(toret)
    } else {
        anyhow::bail!("no role in response")
    }
}
