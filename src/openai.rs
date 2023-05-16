use anyhow::Context;
use isahc::{AsyncReadResponseExt, Request, RequestExt};
use serde_json::{json, Value};

use crate::{CONFIG, DB};

pub async fn call_openai_api(
    prompt: String,
    role_contents: Vec<(String, String)>,
) -> anyhow::Result<String> {
    let mut msgs: Vec<Value> = role_contents
        .iter()
        .map(|(role, content)| json!({"role": role, "content": content}))
        .collect();
    msgs.insert(0, json!({"role": "system", "content": prompt}));

    let req = json!({
        "model": "gpt-4",
        "messages": msgs
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
    log::debug!("OPENAI RESP = {:?}", resp);
    let resp = &mut resp["choices"][0]["message"];
    if resp["role"].is_string() {
        let toret = resp["content"]
            .as_str()
            .context("no content for response")?
            .to_string();
        Ok(toret)
    } else {
        anyhow::bail!("no role in response")
    }
}

pub async fn get_chatbot_prompt() -> anyhow::Result<String> {
    let initial_prompt = include_str!("initial-prompt.txt").to_owned();
    let facts = DB.get_all_facts().await?.join("\n");
    let ret = initial_prompt + "\n" + &facts;
    Ok(ret)
}
