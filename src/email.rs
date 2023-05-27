use std::collections::HashMap;

use anyhow::Context;
use async_compat::CompatExt;
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::header;
use serde_json::json;
use smol::lock::Semaphore;
use warp::Filter;

use crate::{
    database::{Platform, Role},
    responder::respond,
    Message, CONFIG, DB,
};

#[derive(Debug)]
struct ParsedEmail {
    title: String,
    body: String,
    sender_name: String,
    sender_email: String,
    message_id: String,
}

pub async fn handle_email() -> () {
    let lol = warp::path("support-bot-email").and(warp::body::form()).map(
        |email: HashMap<String, String>| {
            smol::block_on(async {
                match handle_email_inner(email).await {
                    Ok(()) => "Success".to_owned(),
                    Err(err) => format!("Our email bot encountered an error! {:?}\nPlease try sending your email again!", err),
                }
            })
        },
    );

    warp::serve(lol).run(([0, 0, 0, 0], 3030)).compat().await;
}

async fn handle_email_inner(email: HashMap<String, String>) -> anyhow::Result<()> {
    let parsed_email = parse_email(email)?;
    log::debug!(
        "title: {}\nbody: {}\nsender_name: {}\nsender_email: {}\nmessage_id: {}",
        parsed_email.title,
        parsed_email.body,
        parsed_email.sender_name,
        parsed_email.sender_email,
        parsed_email.message_id
    );

    // let msg = Message {
    //     text: parsed_email.title + ": " + &parsed_email.body, // text = title + email body
    //     convo_id: get_convo_id(&parsed_email.sender_email).await?, // convo_id = sender email address
    // };
    // let resp = respond(msg.clone())
    //     .await
    //     .context("cannot calculate response")?;
    let resp = "Hi! My name is GephSupportBot. How can I help you today?".to_owned();

    // add question & response to db
    // DB.insert_msg(
    //     &msg,
    //     Platform::Email,
    //     Role::User,
    //     json!({ "sender": parsed_email.sender_email }),
    // )
    // .await?;
    // DB.insert_msg(
    //     &Message {
    //         text: resp.clone(),
    //         convo_id: msg.convo_id,
    //     },
    //     Platform::Email,
    //     Role::Assistant,
    //     json!({"lol": "todo"}),
    // )
    // .await?;

    // send email response
    send_email(
        &("RE: ".to_owned() + &parsed_email.title),
        &resp,
        &parsed_email.sender_email,
        Some(&parsed_email.message_id),
    )
    .await?;

    Ok(())
}

fn parse_email(email: HashMap<String, String>) -> anyhow::Result<ParsedEmail> {
    let title = email
        .get("subject")
        .unwrap_or(&"Unknown Subject".to_string())
        .clone();
    let body = email
        .get("body-plain")
        .unwrap_or(&"No Content".to_string())
        .clone();
    let sender = email
        .get("from")
        .unwrap_or(&"Unknown Sender".to_string())
        .clone();
    let message_id = email
        .get("Message-Id")
        .unwrap_or(&"No Message-Id".to_string())
        .clone();

    let re = Regex::new(r"^(?P<name>[^<]+)\s*<(?P<email>[^>]+)>$").unwrap();
    let captures = re.captures(&sender).context("cannot parse sender")?;

    let sender_name = captures
        .name("name")
        .context("no sender name")?
        .as_str()
        .trim()
        .to_string();
    let sender_email = captures
        .name("email")
        .context("no sender email")?
        .as_str()
        .trim()
        .to_string();

    Ok(ParsedEmail {
        title,
        body,
        sender_name,
        sender_email,
        message_id,
    })
}

async fn get_convo_id(sender_email: &str) -> anyhow::Result<i64> {
    // sender is stored in the metadata field of each email conversation
    todo!()
}

async fn send_email(
    subject: &str,
    body: &str,
    to: &str,
    in_reply_to: Option<&str>,
) -> anyhow::Result<()> {
    static MAILGUN_LIMIT: Lazy<Semaphore> = Lazy::new(|| Semaphore::new(16));
    let _guard = MAILGUN_LIMIT.acquire().await;
    log::info!("sending email!");
    let mut params = vec![
        ("from", "GephSupportBot <support@bot.geph.io>"),
        ("to", to),
        ("subject", subject),
        ("text", body),
    ];
    if let Some(in_reply_to) = in_reply_to {
        params.push(("h:In-Reply-To", in_reply_to));
    }

    log::debug!("params = {:?}", params);

    let base64_uname_pwd = base64::encode(format!("api:{}", CONFIG.mailgun_key));
    let auth_value = format!("Basic {}", base64_uname_pwd);

    let client = reqwest::Client::new();
    log::debug!("got a reqwest client!");
    let res = client
        .post("https://api.eu.mailgun.net/v3/bot.geph.io/messages")
        .header(header::AUTHORIZATION, auth_value)
        .form(&params)
        .send()
        .compat()
        .await?;
    log::debug!("{:?}", res);
    Ok(())
}
