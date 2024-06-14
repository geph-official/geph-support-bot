use async_trait::async_trait;

#[derive(Debug)]
pub struct PlatformMsg {
    pub text: String,
    pub from: String,
    pub msg_id: String,
}
#[async_trait]
pub trait Platform {
    async fn send_msg(
        &self,
        msg: String,
        to: String,
        in_reply_to: Option<String>,
    ) -> anyhow::Result<()>;
    async fn recv_msg(&self) -> PlatformMsg;
}
