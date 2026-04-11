use serde::{Serialize, ser::SerializeMap};

use super::Result;

pub struct Notification<T: Serialize> {
    method: String,
    content: T,
}

impl<T: Serialize> Notification<T> {
    pub fn from_serializable<S>(method: S, value: T) -> Result<Self>
    where
        S: Into<String>,
    {
        Ok(Notification {
            method: method.into(),
            content: value,
        })
    }
}

impl<T: Serialize> Serialize for Notification<T> {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(4))?;
        map.serialize_entry("jsonrpc", "2.0")?;
        map.serialize_entry("method", &self.method)?;
        map.serialize_entry("params", &self.content)?;
        map.end()
    }
}
