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
            thread BIGINT,
            chat_entry BLOB

        You should use this tool when asked to give information about support history. The 'secret' field is a key that the user passes in.
        "
        .to_string()
    }
}
