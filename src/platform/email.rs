use std::{collections::HashMap, sync::RwLock, time::Duration};

use anyhow::Context;
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
    recv_msgs: smol::channel::Receiver<(IncomingMsg, String)>,
    email_to_title: RwLock<HashMap<String, String>>,
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
                        Ok((msg, title)) => {
                            let _ = send_msgs.send((msg, title)).await;
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
            email_to_title: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl Platform for Email {
    async fn send_msg(&self, outgoing_msg: &OutgoingMsg) -> anyhow::Result<()> {
        let title = "RE: ".to_owned()
            + &self
                .email_to_title
                .read()
                .unwrap()
                .get(&outgoing_msg.to)
                .context("no 'to' field in outgoing email msg")?;

        static MAILGUN_LIMIT: Lazy<Semaphore> = Lazy::new(|| Semaphore::new(16));
        let _guard = MAILGUN_LIMIT.acquire().await;

        let text = format!(
            "From GephSupportBot\n来自迷雾通客服机器人：\n\n{}\n\n---\nIf GephSupportBot cannot resolve the issue, our human support will respond within 48 hours. Thanks for your patience!\n如果机器人不能解决您的问题，我们的人工客服会在48小时以内回复您。请耐心等待！",
            outgoing_msg.text.clone()
        );

        let mut params = vec![
            ("from".to_string(), self.config.address.clone()),
            ("to".to_string(), outgoing_msg.to.to_string()),
            ("subject".to_string(), title),
            ("text".to_string(), text),
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
        let (msg, title) = self.recv_msgs.recv().await.unwrap();
        self.email_to_title
            .write()
            .unwrap()
            .insert(msg.from.clone(), title);
        msg
    }
}

fn parse_email(email: HashMap<String, String>) -> anyhow::Result<(IncomingMsg, String)> {
    let title = email
        .get("subject")
        .unwrap_or(&"Unknown Subject".to_string())
        .clone();
    let body = email
        .get("body-plain")
        .unwrap_or(&"No Content".to_string())
        .clone();
    let text = serde_json::to_string(&EmailMsg {
        title: title.clone(),
        body,
    })?;

    let from = email
        .get("from")
        .unwrap_or(&"Unknown Sender".to_string())
        .clone();
    let msg_id = email
        .get("Message-Id")
        .unwrap_or(&"No Message-Id".to_string())
        .clone();

    log::debug!("got an email from {from}");

    Ok((IncomingMsg { text, from, msg_id }, title))

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
