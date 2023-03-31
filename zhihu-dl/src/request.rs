use reqwest::{self as req, IntoUrl, Method};
use reqwest_cookie_store::CookieStoreMutex;
use std::{sync::Arc, time::Duration};

pub struct Client {
    pub(crate) http_client: req::Client,
    pub(crate) request_interval: Duration,
    cookie_store: Arc<CookieStoreMutex>,
}

pub(crate) trait Signer {
    fn sign_request<U>(client: &Client, method: Method, url: U) -> req::RequestBuilder
    where
        U: IntoUrl;
}

pub(crate) struct NoSign;
impl Signer for NoSign {
    fn sign_request<U: IntoUrl>(client: &Client, method: Method, url: U) -> req::RequestBuilder {
        client.http_client.request(method, url)
    }
}

mod zse96_v3;
pub use zse96_v3::Zse96V3;

impl Client {
    pub fn new() -> Self {
        Self::with_http_client(req::ClientBuilder::new()).unwrap()
    }
    pub fn with_http_client(client_builder: req::ClientBuilder) -> Result<Self, reqwest::Error> {
        let cookie_store = Arc::new(CookieStoreMutex::default());
        Ok(Self {
            http_client: client_builder
                .cookie_provider(cookie_store.clone())
                .build()?,
            request_interval: Duration::from_secs(5),
            cookie_store,
        })
    }
    pub async fn init(&self) -> Result<(), reqwest::Error> {
        self.http_client
            .get("https://www.zhihu.com/explore")
            .send()
            .await
            .map(|_| ())
    }
    pub(crate) fn request_signed<S: Signer, U: IntoUrl>(
        &self,
        method: Method,
        url: U,
    ) -> req::RequestBuilder {
        S::sign_request(self, method, url)
    }
}
impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) mod paging;
