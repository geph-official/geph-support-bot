mod database;
mod email;
mod learn;
mod openai;
mod responder;
mod telegram;

use std::path::PathBuf;

use argh::FromArgs;
use database::ChatHistoryDb;
use email::handle_email;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use telegram::handle_telegram;

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
    openai_key: String,
    telegram_config: Option<TelegramConfig>,
    email_config: Option<EmailConfig>,
    actions_config: Option<ActionsConfig>,
}

#[derive(Serialize, Deserialize, Clone)]
struct TelegramConfig {
    telegram_token: String,
    admin_uname: String,
    bot_uname: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct EmailConfig {
    mailgun_url: String,
    mailgun_key: String,
    address: String,
    signature: String,
    cc: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct ActionsConfig {
    binder_db: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Message {
    pub text: String,
    pub convo_id: i64,
}

// global variables //

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

    if CONFIG.email_config.is_some() {
        smolscale::spawn(handle_email()).detach();
    }

    if CONFIG.telegram_config.is_some() {
        smolscale::block_on(handle_telegram());
    }
}
