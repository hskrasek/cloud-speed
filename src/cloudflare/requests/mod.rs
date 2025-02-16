extern crate serde;

pub mod trace;

use reqwest::{header::{HeaderMap, USER_AGENT}, Method};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

const NAME: &str = env!("CARGO_PKG_NAME");
const VERSION: &str = env!("CARGO_PKG_VERSION");
const REPO: &str = env!("CARGO_PKG_REPOSITORY");

pub trait Request {
    type Body: Serialize;

    type Response: for<'de> Deserialize<'de>;

    const METHOD: Method = Method::GET;

    fn endpoint(&self) -> Cow<str>;

    fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        
        headers.insert(
            USER_AGENT,
            format!("{}/{} ({})", NAME, VERSION, REPO).parse().unwrap(),
        );
        
        headers
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
}
