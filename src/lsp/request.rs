use serde::{Serialize, ser::SerializeMap};

use super::{RequestID, Result};

pub struct Request<T: Serialize> {
    pub id: RequestID,
    method: String,
    content: T,
}

impl<T: Serialize> Request<T> {
    pub fn from_serializable(method: &str, value: T) -> Result<Self> {
        Ok(Request {
            id: 0,
            method: method.into(),
            content: value,
        })
    }
}

impl<T: Serialize> Serialize for Request<T> {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(4))?;
        map.serialize_entry("jsonrpc", "2.0")?;
        map.serialize_entry("id", &self.id)?;
        map.serialize_entry("method", &self.method)?;
        map.serialize_entry("params", &self.content)?;
        map.end()
    }
}
