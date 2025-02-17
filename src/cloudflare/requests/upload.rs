// const data = "0".repeat(bytes);
// const options = {
//     headers: {
//         "Content-Length": Buffer.byteLength(data),
//     },
// };
//
// return request(options, data);

use crate::cloudflare::requests::{Request, RequestBody, UA};
use reqwest::header::{HeaderMap, CONTENT_LENGTH, USER_AGENT};
use reqwest::Method;
use std::borrow::Cow;

pub(crate) struct Upload {
    pub bytes: usize,
    data: String,
}

impl Upload {
    pub fn new(bytes: usize) -> Self {
        let body = "0".repeat(bytes);

        Self { bytes, data: body }
    }
}

impl Request for Upload {
    type Body = String;
    type Response = ();

    const METHOD: Method = Method::POST;

    fn endpoint(&self) -> Cow<str> {
        "/__up".into()
    }

    fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();

        headers.insert(USER_AGENT, UA.parse().unwrap());

        headers.insert(CONTENT_LENGTH, self.data.bytes().len().into());

        headers
    }

    fn body(&self) -> RequestBody<Self::Body> {
        RequestBody::Text(self.data.clone())
    }
}
