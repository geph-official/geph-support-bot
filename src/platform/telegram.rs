use std::{convert::TryFrom, time::Duration};

use anyhow::{Context, Result};
use async_channel::{unbounded, Receiver, Sender};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use teloxide::{
    adaptors::DefaultParseMode,
    prelude::*,
    types::{ChatId, ChatKind, Message, MessageId, ParseMode, UpdateKind},
};
use tokio::runtime::Runtime;

use crate::{
    platform::{IncomingMsg, Platform},
    TelegramConfig,
};

use super::OutgoingMsg;

type MarkdownBot = DefaultParseMode<teloxide::Bot>;

fn escape_markdown_v2(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '_' | '*' | '[' | ']' | '(' | ')' | '~' | '`' | '>' | '#' | '+' | '-' | '=' | '|'
            | '{' | '}' | '.' | '!' | '\\' => {
                escaped.push('\\');
                escaped.push(ch);
            }
            _ => escaped.push(ch),
        }
    }
    escaped
}

pub struct Telegram {
    recv_msgs: Receiver<IncomingMsg>,
    send_reqs: Sender<SendRequest>,
    _thread: std::thread::JoinHandle<()>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramThread {
    pub chat_id: i64,
    pub user_id: i64,
}

struct SendRequest {
    msg: OutgoingMsg,
    completion: Sender<anyhow::Result<()>>,
}

impl Telegram {
    pub fn new(config: &TelegramConfig) -> Self {
        let config = config.clone();
        let (incoming_tx, incoming_rx) = unbounded();
        let (send_tx, send_rx) = unbounded();

        let thread_handle = std::thread::spawn(move || {
            let runtime = Runtime::new().expect("failed to start tokio runtime for telegram");
            runtime.block_on(async move {
                run_telegram_loop(config, incoming_tx, send_rx).await;
            });
        });

        Self {
            recv_msgs: incoming_rx,
            send_reqs: send_tx,
            _thread: thread_handle,
        }
    }
}

async fn run_telegram_loop(
    config: TelegramConfig,
    incoming_tx: Sender<IncomingMsg>,
    send_rx: Receiver<SendRequest>,
) {
    let http_client = teloxide::net::default_reqwest_settings()
        // Allow Telegram long polling (120s) to finish before reqwest aborts.
        .timeout(Duration::from_secs(130))
        .build()
        .expect("failed to build Telegram reqwest client");
    let bot = teloxide::Bot::with_client(config.telegram_token, http_client)
        .parse_mode(ParseMode::MarkdownV2);
    let bot_username = config.bot_uname;

    let updates_task = tokio::spawn(run_updates(bot.clone(), bot_username, incoming_tx));
    let send_task = tokio::spawn(run_sender(bot.clone(), send_rx));

    let _ = tokio::join!(updates_task, send_task);
}

async fn run_updates(bot: MarkdownBot, bot_username: String, incoming_tx: Sender<IncomingMsg>) {
    let mention = format!("@{}", bot_username);
    let mut offset: i32 = 0;
    loop {
        match bot
            .get_updates()
            .offset(offset + 1)
            .allowed_updates(Vec::new())
            .timeout(120)
            .send()
            .await
        {
            Ok(updates) => {
                for update in updates {
                    offset = offset.max(update.id);
                    if let UpdateKind::Message(message) = update.kind {
                        if let Some(text) = message.text() {
                            if should_handle(&message, text, &mention, &bot_username) {
                                if let Err(err) =
                                    forward_message(&message, text, &mention, &incoming_tx).await
                                {
                                    log::error!("failed to forward telegram message: {err:?}");
                                }
                            }
                        }
                    }
                }
            }
            Err(err) => {
                log::error!("failed to fetch telegram updates: {err:?}");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    }
}

fn should_handle(message: &Message, text: &str, mention: &str, bot_username: &str) -> bool {
    let mentions_bot = text.contains(mention);
    let is_reply_to_bot = message
        .reply_to_message()
        .and_then(|msg| msg.from())
        .and_then(|user| user.username.as_deref())
        .map(|uname| uname == bot_username)
        .unwrap_or(false);
    let is_private = matches!(message.chat.kind, ChatKind::Private(_));

    is_private || mentions_bot || is_reply_to_bot
}

async fn forward_message(
    message: &Message,
    text: &str,
    mention: &str,
    incoming_tx: &Sender<IncomingMsg>,
) -> Result<()> {
    let chat_id = message.chat.id.0;
    let user_id = message
        .from()
        .and_then(|user| i64::try_from(user.id.0).ok())
        .unwrap_or(chat_id);
    let from = serde_json::to_string(&TelegramThread { chat_id, user_id })?;
    let msg_id = message.id.0.to_string();
    let mut cleaned = text.replace(mention, "");
    if let Some(uname) = message.from().and_then(|user| user.username.as_deref()) {
        cleaned = format!("From {uname}: \n{cleaned}");
    }
    incoming_tx
        .send(IncomingMsg {
            text: cleaned,
            from,
            msg_id,
        })
        .await
        .context("telegram: failed to enqueue incoming message")?;
    Ok(())
}

async fn run_sender(bot: MarkdownBot, send_rx: Receiver<SendRequest>) {
    while let Ok(req) = send_rx.recv().await {
        let result = send_single(bot.clone(), req.msg).await;
        let _ = req.completion.send(result).await;
    }
}

async fn send_single(bot: MarkdownBot, outgoing: OutgoingMsg) -> anyhow::Result<()> {
    let TelegramThread { chat_id, .. } = serde_json::from_str(&outgoing.to)?;
    let escaped_text = escape_markdown_v2(&outgoing.text);
    let mut request = bot.send_message(ChatId(chat_id), escaped_text);
    if let Some(reply_to) = outgoing
        .in_reply_to
        .as_deref()
        .and_then(|id| id.parse::<i32>().ok())
    {
        request = request.reply_to_message_id(MessageId(reply_to));
    }
    request.send().await?;
    Ok(())
}

#[async_trait]
impl Platform for Telegram {
    async fn send_msg(&self, outgoing_msg: &OutgoingMsg) -> anyhow::Result<()> {
        let (tx, rx) = async_channel::bounded(1);
        self.send_reqs
            .send(SendRequest {
                msg: outgoing_msg.clone(),
                completion: tx,
            })
            .await
            .context("telegram: failed to queue outgoing message")?;
        rx.recv().await.context("telegram: sender task dropped")?
    }

    async fn recv_msg(&self) -> IncomingMsg {
        self.recv_msgs
            .recv()
            .await
            .expect("telegram receiver closed")
    }
}
