use std::collections::HashMap;

use once_cell::sync::Lazy;
use schemars::{schema::SchemaObject, schema_for, JsonSchema};
use serde::{de::DeserializeOwned, Deserialize};

use crate::{CONFIG, DB};

pub trait Tool: Sized + 'static {
    type P: JsonSchema + DeserializeOwned;

    fn call(&self, params: Self::P) -> anyhow::Result<String>;

    fn name(&self) -> String;

    fn description(&self) -> String;

    fn erased(self) -> ErasedTool {
        ErasedTool {
            name: self.name(),
            description: self.description(),
            parameters: schema_for!(Self::P).schema,
            call: Box::new(move |val| {
                let param: Self::P = serde_json::from_value(val)?;
                let result = self.call(param)?;
                Ok(result)
            }),
        }
    }
}

pub struct ErasedTool {
    pub name: String,
    pub description: String,
    pub parameters: SchemaObject,
    pub call: Box<dyn Fn(serde_json::Value) -> anyhow::Result<String>>,
}

pub const TOOLS: Lazy<HashMap<String, ErasedTool>> = Lazy::new(|| {
    let mut map = HashMap::new();
    map.insert(QueryChatHistoryDb.name(), QueryChatHistoryDb.erased());
    map.insert(ContactManager.name(), ContactManager.erased());
    map.insert(SendManagerResponse.name(), SendManagerResponse.erased());
    map
});

pub struct QueryChatHistoryDb;

#[derive(JsonSchema, Deserialize)]
pub struct QueryChatHistoryDbParams {
    sql: String,
    secret: String,
}

impl Tool for QueryChatHistoryDb {
    type P = QueryChatHistoryDbParams;

    fn call(&self, params: Self::P) -> anyhow::Result<String> {
        const query_chat_history_db_secret: &str = "lolsecret";
        if params.secret == query_chat_history_db_secret {
            let res = smolscale::block_on(async move { DB.query(&params.sql).await })?;
            // println!("QueryChatHistoryDb.call() = {res}");
            Ok(res)
        } else {
            anyhow::bail!("Unauthorized.");
        }
    }

    fn name(&self) -> String {
        "query_chat_history_db".to_string()
    }

    fn description(&self) -> String {
        "This tool runs an arbitrary sql query on the internal chat history database, returning columns formatted in CSS. The db is an SQLite db with two tables:

        conversations (
            convo_id BIGINT PRIMARY KEY,
            metadata BLOB
        )

        chat_entries (
            convo_id BIGINT,
            chat_entry BLOB,
            FOREIGN KEY(convo_id) REFERENCES conversations(convo_id)
        )

        You should use this tool when asked to give information about support history, such as giving reports about support requests over the past day. The 'secret' field is a key that the user passes in.
        "
        .to_string()
    }
}

pub struct ContactManager;

#[derive(JsonSchema, Deserialize)]
pub struct ContactManagerParams {
    msg: String,
    req_convo_id: i64,
    req_chat_id: i64,
    req_reply_to_msg_id: i64,
}

impl Tool for ContactManager {
    type P = ContactManagerParams;

    fn call(&self, params: Self::P) -> anyhow::Result<String> {
        // let admin_chat_convo_id = CONFIG.telegram_config.as_ref().unwrap().admin_chat_id;
        // smolscale::block_on(send_telegram_msg(
        //     format!(
        //         "Received a request from a different chat with req_chat_id = {}, req_convo_id: {}, req_reply_to_msg_id = {}:\n\n{}.\n\nRespond to this chat to send a manager response with the above parameters.",
        //         params.req_chat_id, params.req_convo_id, params.req_reply_to_msg_id, params.msg
        //     ),
        //     admin_chat_convo_id,
        //     admin_chat_convo_id,
        //     None,
        // ))
        // .map(|_| "success".to_string())
        Ok("success".to_string())
    }

    fn name(&self) -> String {
        "contact_manager".to_string()
    }

    fn description(&self) -> String {
        "Sends a msg to manager on behalf of user with chat_id and latest msg id reply_to_msg_id. Use this tool when the user asks to speak with human support, when they need to transfer plus time or when Plus time they bought didn't show up, or when they have questions you cannot resolve. The msg field should contain, verbatim, as much of the information from the user as possible, without rewording what the user says too much."
            .to_string()
    }
}

pub struct SendManagerResponse;

#[derive(JsonSchema, Deserialize)]
pub struct SendManagerResponseParams {
    manager_msg: String,
    req_chat_id: i64,
    req_convo_id: i64,
    req_reply_to_msg_id: i64,
}

impl Tool for SendManagerResponse {
    type P = SendManagerResponseParams;

    // todo: support email
    fn call(&self, params: Self::P) -> anyhow::Result<String> {
        // smolscale::block_on(send_telegram_msg(
        //     format!("The manager has responded:\n\n{}", params.manager_msg),
        //     params.req_convo_id,
        //     params.req_chat_id,
        //     Some(params.req_reply_to_msg_id),
        // ))
        // .map(|_| "success".to_string())
        Ok("success".to_string())
    }

    fn name(&self) -> String {
        "send_manager_response".to_string()
    }

    fn description(&self) -> String {
        "Responds to user with req_chat_id, req_convo_id, and req_reply_to_msg_id, on behalf of manager. These parameters should be reconstructed from the chat history by looking at the parameters of the last contact_manager tool call. You should ignore all other parameters in the history. Call this when manager responds to your request to contact manager. ".to_string()
    }
}
