extern crate serde;

use crate::cloudflare::requests::Request;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

/// Cloudflare datacenter (colo) information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Colo {
    /// IATA airport code for the datacenter location
    pub iata: String,
    /// Latitude of the datacenter
    pub lat: f64,
    /// Longitude of the datacenter
    pub lon: f64,
    /// ISO 3166-1 alpha-2 country code
    pub cca2: String,
    /// Geographic region name
    pub region: String,
    /// City name
    pub city: String,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct Meta {
    pub hostname: String,
    #[serde(rename = "clientIp")]
    pub client_ip: String,
    #[serde(rename = "httpProtocol")]
    pub http_protocol: String,
    pub asn: i64,
    #[serde(rename = "asOrganization")]
    pub as_organization: String,
    pub colo: Colo,
    pub country: String,
    pub city: String,
    pub region: String,
    #[serde(rename = "postalCode")]
    pub postal_code: String,
    pub latitude: String,
    pub longitude: String,
}

pub(crate) struct MetaRequest {}

impl Request for MetaRequest {
    type Body = &'static str;

    type Response = Meta;

    fn endpoint(&'_ self) -> Cow<'_, str> {
        "/meta".into()
    }
}
