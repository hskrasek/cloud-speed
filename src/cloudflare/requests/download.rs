use crate::cloudflare::requests::{Request, UA};
use reqwest::header::{HeaderMap, HeaderValue, CACHE_CONTROL, CONNECTION, USER_AGENT};
use std::borrow::Cow;

#[derive(Copy, Clone)]
pub(crate) struct Download {
    pub bytes: usize,
}

impl Request for Download {
    type Body = &'static str;
    type Response = String;

    fn endpoint(&self) -> Cow<str> {
        format!("/__down?bytes={}", self.bytes).into()
    }

    fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();

        headers.insert(USER_AGENT, UA.parse().unwrap());

        headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-cache"));

        headers.insert(CONNECTION, HeaderValue::from_static("close"));

        headers
    }
}
