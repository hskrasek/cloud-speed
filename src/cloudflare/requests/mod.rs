extern crate serde;

pub mod locations;
pub mod meta;

use http::header::REFERER;
use http::HeaderValue;
use reqwest::{
    header::{HeaderMap, USER_AGENT},
    Body, Method,
};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

pub const UA: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("CARGO_PKG_REPOSITORY"),
    ")"
);

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default,
)]
pub enum RequestBody<T> {
    #[default]
    None,
    Text(T),
}

pub trait Request {
    type Body: Into<Body>;

    type Response: for<'de> Deserialize<'de>;

    const METHOD: Method = Method::GET;

    fn endpoint(&'_ self) -> Cow<'_, str>;

    fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();

        headers.insert(USER_AGENT, UA.parse().unwrap());
        headers.insert(
            REFERER,
            HeaderValue::from_static("https://speed.cloudflare.com/"),
        );

        headers
    }

    fn body(&self) -> RequestBody<Self::Body> {
        Default::default()
    }
}

impl<R: Request> Request for &R {
    type Body = R::Body;
    type Response = R::Response;

    const METHOD: Method = R::METHOD;

    fn endpoint(&'_ self) -> Cow<'_, str> {
        (**self).endpoint()
    }

    fn headers(&self) -> HeaderMap {
        (**self).headers()
    }

    fn body(&self) -> RequestBody<Self::Body> {
        (**self).body()
    }
}

impl<R: Request> Request for &mut R {
    type Body = R::Body;
    type Response = R::Response;

    const METHOD: Method = R::METHOD;

    fn endpoint(&'_ self) -> Cow<'_, str> {
        (**self).endpoint()
    }

    fn headers(&self) -> HeaderMap {
        (**self).headers()
    }

    fn body(&self) -> RequestBody<Self::Body> {
        (**self).body()
    }
}
