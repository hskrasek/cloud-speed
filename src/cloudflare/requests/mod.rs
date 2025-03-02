extern crate serde;

pub mod download;
pub mod locations;
pub mod meta;
pub mod upload;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RequestBody<T> {
    None,
    Text(T),
}

impl<T> Default for RequestBody<T> {
    fn default() -> Self {
        RequestBody::None
    }
}

pub trait Request {
    type Body: Into<Body>;

    type Response: for<'de> Deserialize<'de>;

    const METHOD: Method = Method::GET;

    fn endpoint(&self) -> Cow<str>;

    fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();

        headers.insert(USER_AGENT, UA.parse().unwrap());

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

    fn endpoint(&self) -> Cow<str> {
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

    fn endpoint(&self) -> Cow<str> {
        (**self).endpoint()
    }

    fn headers(&self) -> HeaderMap {
        (**self).headers()
    }

    fn body(&self) -> RequestBody<Self::Body> {
        (**self).body()
    }
}
