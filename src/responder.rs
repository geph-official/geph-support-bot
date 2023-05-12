use anyhow::Context;
use isahc::{AsyncReadResponseExt, Request, RequestExt};
use serde_json::{json, Value};

use crate::{Message, CONFIG, DB};

pub async fn respond(msg: Message) -> anyhow::Result<String> {
    // prompt
    let prompt = include_str!("initial-prompt.txt").to_owned();

    // context
    let mut context = DB.get_context(msg.convo_id).await?;
    // trim context if too long
    while context.iter().fold(0, |len, s| len + s.len()) > 10000 {
        context.remove(0);
    }
    // add the latest msg to the convo
    let latest_msg = "you: ".to_owned() + &msg.text.replace("@GephSupportBot", "");
    context.push(latest_msg);
    let msg = context.join("\n");

    let resp = call_openai_api(prompt, msg).await?;
    log::debug!("LLM response: {resp}");
    Ok(resp)
}

async fn call_openai_api(prompt: String, msg: String) -> anyhow::Result<String> {
    let req = json!({
        "model": "gpt-4",
        "messages": vec![
            json!({"role": "system", "content": prompt}),
            json!({"role": "user", "content": msg})],
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
        let toret = resp["content"]
            .as_str()
            .context("no content for response")?
            .to_string();
        Ok(toret)
    } else {
        anyhow::bail!("no role in response")
    }
}
