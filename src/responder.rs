use crate::{
    openai::{call_openai_api, ChatEntry},
    tools::TOOLS,
    CONFIG, DB,
};

use anyhow::Context;
use serde_json::Value;

pub async fn generate_response(thread: &str, user_input: &str) -> anyhow::Result<String> {
    DB.add_msg(
        thread,
        ChatEntry::User {
            content: user_input.to_string(),
        },
    )
    .await?;

    let llm_config = CONFIG.llm_config.clone();
    let prompt = include_str!("prompt.txt");

    loop {
        let inputs = DB.get_convo_history(thread).await?;
        let ai_resp = call_openai_api(&llm_config.main_model, prompt, inputs).await?;
        DB.add_msg(thread, ai_resp.clone()).await?;
        if let ChatEntry::Assistant {
            content,
            tool_calls,
        } = ai_resp
        {
            if let Some(content) = content {
                return Ok(content);
            } else if let Some(tool_calls) = tool_calls {
                for tool_call in tool_calls {
                    // call tool, save to db
                    let content =
                        call_tool(&tool_call.function.name, &tool_call.function.arguments).await?;
                    DB.add_msg(
                        thread,
                        ChatEntry::Tool {
                            content,
                            tool_call_id: tool_call.id,
                            name: tool_call.function.name,
                        },
                    )
                    .await?;
                }
            } else {
                anyhow::bail!(
                    "OpenAi sent a ChatEntry::Assistant with no content AND no tool_calls!"
                )
            }
        } else {
            anyhow::bail!("OpenAi sent a ChatEntry whose role is NOT 'assistant'!")
        }
    }
}

async fn call_tool(name: &str, arguments: &str) -> anyhow::Result<String> {
    let name = name.to_string();
    let param = arguments.to_string();
    smol::unblock(move || {
        let args: Value = serde_json::from_str(&param).context("cannot deserialize param")?;
        let res = (TOOLS
            .get(&name)
            .context("requested tool does not exist")?
            .call)(args)?;
        Ok(res)
    })
    .await
}
