use std::time::Duration;

use crate::{
    database::trim_convo_history,
    openai::{call_openai_api, get_chatbot_prompt},
    Message, CONFIG, DB,
};
use serde::{Deserialize, Serialize};
use smol::future::FutureExt;
use sqlx::{Connection, PgConnection};

#[derive(Serialize, Deserialize, Debug, Clone)]
enum Action {
    Null,
    TransferPlus {
        old_uname: String,
        new_uname: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct AiResponse {
    action: Action,
    text: String,
}

pub async fn respond(msg: Message) -> anyhow::Result<String> {
    // prompt
    let prompt = get_chatbot_prompt().await?;
    // chat history
    let mut role_contents = trim_convo_history(DB.get_convo_history(msg.convo_id).await?).await;
    let latest_msg = ("user".to_owned(), msg.text.replace("@GephSupportBot", ""));
    role_contents.push(latest_msg);
    let resp_string = call_openai_api("gpt-4", &prompt, &role_contents)
        .or(async {
            smol::Timer::after(Duration::from_secs(500)).await; // if gpt-4 doesn't respond in under 5 minutes, fall back to gpt-3.5
            log::warn!("FALLBACK to gpt-3.5"); // this is to handle the situation where OpenAI rate-limits gpt-4
            call_openai_api("gpt-3.5-turbo", &prompt, &role_contents).await
        })
        .await?;

    let resp: AiResponse = serde_json::from_str(&resp_string).unwrap_or_else(|_| AiResponse {
        action: Action::Null,
        text: resp_string.clone(),
    });
    log::debug!("AiResponse = {:?}", resp);

    // perform the action
    match resp.action {
        Action::Null => {}
        Action::TransferPlus {
            old_uname,
            new_uname,
        } => {
            transfer_plus(&old_uname, &new_uname).await?;
        }
    }

    Ok(resp.text)
    // Ok("Hello! Excited to be of assistance ^_^".to_owned())
}

async fn transfer_plus(old_uname: &str, new_uname: &str) -> anyhow::Result<()> {
    log::debug!("transfer_plus({old_uname}, {new_uname})");
    let mut conn =
        PgConnection::connect(&CONFIG.actions_config.as_ref().unwrap().binder_db).await?;
    log::debug!("connected to binder!");
    let res = sqlx::query("update subscriptions set id = (select id from users_legacy where username=$1) where id = (select id from users_legacy where username=$2)")
    .bind(new_uname)
    .bind(old_uname).
    execute(&mut conn).await?;
    log::debug!("{} rows affected!", res.rows_affected());
    Ok(())
}
