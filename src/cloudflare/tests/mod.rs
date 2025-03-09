use reqwest::Method;
use std::borrow::Cow;
use std::error::Error;
use std::io::{Read, Write};
use std::time::Duration;

pub(crate) mod download;

pub(crate) static BASE_URL: &str = "https://speed.cloudflare.com";

pub trait IoReadAndWrite: Read + Write {}

impl<T: Read + Write> IoReadAndWrite for T {}

pub(crate) trait Test {
    const METHOD: Method = Method::GET;

    fn endpoint(&self) -> Cow<str>;

    async fn run(&self, bytes: u64) -> Result<TestResults, Box<dyn Error>>;
}

impl<T: Test> Test for &T {
    const METHOD: Method = T::METHOD;

    fn endpoint(&self) -> Cow<str> {
        (**self).endpoint()
    }

    async fn run(&self, bytes: u64) -> Result<TestResults, Box<dyn Error>> {
        (**self).run(bytes).await
    }
}

impl<T: Test> Test for &mut T {
    const METHOD: Method = T::METHOD;

    fn endpoint(&self) -> Cow<str> {
        (**self).endpoint()
    }

    async fn run(&self, bytes: u64) -> Result<TestResults, Box<dyn Error>> {
        (**self).run(bytes).await
    }
}

#[derive(Debug)]
pub(crate) struct TestResults {
    pub dns_duration: Duration,
    pub tcp_duration: Duration,
    pub tls_handshake_duration: Duration,
    pub connect_duration: Duration,
    pub ttfb_duration: Duration,
    pub request_duration: Duration,
    pub end_duration: Duration,
}

impl TestResults {
    pub(crate) const fn new(
        dns_duration: Duration,
        tcp_duration: Duration,
        tls_handshake_duration: Duration,
        connect_duration: Duration,
        ttfb_duration: Duration,
        request_duration: Duration,
        end_duration: Duration,
    ) -> Self {
        TestResults {
            dns_duration,
            tcp_duration,
            tls_handshake_duration,
            connect_duration,
            ttfb_duration,
            request_duration,
            end_duration,
        }
    }
}
