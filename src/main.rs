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
use platform::{Email, Platform, PlatformMsg, Telegram};
use responder::generate_response;
use serde::{Deserialize, Serialize};
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
    telegram_config: Option<TelegramConfig>,
    email_config: Option<EmailConfig>,
}

#[derive(Serialize, Deserialize, Clone)]
struct LlmConfig {
    openai_key: String,
    main_model: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct TelegramConfig {
    telegram_token: String,
    admin_chat_id: i64,
    bot_uname: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct EmailConfig {
    mailgun_url: String,
    mailgun_key: String,
    address: String,
    signature: String,
    cc: Option<String>,
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

// static TELEGRAM: Lazy<TelegramBot> =
//     Lazy::new(|| TelegramBot::new(&CONFIG.telegram_config.as_ref().unwrap().telegram_token));

fn main() {
    env_logger::init();

    if let Some(config) = CONFIG.email_config.as_ref() {
        smolscale::spawn(run_bot(Email::new(config))).detach();
    }

    if let Some(config) = CONFIG.telegram_config.as_ref() {
        smolscale::block_on(run_bot(Telegram::new(config)))
    }

    // match smolscale::block_on(DB.add_msg(
    //     "lol",
    //     ChatEntry::User {
    //         content: "hey test".to_owned(),
    //     },
    // )) {
    //     Ok(_) => println!("success"),
    //     Err(e) => println!("ERR!!: {e}"),
    // }

    // match smolscale::block_on(call_openai_api(
    //     "gpt-4-turbo",
    //     include_str!("prompt.txt"),
    //     vec![ChatEntry::User {
    //         content: "hey how's it going".to_string(),
    //     }],
    // )) {
    //     Ok(res) => println!("RESPONSE: {:#?}", res),
    //     Err(e) => println!("ERR!!: {e}"),
    // };

    // let telegram = Telegram::new(CONFIG.telegram_config.as_ref().unwrap());
    // smolscale::block_on(async move {
    //     telegram
    //         .send_msg("hey thisbe".to_string(), "802173924".to_string(), None)
    //         .await
    //         .unwrap()
    // });
}

async fn run_bot(platform: impl Platform) {
    loop {
        let fallible = async {
            let PlatformMsg { text, from, msg_id } = platform.recv_msg().await;
            let resp = generate_response(&from, &text).await?;
            platform.send_msg(&resp, &from, Some(&msg_id)).await?;
            anyhow::Ok(())
        };
        if let Err(e) = fallible.await {
            log::error!("run_bot failed with err = {e}")
        }
    }
}
