use std::fmt::Debug;
use std::net::{IpAddr, Ipv4Addr};

use http::Uri;
use http::uri::{PathAndQuery, Scheme};

use super::{ApiClient, ApiClientError};

#[derive(Debug, Clone)]
pub struct ApiClientBuilder {
    client: reqwest::Client,
    scheme: Scheme,
    host: String,
    port: u16,
    base_path: Option<PathAndQuery>,
}

impl ApiClientBuilder {
    pub fn build(self) -> Result<ApiClient, ApiClientError> {
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
            builder.path_and_query("/")
        };

        let base_uri = builder.build()?;
        let base_path = base_path
            .as_ref()
            .map(|it| it.path().to_string())
            .unwrap_or_default();

        let collectors = Default::default();

        Ok(ApiClient {
            client,
            base_uri,
            base_path,
            collectors,
        })
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

    pub fn base_path<P>(mut self, base_path: P) -> Result<Self, ApiClientError>
    where
        P: TryInto<PathAndQuery>,
        P::Error: Debug + 'static,
    {
        let base_path = base_path
            .try_into()
            .map_err(|err| ApiClientError::InvalidBasePath {
                error: format!("{err:?}"),
            })?;
        self.base_path = Some(base_path);
        Ok(self)
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

#[cfg(test)]
mod tests {
    use super::*;
    use http::uri::Scheme;

    #[test]
    fn test_default_builder_creates_localhost_http_client() {
        let client = ApiClientBuilder::default()
            .build()
            .expect("should build client");

        let uri = client.base_uri.to_string();
        insta::assert_snapshot!(uri, @"http://127.0.0.1:80/");
    }

    #[test]
    fn test_builder_with_custom_scheme() {
        let client = ApiClientBuilder::default()
            .scheme(Scheme::HTTPS)
            .build()
            .expect("should build client");

        let uri = client.base_uri.to_string();
        insta::assert_snapshot!(uri, @"https://127.0.0.1:80/");
    }

    #[test]
    fn test_builder_with_custom_host() {
        let client = ApiClientBuilder::default()
            .host("api.example.com")
            .build()
            .expect("should build client");

        let uri = client.base_uri.to_string();
        insta::assert_snapshot!(uri, @"http://api.example.com:80/");
    }

    #[test]
    fn test_builder_with_custom_port() {
        let client = ApiClientBuilder::default()
            .port(8080)
            .build()
            .expect("should build client");

        let uri = client.base_uri.to_string();
        insta::assert_snapshot!(uri, @"http://127.0.0.1:8080/");
    }

    #[test]
    fn test_builder_with_valid_base_path() {
        let client = ApiClientBuilder::default()
            .base_path("/api/v1")
            .expect("valid base path")
            .build()
            .expect("should build client");

        insta::assert_debug_snapshot!(client.base_path, @r#""/api/v1""#);
    }

    #[test]
    fn test_builder_with_invalid_base_path_warns_and_continues() {
        let result = ApiClientBuilder::default().base_path("invalid path with spaces");
        assert!(result.is_err());
    }
}
