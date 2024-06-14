mod email;
mod telegram;
pub use email::Email;
pub use telegram::Telegram;

use async_trait::async_trait;

#[derive(Debug)]
pub struct PlatformMsg {
    pub text: String,
    pub from: String,
    pub msg_id: String,
}
#[async_trait]
pub trait Platform {
    async fn send_msg(&self, msg: &str, to: &str, in_reply_to: Option<&str>) -> anyhow::Result<()>;
    async fn recv_msg(&self) -> PlatformMsg;
}
