extern crate serde;

use crate::cloudflare::requests::Request;
use serde::de::{Error, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::borrow::Cow;
use std::fmt::Formatter;
use structmap::FromMap;
use structmap_derive::FromMap;

#[derive(FromMap, Debug, Serialize)]
pub(crate) struct Trace {
    fl: String,
    h: String,
    pub ip: String,
    ts: String,
    visit_scheme: String,
    uag: String,
    pub colo: String,
    sliver: String,
    http: String,
    pub loc: String,
    tls: String,
    sni: String,
    warp: String,
    gateway: String,
    rbi: String,
    kex: String,
}

impl Default for Trace {
    fn default() -> Self {
        Self {
            fl: "".to_string(),
            h: "".to_string(),
            ip: "".to_string(),
            ts: "".to_string(),
            visit_scheme: "".to_string(),
            uag: "".to_string(),
            colo: "".to_string(),
            sliver: "".to_string(),
            http: "".to_string(),
            loc: "".to_string(),
            tls: "".to_string(),
            sni: "".to_string(),
            warp: "".to_string(),
            gateway: "".to_string(),
            rbi: "".to_string(),
            kex: "".to_string(),
        }
    }
}

pub(crate) struct TraceRequest {}

impl Request for TraceRequest {
    type Body = &'static str;

    type Response = Trace;

    fn endpoint(&self) -> Cow<str> {
        "cdn-cgi/trace".into()
    }
}

impl<'de> Deserialize<'de> for Trace {
    fn deserialize<D>(deserializer: D) -> Result<Trace, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(TraceVisitor)
    }
}

struct TraceVisitor;

impl<'de> Visitor<'de> for TraceVisitor {
    type Value = Trace;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        formatter.write_str("a newline-separated list of key=value pairs")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        let properties: StringMap = v
            .split("\n")
            .filter_map(|property| {
                let mut split = property.split("=");

                if let (Some(key), Some(value)) = (split.next(), split.next()) {
                    Some((key.to_string(), value.to_string()))
                } else {
                    None
                }
            })
            .collect();

        Ok(Trace::from_stringmap(properties))
    }
}
