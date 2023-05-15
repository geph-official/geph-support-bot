mod database;
mod learn;
mod responder;
mod telegram;

use std::path::PathBuf;

use argh::FromArgs;
use database::ChatHistory;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use telegram::{handle_telegram, TelegramBot};

/// A tool to run the Geph support bot.
#[derive(FromArgs, PartialEq, Debug)]
struct Args {
    /// configuration YAML file path
    #[argh(option, short = 'c', long = "config")]
    config: PathBuf,
}

/// The struct containing the bot configuration
#[derive(Serialize, Deserialize)]
struct Config {
    telegram_token: String,
    openai_key: String,
    db_path: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Message {
    pub text: String,
    pub convo_id: i64,
}

static ARGS: Lazy<Args> = Lazy::new(argh::from_env);

static CONFIG: Lazy<Config> = Lazy::new(|| {
    let s = &std::fs::read(&ARGS.config).expect("cannot read config file");
    serde_yaml::from_slice(s).expect("cannot parse config file")
});

static DB: Lazy<ChatHistory> = Lazy::new(|| {
    smol::future::block_on(ChatHistory::new(&CONFIG.db_path))
        .expect("cannot create chat history db")
});

static TELEGRAM: Lazy<TelegramBot> = Lazy::new(|| TelegramBot::new(&CONFIG.telegram_token));

fn main() {
    env_logger::init();
    smolscale::block_on(handle_telegram());
    // todo: email loop
}
