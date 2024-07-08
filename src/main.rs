mod database;
mod openai;
mod platform;
mod responder;
mod tools;

use std::path::PathBuf;

use argh::FromArgs;
use database::ChatHistoryDb;
// use email::handle_email;
use once_cell::sync::Lazy;
use platform::{Email, IncomingMsg, OutgoingMsg, Platform, Telegram};
use responder::generate_response;
use serde::{Deserialize, Serialize};
use smol::future::FutureExt;
// use telegram::{handle_telegram, TelegramBot};

/// A tool to run the Geph support bot.
#[derive(FromArgs, PartialEq, Debug)]
struct Args {
    /// configuration YAML file path
    #[argh(option, short = 'c', long = "config")]
    config: PathBuf,
}

/// The struct containing the bot configuration
#[derive(Serialize, Deserialize, Clone)]
struct Config {
    history_db: String,
    llm_config: LlmConfig,
    telegram_config: TelegramConfig,
    email_config: EmailConfig,
    tools_config: ToolsConfig,
}

#[derive(Serialize, Deserialize, Clone)]
struct LlmConfig {
    openai_key: String,
    model: String,
    temperature: f32,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TelegramConfig {
    pub telegram_token: String,
    pub admin_chat_id: i64,
    pub bot_uname: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct EmailConfig {
    pub mailgun_url: String,
    pub mailgun_key: String,
    pub address: String,
    pub signature: String,
    pub cc: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
struct ToolsConfig {
    query_chat_history_db_secret: String,
    support_secret: String,
}

static ARGS: Lazy<Args> = Lazy::new(argh::from_env);

static CONFIG: Lazy<Config> = Lazy::new(|| {
    let s = &std::fs::read(&ARGS.config).expect("cannot read config file");
    serde_yaml::from_slice(s).expect("cannot parse config file")
});

static DB: Lazy<ChatHistoryDb> = Lazy::new(|| {
    smol::future::block_on(ChatHistoryDb::new(&CONFIG.history_db))
        .expect("cannot create chat history db")
});

fn main() {
    env_logger::init();

    let email = Email::new(&CONFIG.email_config);
    let telegram = Telegram::new(&CONFIG.telegram_config);

    smolscale::block_on(async { run_bot(email).race(run_bot(telegram)).await });
}

async fn run_bot(platform: impl Platform) {
    loop {
        let fallible = async {
            let IncomingMsg { text, from, msg_id } = platform.recv_msg().await;
            let resp = generate_response(&from, &text).await?;
            platform
                .send_msg(&OutgoingMsg {
                    text: resp,
                    to: from,
                    in_reply_to: Some(msg_id),
                })
                .await?;
            anyhow::Ok(())
        };
        if let Err(e) = fallible.await {
            log::error!("run_bot failed with err = {e}")
        }
    }
}
