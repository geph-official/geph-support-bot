use std::{collections::HashMap, time::Duration};

use anyhow::Context;
use async_compat::CompatExt;
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::header;
use serde_json::{json, Value};
use smol::lock::Semaphore;
use smol_timeout::TimeoutExt;
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
    date: String,
}

pub async fn handle_email() -> () {
    let lol = warp::path("support-bot-email").and(warp::body::form()).then(
         |email: HashMap<String, String>| async move {
            match handle_email_inner(email).await {
                Ok(()) => "Success".to_owned(),
                Err(err) => format!("Our email bot encountered an error! {:?}\nPlease try sending your email again!", err),
            }
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
        parsed_email.message_id,
    );

    let msg = Message {
        text: parsed_email.title.clone() + ": " + &parsed_email.body, // text = title + email body
        convo_id: get_convo_id(make_email_metadata(&parsed_email.sender_email)).await?, // convo_id = sender email address
    };
    let resp = respond(msg.clone())
        .await
        .context("cannot calculate response")?;

    if resp != "".to_string() {
        let resp = format!(
            "{}\n\n{}\n\n\n\n> ------- Original Message -------\nOn {}, {} <{}> wrote:\n\n{}",
            resp,
            &CONFIG.email_config.as_ref().unwrap().signature,
            parsed_email.date,
            parsed_email.sender_name,
            parsed_email.sender_email,
            parsed_email.body
        );

        // add question & response to db
        DB.insert_msg(
            &msg,
            Platform::Email,
            Role::User,
            make_email_metadata(&parsed_email.sender_email),
        )
        .await?;
        DB.insert_msg(
            &Message {
                text: resp.clone(),
                convo_id: msg.convo_id,
            },
            Platform::Email,
            Role::Assistant,
            make_email_metadata(&parsed_email.sender_email),
        )
        .await?;

        // send email response
        send_email(
            &("RE: ".to_owned() + &parsed_email.title),
            &resp,
            &parsed_email.sender_email,
            Some(&parsed_email.message_id),
        )
        .await?;
    }

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
    let date = email
        .get("Date")
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
        date,
    })
}

async fn get_convo_id(email_metadata: Value) -> anyhow::Result<i64> {
    // sender is stored in the metadata field of each email conversation
    match DB.email_metadata_to_id(email_metadata).await {
        Some(id) => Ok(id),
        None => Ok(rand::random()),
    }
}

fn make_email_metadata(user_email: &str) -> Value {
    json!({ "user": user_email })
}

pub async fn send_email(
    subject: &str,
    body: &str,
    to: &str,
    in_reply_to: Option<&str>,
) -> anyhow::Result<()> {
    static MAILGUN_LIMIT: Lazy<Semaphore> = Lazy::new(|| Semaphore::new(16));
    let _guard = MAILGUN_LIMIT.acquire().await;
    log::info!("sending email!");
    let mut params = vec![
        (
            "from".to_string(),
            CONFIG.email_config.as_ref().unwrap().address.clone(),
        ),
        ("to".to_string(), to.to_string()),
        ("subject".to_string(), subject.to_string()),
        ("text".to_string(), body.to_string()),
    ];
    if let Some(cc) = CONFIG.email_config.as_ref().unwrap().cc.clone() {
        params.push(("cc".to_string(), cc));
    }
    if let Some(in_reply_to) = in_reply_to {
        params.push(("h:In-Reply-To".to_string(), in_reply_to.to_string()));
    }

    log::debug!("params = {:?}", params);

    let base64_uname_pwd = base64::encode(format!(
        "api:{}",
        CONFIG.email_config.as_ref().unwrap().mailgun_key
    ));
    let auth_value = format!("Basic {}", base64_uname_pwd);

    let client = reqwest::Client::new();
    let res = client
        .post(&CONFIG.email_config.as_ref().unwrap().mailgun_url)
        .header(header::AUTHORIZATION, auth_value)
        .form(&params)
        .send()
        .timeout(Duration::from_secs(10))
        .await;
    log::debug!("response from mailgun: {:?}", res);
    Ok(())
}
