use std::collections::HashMap;

use crate::lsp::Error;

use super::Result;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::value::Value;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Response {
    // the headers for the response
    #[serde(skip, default)]
    pub headers: HashMap<String, String>,

    // The version, this should always be 2.0
    #[serde(rename(deserialize = "jsonrpc"))]
    version: String,

    // The id
    pub id: Option<u32>,

    // The result, which we store the data for in a raw
    // value to be requested at query time
    #[serde(flatten)]
    payload: ResponsePayload,
}

#[derive(Deserialize, Debug, Clone, Serialize)]
pub enum ResponsePayload {
    #[serde(rename = "result")]
    Result(Value),
    #[serde(rename = "error")]
    Error { code: isize, message: String },
}

impl Response {
    pub fn new(headers: HashMap<String, String>, content: &[u8]) -> Result<Self> {
        let mut response: Response = serde_json::from_slice(content)?;
        response.headers = headers;
        Ok(response)
    }

    pub fn result<D: DeserializeOwned>(self) -> Result<D> {
        match self.payload {
            ResponsePayload::Result(result) => Ok(serde_json::from_value(result)?),
            ResponsePayload::Error { code: _, message } => Err(Error::LSPError(message)),
        }
    }
}
