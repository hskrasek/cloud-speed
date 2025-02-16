use crate::cloudflare::requests::Request;
use reqwest::{header, Client as ReqwestClient};

static BASE_URL: &str = "https://speed.cloudflare.com";

#[derive(Debug, Clone)]
pub struct Client {
    client: ReqwestClient,
}

impl Client {
    pub fn new() -> Self {
        Client {
            client: ReqwestClient::new(),
        }
    }

    pub async fn send<R: Request>(&self, request: R) -> Result<R::Response, Box<dyn std::error::Error>> {
        let endpoint = request.endpoint();
        let endpoint = endpoint.trim_matches('/');
        let url = format!("{}/{}", BASE_URL, endpoint);

        let response = self.client
            .request(R::METHOD, &url)
            .headers(request.headers())
            .send()
            .await?
            .error_for_status()?;

        if let Some(ct_value) = response.headers().get(header::CONTENT_TYPE) {
            if let Ok(content_type) = ct_value.to_str() {
                if content_type.starts_with("application/json") {
                    return response.json::<R::Response>().await.map_err(Into::into)
                }
            }
        }
        
        let text = response.text().await?;
        let deserialized = serde_plain::from_str(&text)?;
        
        Ok(deserialized)
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}
