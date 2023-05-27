use std::collections::HashMap;

use anyhow::Context;
use async_compat::CompatExt;
use warp::Filter;

use crate::{responder::respond, Message};

#[derive(Debug)]
struct ParsedEmail {
    title: String,
    body: String,
    sender: String,
    message_id: String,
}

pub async fn handle_email() -> () {
    let lol = warp::path("support-bot-email").and(warp::body::form()).map(
        |email: HashMap<String, String>| {
            smol::block_on(async {
                match handle_email_inner(email).await {
                    Ok(resp) => resp,
                    Err(err) => format!("Our email bot encountered an error! {:?}\nPlease try sending your email again!", err),
                }
            })
        },
    );

    warp::serve(lol).run(([0, 0, 0, 0], 3030)).compat().await;
}

async fn handle_email_inner(email: HashMap<String, String>) -> anyhow::Result<String> {
    let parsed_email = parse_email(email);
    log::debug!(
        "title: {}\nbody: {}\nsender: {}\nmessage_id: {}",
        parsed_email.title,
        parsed_email.body,
        parsed_email.sender,
        parsed_email.message_id
    );

    Ok("lol".to_owned())
    // let msg = Message {
    //     text: todo!(),     // text = title + email body
    //     convo_id: todo!(), // convo_id = sender email address
    // };
    // let resp = respond(msg.clone())
    //     .await
    //     .context("cannot calculate response")?;
    // // add question & response to db

    // send email response
}

fn parse_email(email: HashMap<String, String>) -> ParsedEmail {
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

    ParsedEmail {
        title,
        body,
        sender,
        message_id,
    }
}
