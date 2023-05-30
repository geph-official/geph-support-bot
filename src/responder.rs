use std::time::Duration;

use crate::{
    actions::{self, transfer_plus, Action, AiResponse},
    database::trim_convo_history,
    openai::{call_openai_api, get_chatbot_prompt},
    Message, CONFIG, DB,
};

use smol::future::FutureExt;

pub async fn respond(msg: Message) -> anyhow::Result<String> {
    let actions_enabled = CONFIG.actions_config.is_some();
    // prompt
    let prompt = get_chatbot_prompt(actions_enabled).await?;
    // chat history
    let mut role_contents = trim_convo_history(DB.get_convo_history(msg.convo_id).await?).await;
    let latest_msg = ("user".to_owned(), msg.text);
    role_contents.push(latest_msg);
    let resp_string = call_openai_api("gpt-4", &prompt, &role_contents)
        .or(async {
            smol::Timer::after(Duration::from_secs(500)).await; // if gpt-4 doesn't respond in under 5 minutes, fall back to gpt-3.5
            log::warn!("FALLBACK to gpt-3.5"); // this is to handle the situation where OpenAI rate-limits gpt-4
            call_openai_api("gpt-3.5-turbo", &prompt, &role_contents).await
        })
        .await?;
    if actions_enabled {
        let resp = serde_json::from_str(&resp_string).unwrap_or_else(|_| AiResponse {
            action: Action::Null,
            text: resp_string.clone(),
        });
        // perform the action
        match resp.action {
            Action::Null => {}
            Action::TransferPlus {
                old_uname,
                new_uname,
            } => {
                transfer_plus(&old_uname, &new_uname).await?;
            }
        };

        Ok(resp.text)
    } else {
        Ok(resp_string)
    }
}
