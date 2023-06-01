use anyhow::Context;
use isahc::{AsyncReadResponseExt, Request, RequestExt};
use serde_json::{json, Value};

use crate::{actions::ACTIONS_PROMPT, CONFIG, DB};

pub async fn call_openai_api(
    model: &str,
    prompt: &str,
    role_contents: &[(String, String)],
) -> anyhow::Result<String> {
    let mut msgs: Vec<Value> = role_contents
        .iter()
        .map(|(role, content)| json!({"role": role, "content": content}))
        .collect();
    msgs.insert(0, json!({"role": "system", "content": prompt}));

    let req = json!({
        "model": model,
        "messages": msgs,
        "max_tokens": 500
    });

    let mut resp: Value = Request::post("https://api.openai.com/v1/chat/completions")
        .header("Content-Type", "application/json")
        .header(
            "Authorization",
            "Bearer ".to_string() + &CONFIG.llm_config.openai_key,
        )
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

pub async fn get_chatbot_prompt(actions_enabled: bool) -> anyhow::Result<String> {
    let mut initial_prompt = include_str!("initial-prompt.txt").to_owned();
    if actions_enabled {
        initial_prompt = initial_prompt + ACTIONS_PROMPT;
    }
    let facts = DB.get_all_facts().await?.join("\n");
    let ret = initial_prompt + "\n" + &facts;
    Ok(ret)
}
