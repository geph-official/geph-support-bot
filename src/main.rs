mod responder;
mod telegram;

use std::path::PathBuf;

use argh::FromArgs;
use async_broadcast::broadcast;
use once_cell::sync::Lazy;
use responder::respond_loop;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use telegram::TelegramBot;

/// A tool to run the Geph support bot.
#[derive(FromArgs, PartialEq, Debug)]
struct Args {
    /// configuration YAML file path
    #[argh(option, short = 'c', long = "config")]
    config: PathBuf,
}

static ARGS: Lazy<Args> = Lazy::new(argh::from_env);

static CONFIG: Lazy<Config> = Lazy::new(|| {
    serde_yaml::from_slice(&std::fs::read(&ARGS.config).expect("cannot read config file"))
        .expect("cannot parse config file")
});

static TELEGRAM: Lazy<TelegramBot> = Lazy::new(|| TelegramBot::new(&CONFIG.telegram_token));

static RECV_UPDATES: Lazy<async_broadcast::Receiver<Value>> = Lazy::new(|| {
    // spin off a task to receive the updates
    let (mut send, recv) = broadcast(100);
    send.set_overflow(true);
    smolscale::spawn(async move {
        let mut counter = 0;
        loop {
            log::debug!("getting updates at {counter}");
            let fallible = async {
                let updates = TELEGRAM
                    .call_api(
                        "getUpdates",
                        json!({"timeout": 120, "offset": counter + 1, "allowed_updates": []}),
                    )
                    .await?;
                let updates: Vec<Value> = serde_json::from_value(updates)?;

                for update in updates {
                    counter = counter.max(update["update_id"].as_i64().unwrap_or_default());
                    if update["my_chat_member"].is_null() {
                        send.broadcast(update).await?;
                    }
                }
                anyhow::Ok(())
            };
            if let Err(err) = fallible.await {
                log::warn!("error getting updates: {:?}", err)
            }
        }
    })
    .detach();
    recv
});

/// The struct containing the bot configuration
#[derive(Serialize, Deserialize)]
struct Config {
    telegram_token: String,
    openai_key: String,
    learn_db: PathBuf,
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let _resp_loop = smolscale::spawn(respond_loop());
    smolscale::block_on(async move {
        let mut recv = RECV_UPDATES.clone();
        loop {
            let update = recv.recv().await?;
            if !update["message"].is_null() {
                log::debug!("GOT MESSAGE: {}", serde_json::to_string(&update)?);
            }
        }
    })
}
