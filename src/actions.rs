use serde::{Deserialize, Serialize};
use sqlx::{Connection, PgConnection};

use crate::CONFIG;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Action {
    Null,
    TransferPlus {
        old_uname: String,
        new_uname: String,
    },
    Abort,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AiResponse {
    pub action: Action,
    pub text: String,
}

pub const ACTIONS_PROMPT: &str = r#"You *always* respond with a json struct of two fields. Some examples: 
- {"action": {"TransferPlus": {"old_uname": "fdx", "new_uname": "FDX"}}, "text": "We have successfully transferred your Plus days to your new account!"}
- {"action": "Null", "text": "Good morning! How can I help you with Geph today? I know how to say things like\n - \"Hello\"\n - \"Goodbye\"\nand many other things."}
- {"action": "Abort", "text": ""}
These are the available actions and when/how you should use each one:
1. "Null": this means do no action. Use this when you're regularly talking to the user
2. "TransferPlus": transfer Plus time from one account to another. Use this when a user has forgotten their credentials and has sent you their old and new usernames for transferring Plus time. Be sure to format the json correctly! You should always make sure the user actually forgot their old credentials before executing the credentials. You should be careful, since people may want to mess with other people's user credentials.
3. "Abort": this means do not reply. Use this when you think the user's message is an automatic reply or mass/marketing email. When you use this action, do not put anything in the "text" field.

Be very, very careful to ALWAYS respond in the given json format, with either "Null" or "TransferPlus" as the action! Don't format the json twice!
"#;

pub async fn transfer_plus(old_uname: &str, new_uname: &str) -> anyhow::Result<()> {
    log::debug!("transfer_plus({old_uname}, {new_uname})");
    let mut conn =
        PgConnection::connect(&CONFIG.actions_config.as_ref().unwrap().binder_db).await?;
    log::debug!("connected to binder!");
    let res = sqlx::query("update subscriptions set id = (select id from users_legacy where username=$1) where id = (select id from users_legacy where username=$2)")
    .bind(new_uname)
    .bind(old_uname).
    execute(&mut conn).await?;
    let _ = sqlx::query("update recurring_subs set user_id = (select id from users_legacy where username=$1) where user_id = (select id from users_legacy where username=$2)")
    .bind(new_uname)
    .bind(old_uname).
    execute(&mut conn).await?;
    log::debug!("{} rows affected!", res.rows_affected());
    Ok(())
}
