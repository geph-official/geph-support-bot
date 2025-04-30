use isahc::{AsyncReadResponseExt, Request, RequestExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{tools::TOOLS, CONFIG};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum ChatEntry {
    System {
        content: String,
    },
    User {
        content: String,
    },
    Assistant {
        content: Option<String>,
        tool_calls: Option<Vec<ToolCall>>,
    },
    Tool {
        content: String,
        tool_call_id: String,
        name: String, // optional
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolCall {
    pub id: String,
    pub r#type: String,
    pub function: ToolCallFunction,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

pub async fn call_openai_api(
    model: &str,
    prompt: &str,
    mut inputs: Vec<ChatEntry>,
) -> anyhow::Result<ChatEntry> {
    inputs.insert(
        0,
        ChatEntry::System {
            content: prompt.to_string(),
        },
    );
    let req = json!({
        "model": model,
        "temperature": 0.7,
        "messages": serde_json::to_value(inputs)?,
        // "tools": get_tools(),
        // "tool_choice": "auto",
    });

    log::debug!("sending to openai: {:#?}", req);

    let resp: Value = Request::post(CONFIG.llm_config.api_url.clone() + "/chat/completions")
        .header("Content-Type", "application/json")
        .header(
            "Authorization",
            "Bearer ".to_string() + &CONFIG.llm_config.api_key,
        )
        .body(serde_json::to_vec(&req)?)?
        .send_async()
        .await?
        .json()
        .await?;

    log::debug!("OPENAI RESP = {:#?}", resp);

    let resp_msg = resp["choices"][0]["message"].clone();
    let content = resp_msg["content"].as_str().map(|s| s.to_string());
    let tool_calls = resp_msg["tool_calls"]
        .as_array()
        .map(|vec_values| {
            vec_values
                .iter()
                .map(|value| serde_json::from_value(value.clone()))
                .collect()
        })
        .transpose()?;
    Ok(ChatEntry::Assistant {
        content,
        tool_calls,
    })
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
