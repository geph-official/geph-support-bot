use crate::{
    database::trim_convo_history,
    openai::{call_openai_api, get_chatbot_prompt},
    Message, CONFIG, DB,
};
use serde::{Deserialize, Serialize};
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

    let resp: AiResponse = serde_json::from_str(&call_openai_api(prompt, role_contents).await?)?;
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
    let mut conn = PgConnection::connect(&CONFIG.binder_db).await?;
    log::debug!("connected to binder!");
    let res = sqlx::query("update subscriptions set id = (select id from users_legacy where username='$1') where id = (select id from users_legacy where username='$2')")
    .bind(new_uname)
    .bind(old_uname).
    execute(&mut conn).await?;
    log::debug!("{} rows affected!", res.rows_affected());
    Ok(())
}
