use anyhow::Context;
use isahc::{AsyncReadResponseExt, Request, RequestExt};
use serde_json::{json, Value};

use crate::{Message, CONFIG, DB};

pub async fn respond(msg: Message) -> anyhow::Result<String> {
    // prompt
    let prompt = include_str!("initial-prompt.txt").to_owned();

    // chat history
    let mut role_contents = trim_context(DB.get_context(msg.convo_id).await?).await;
    // add the latest msg to the convo
    let latest_msg = ("user".to_owned(), msg.text.replace("@GephSupportBot", ""));
    role_contents.push(latest_msg);

    let resp = call_openai_api(prompt, role_contents).await?;
    log::debug!("LLM response: {resp}");
    Ok(resp)
}

// TODO!
async fn trim_context(mut context: Vec<(String, String)>) -> Vec<(String, String)> {
    // trim context if too long
    // currently: simple truncation. may summarize with gpt to compress later
    while context
        .iter()
        .fold(0, |len, (s1, s2)| len + s1.len() + s2.len())
        > 10000
    {
        context.remove(0);
    }
    context
}

async fn call_openai_api(
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
