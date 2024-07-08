use std::collections::HashMap;

use isahc::ReadResponseExt;
use once_cell::sync::Lazy;
use schemars::{schema::SchemaObject, schema_for, JsonSchema};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::json;

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
    map.insert(TransferPlus.name(), TransferPlus.erased());
    // map.insert(QueryChatHistoryDb.name(), QueryChatHistoryDb.erased());
    map
});

// ---------- Transfer Plus ------------
pub struct TransferPlus;

#[derive(JsonSchema, Deserialize)]
pub struct TransferPlusParams {
    old_uname: String,
    new_uname: String,
}

impl Tool for TransferPlus {
    type P = TransferPlusParams;

    fn call(&self, params: Self::P) -> anyhow::Result<String> {
        let mut res = isahc::post(
            "https://beegsquush.labooyah.be/support/transfer-plus",
            json!({
                "old_uname": params.old_uname,
                "new_uname": params.new_uname,
                "secret": CONFIG.tools_config.support_secret,
            })
            .to_string(),
        )?;
        if res.status().is_success() {
            Ok("Success".to_string())
        } else {
            let error_msg = res.text()?;
            anyhow::bail!(
                "Request failed with status: {}. Error message: {}",
                res.status(),
                error_msg
            )
        }
    }

    fn name(&self) -> String {
        "transfer_plus".to_string()
    }

    fn description(&self) -> String {
        "This tool transfer Plus time from one username to another. You should use this tool when a user has forgotten their password and needs to transfer their Plus time to a new account.".to_string()
    }
}

// ---------- Query Chat History DB -----------
pub struct QueryChatHistoryDb;

#[derive(JsonSchema, Deserialize)]
pub struct QueryChatHistoryDbParams {
    sql: String,
    secret: String,
}

impl Tool for QueryChatHistoryDb {
    type P = QueryChatHistoryDbParams;

    fn call(&self, params: Self::P) -> anyhow::Result<String> {
        if params.secret == CONFIG.tools_config.query_chat_history_db_secret {
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
        "This tool runs an arbitrary sql query on the internal chat history database, returning columns formatted in CSS. The db is an SQLite db with one table:

        chat_entries (
            thread TEXT,
            chat_entry BLOB

        You should use this tool when asked to give information about support history. The 'secret' field is a key that the user passes in.
        "
        .to_string()
    }
}
