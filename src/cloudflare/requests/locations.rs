extern crate serde;

use crate::cloudflare::requests::Request;
use serde::Deserialize;
use std::borrow::Cow;

#[derive(Deserialize, Debug)]
pub struct LocationsResponse(Vec<Location>);

#[derive(Debug, Deserialize)]
pub(crate) struct Location {
    pub iata: String,
    pub lat: f64,
    pub lon: f64,
    pub region: String,
    pub city: String,
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
