mod database;
mod email;
mod learn;
mod openai;
mod responder;
mod telegram;

use std::path::PathBuf;

use argh::FromArgs;
use database::ChatHistoryDb;
use email::{handle_email, send_email};
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
    history_db: String,
    binder_db: String,
    openai_key: String,
    telegram_token: String,
    mailgun_key: String,
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

static DB: Lazy<ChatHistoryDb> = Lazy::new(|| {
    smol::future::block_on(ChatHistoryDb::new(&CONFIG.history_db))
        .expect("cannot create chat history db")
});

static TELEGRAM: Lazy<TelegramBot> = Lazy::new(|| TelegramBot::new(&CONFIG.telegram_token));

fn main() {
    env_logger::init();
    // smolscale::block_on(send_email(
    //     "LOL",
    //     "testing",
    //     "thisbefruit@protonmail.com",
    //     Some("<Op0N_ZWG9kJ94MhV7sZn8HQknrT7KomlM2wlgpfj__SWIgMRYUpzeMA06dXDI8AqvRNqKnx4FH_v2vEqSznlb6GspwdvtTgfPjl-kUkLlXc=@proton.me>
    //     "),
    // ));
    smolscale::spawn(handle_email()).detach();
    smolscale::block_on(handle_telegram());
    // let s = include_str!("facts.txt");
    // let lines = s.lines();
    // smol::block_on(async {
    //     for line in lines {
    //         DB.insert_fact(line).await.unwrap();
    //     }
    // })
}
