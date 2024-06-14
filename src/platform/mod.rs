mod email;
mod telegram;
pub use email::Email;
pub use telegram::Telegram;

use async_trait::async_trait;

#[derive(Debug)]
pub struct IncomingMsg {
    pub text: String,
    pub from: String,
    pub msg_id: String,
}

#[derive(Debug)]
pub struct OutgoingMsg {
    pub text: String,
    pub to: String,
    pub in_reply_to: Option<String>,
}

#[async_trait]
pub trait Platform {
    async fn send_msg(&self, outgoing_msg: &OutgoingMsg) -> anyhow::Result<()>;
    async fn recv_msg(&self) -> IncomingMsg;
}
