use anyhow::Context;
use isahc::{AsyncReadResponseExt, Request, RequestExt};
use serde_json::{json, Value};

use crate::{tools::TOOLS, CONFIG};

pub async fn call_openai_api(
    model: &str,
    prompt: &str,
    mut input: Vec<Value>,
) -> anyhow::Result<String> {
    input.insert(0, json!({"role": "system", "content": prompt}));

    let req = json!({
        "model": model,
        "messages": input,
        "tools": get_tools(),
        "tool_choice": "auto",
        "max_tokens": 500
    });

    log::debug!("sending to openai: {:#?}", req);

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
    log::debug!("OPENAI RESP = {:#?}", resp);
    let resp_msg = &mut resp["choices"][0]["message"];
    println!("{:#?}", resp_msg);
    if resp_msg["role"].is_string() {
        match resp_msg["content"].as_str() {
            Some(toret) => return Ok(toret.to_string()),
            None => {
                input.push(resp_msg.clone());
                if let Ok(tool_calls) = resp_msg["tool_calls"]
                    .as_array()
                    .context("tool_calls is not an array")
                {
                    for tool_call in tool_calls {
                        let name = tool_call["function"]["name"]
                            .as_str()
                            .context("function call has no name")
                            .unwrap();
                        let arguments = tool_call["function"]["arguments"]
                            .as_str()
                            .context("function call has no arguments")
                            .unwrap();
                        // call function
                        let result = match call_tool(name, arguments) {
                            Ok(res) => res,
                            Err(err) => json!(err.to_string()),
                        };
                        input.push(json!({
                        "tool_call_id": tool_call["id"].as_str().context("no tool_call.id").unwrap(),
                        "role": "tool",
                        "name": name,
                        "content": result.to_string(),
                    }))
                    }
                    let fut = Box::pin(call_openai_api(model, prompt, input));
                    return fut.await;
                } else {
                    anyhow::bail!("not tool_call but no msg content")
                }
            }
        }
    } else {
        anyhow::bail!("no role in response")
    }
}

pub async fn get_chatbot_prompt() -> anyhow::Result<String> {
    let prompt = include_str!("prompt.txt").to_owned();
    Ok(prompt)
}

fn call_tool(name: &str, param: &str) -> anyhow::Result<Value> {
    let args: Value = serde_json::from_str(param).context("cannot deserialize param")?;
    let res = (TOOLS
        .get(name)
        .context("requested tool does not exist")?
        .call)(args)?;
    Ok(res)
}

pub fn get_tools() -> Vec<Value> {
    TOOLS
        .values()
        .map(|tool| {
            json!({
                "type": "function",
                "function": {
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.parameters,
                }
            })
        })
        .collect()
}
