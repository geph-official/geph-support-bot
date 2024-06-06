use std::collections::HashMap;

use once_cell::sync::Lazy;
use schemars::{schema::SchemaObject, schema_for, JsonSchema};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::{json, Value};

pub trait Tool: Sized + 'static {
    type P: JsonSchema + DeserializeOwned;

    fn call(&self, params: Self::P) -> anyhow::Result<Value>;

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
    pub call: Box<dyn Fn(serde_json::Value) -> anyhow::Result<serde_json::Value>>,
}

pub const TOOLS: Lazy<HashMap<String, ErasedTool>> = Lazy::new(|| {
    let mut map = HashMap::new();
    map.insert(AddTwoNumbers.name(), AddTwoNumbers.erased());
    map
});

pub struct AddTwoNumbers;

#[derive(JsonSchema, Deserialize)]
pub struct AddTwoNumbersParams {
    num1: i32,
    num2: i32,
}

impl Tool for AddTwoNumbers {
    type P = AddTwoNumbersParams;

    fn call(&self, params: Self::P) -> anyhow::Result<Value> {
        Ok(json!(params.num1 + params.num2))
    }

    fn name(&self) -> String {
        "add_two_numbers".to_string()
    }

    fn description(&self) -> String {
        "adds two int32 numbers, returning the result".to_string()
    }
}
