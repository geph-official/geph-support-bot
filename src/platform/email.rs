use std::{collections::HashMap, time::Duration};

use async_compat::CompatExt;
use async_trait::async_trait;
use isahc::http;
use once_cell::sync::Lazy;
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};
use smol::{lock::Semaphore, Task};
use smol_timeout::TimeoutExt;
use warp::Filter;

use crate::{
    platform::{IncomingMsg, Platform},
    EmailConfig,
};

use super::OutgoingMsg;

pub struct Email {
    config: EmailConfig,
    client: Client,
    _task: Task<()>,
    recv_msgs: smol::channel::Receiver<IncomingMsg>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EmailMsg {
    pub title: String,
    pub body: String,
}

impl Email {
    pub fn new(config: &EmailConfig) -> Self {
        let (send_msgs, recv_msgs) = smol::channel::unbounded();
        let lol = warp::path("support-bot-email")
            .and(warp::body::form())
            .then(move |email: HashMap<String, String>| {
                let send_msgs = send_msgs.clone();
                async move {
                    match parse_email(email) {
                        Ok(msg) => {
                            let _ = send_msgs.send(msg).await;
                            http::StatusCode::OK
                        }
                        Err(err) => {
                            log::error!("Error parsing email: {:?}", err);
                            http::StatusCode::INTERNAL_SERVER_ERROR
                        }
                    }
                }
            });
        let _task = smolscale::spawn(warp::serve(lol).run(([0, 0, 0, 0], 3030)).compat());
        Self {
            config: config.clone(),
            client: reqwest::Client::new(),
            _task,
            recv_msgs,
        }
    }
}

#[async_trait]
impl Platform for Email {
    async fn send_msg(&self, outgoing_msg: &OutgoingMsg) -> anyhow::Result<()> {
        let EmailMsg { title, body } = serde_json::from_str(&outgoing_msg.text)?;
        log::debug!("title={title}, body={body}");
        let title = "RE: ".to_owned() + &title;

        static MAILGUN_LIMIT: Lazy<Semaphore> = Lazy::new(|| Semaphore::new(16));
        let _guard = MAILGUN_LIMIT.acquire().await;
        let mut params = vec![
            ("from".to_string(), self.config.address.clone()),
            ("to".to_string(), outgoing_msg.to.to_string()),
            ("subject".to_string(), title),
            ("text".to_string(), body),
        ];
        if let Some(cc) = self.config.cc.clone() {
            params.push(("cc".to_string(), cc));
        }
        if let Some(in_reply_to) = outgoing_msg.in_reply_to.clone() {
            params.push(("h:In-Reply-To".to_string(), in_reply_to.to_string()));
        }

        log::debug!("params = {:?}", params);

        let base64_uname_pwd = base64::encode(format!("api:{}", self.config.mailgun_key));
        let auth_value = format!("Basic {}", base64_uname_pwd);

        let res = self
            .client
            .post(&self.config.mailgun_url)
            .header(header::AUTHORIZATION, auth_value)
            .form(&params)
            .send()
            .compat()
            .timeout(Duration::from_secs(10))
            .await;
        log::debug!("response from mailgun: {:?}", res);
        Ok(())
    }

    async fn recv_msg(&self) -> IncomingMsg {
        self.recv_msgs.recv().await.unwrap()
    }
}

fn parse_email(email: HashMap<String, String>) -> anyhow::Result<IncomingMsg> {
    let title = email
        .get("subject")
        .unwrap_or(&"Unknown Subject".to_string())
        .clone();
    let body = email
        .get("body-plain")
        .unwrap_or(&"No Content".to_string())
        .clone();
    let text = serde_json::to_string(&EmailMsg { title, body })?;

    let from = email
        .get("from")
        .unwrap_or(&"Unknown Sender".to_string())
        .clone();
    let msg_id = email
        .get("Message-Id")
        .unwrap_or(&"No Message-Id".to_string())
        .clone();

    Ok(IncomingMsg { text, from, msg_id })

    // let date = email
    //     .get("Date")
    //     .unwrap_or(&"No Message-Id".to_string())
    //     .clone();

    // let re = Regex::new(r"^(?P<name>[^<]+)\s*<(?P<email>[^>]+)>$").unwrap();
    // let captures = re.captures(&sender).context("cannot parse sender")?;

    // let sender_name = captures
    //     .name("name")
    //     .context("no sender name")?
    //     .as_str()
    //     .trim()
    //     .to_string();
    // let sender_email = captures
    //     .name("email")
    //     .context("no sender email")?
    //     .as_str()
    //     .trim()
    //     .to_string();
}

//         let resp = format!(
//             "{}\n\n{}\n\n> ------- Original Message -------\n> On {}, {} <{}> wrote:\n> \n> {}",
//             resp,
//             &CONFIG.email_config.as_ref().unwrap().signature,
//             parsed_email.date,
//             parsed_email.sender_name,
//             parsed_email.sender_email,
//             parsed_email.body.replace("\n", "\n> ")
//         );
