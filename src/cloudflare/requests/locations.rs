extern crate serde;

use crate::cloudflare::requests::Request;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Deserialize, Debug)]
pub struct LocationsResponse(Vec<Location>);

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Location {
    pub iata: String,
    #[serde(rename(serialize = "lat"))]
    pub _lat: f64,
    #[serde(rename(serialize = "lon"))]
    pub _lon: f64,
    pub city: String,
    #[serde(rename(serialize = "region"))]
    pub _region: String,
}

pub(crate) struct Locations {}

impl Request for Locations {
    type Body = &'static str;

    type Response = LocationsResponse;

    fn endpoint(&self) -> Cow<str> {
        "/locations".into()
    }
}

impl LocationsResponse {
    pub(crate) fn get(self, iata: &str) -> Location {
        self.0
            .into_iter()
            .find(|loc| loc.iata == iata)
            .expect("Location {} not found")
    }
}
