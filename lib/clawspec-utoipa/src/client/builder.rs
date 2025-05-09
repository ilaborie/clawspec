use std::fmt::Debug;
use std::net::{IpAddr, Ipv4Addr};

use http::Uri;
use http::uri::{PathAndQuery, Scheme};
use tracing::warn;

use super::ApiClient;

#[derive(Clone)]
pub struct ApiClientBuilder {
    client: reqwest::Client,
    scheme: Scheme,
    host: String,
    port: u16,
    base_path: Option<PathAndQuery>,
}

impl ApiClientBuilder {
    pub fn build(self) -> ApiClient {
        let Self {
            client,
            scheme,
            host,
            port,
            base_path,
        } = self;

        let builder = Uri::builder()
            .scheme(scheme)
            .authority(format!("{host}:{port}"));
        let builder = if let Some(path) = &base_path {
            builder.path_and_query(path.path())
        } else {
            builder
        };

        let base_uri = builder.build().expect("a valid URI");
        let base_path = base_path
            .as_ref()
            .map(|it| it.path().to_string())
            .unwrap_or_default();

        let collectors = Default::default();

        ApiClient {
            client,
            base_uri,
            base_path,
            collectors,
        }
    }

    pub fn scheme(mut self, scheme: Scheme) -> Self {
        self.scheme = scheme;
        self
    }

    pub fn host(mut self, host: impl Into<String>) -> Self {
        self.host = host.into();
        self
    }

    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub fn base_path<P>(mut self, base_path: P) -> Self
    where
        P: TryInto<PathAndQuery>,
        P::Error: Debug,
    {
        match base_path.try_into() {
            Ok(base_path) => {
                self.base_path = Some(base_path);
            }
            Err(error) => {
                warn!(?error, "invalid base path");
            }
        }
        self
    }
}

impl Default for ApiClientBuilder {
    fn default() -> Self {
        Self {
            client: reqwest::Client::new(),
            scheme: Scheme::HTTP,
            host: IpAddr::V4(Ipv4Addr::LOCALHOST).to_string(),
            port: 80,
            base_path: None,
        }
    }
}
