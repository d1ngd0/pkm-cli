use std::collections::HashMap;

use super::Result;
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

#[derive(Serialize, Deserialize, Debug)]
pub struct Response {
    // the headers for the response
    #[serde(skip)]
    pub headers: HashMap<String, String>,

    // The version, this should always be 2.0
    #[serde(rename(deserialize = "jsonrpc"))]
    version: String,

    // The id
    pub id: u32,

    // The result, which we store the data for in a raw
    // value to be requested at query time
    result: Box<RawValue>,
}

impl Response {
    pub fn new(headers: HashMap<String, String>, content: &[u8]) -> Result<Self> {
        let mut response: Response = serde_json::from_slice(content)?;
        response.headers = headers;
        Ok(response)
    }

    pub fn result<'a, D: Deserialize<'a>>(&'a self) -> Result<D> {
        Ok(serde_json::from_str(self.result.get())?)
    }
}
