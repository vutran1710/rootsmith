use base64::prelude::*;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum HttpClientError {
    #[error("Request failed: {0}")]
    RequestError(#[from] reqwest::Error),
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
}

#[derive(Clone)]
pub enum MultipartValue {
    Text(String),
    Bytes(Vec<u8>),
    File(Vec<u8>, Option<String>),
}

pub struct HttpClient {
    client: reqwest::Client,
    remote_url: String,
    headers: HeaderMap,
}

impl HttpClient {
    pub fn new(remote_url: impl Into<String>, headers: HeaderMap) -> Result<Self, HttpClientError> {
        let url = remote_url.into();

        let _ = url
            .parse::<reqwest::Url>()
            .map_err(|_| HttpClientError::InvalidUrl(url.clone()))?;

        let client = reqwest::Client::new();

        Ok(Self {
            client,
            remote_url: url,
            headers,
        })
    }

    pub fn remote_url(&self) -> &str {
        &self.remote_url
    }

    pub async fn get(&self, path: &str) -> Result<reqwest::Response, HttpClientError> {
        let url = format!("{}{}", self.remote_url, path);
        let response = self
            .client
            .get(&url)
            .headers(self.headers.clone())
            .send()
            .await?;

        Ok(response)
    }

    pub async fn post<T: serde::Serialize>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<reqwest::Response, HttpClientError> {
        let url = format!("{}{}", self.remote_url, path);
        let response = self
            .client
            .post(&url)
            .headers(self.headers.clone())
            .json(body)
            .send()
            .await?;

        Ok(response)
    }

    pub async fn put<T: serde::Serialize>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<reqwest::Response, HttpClientError> {
        let url = format!("{}{}", self.remote_url, path);
        let response = self
            .client
            .put(&url)
            .headers(self.headers.clone())
            .json(body)
            .send()
            .await?;

        Ok(response)
    }

    pub async fn delete(&self, path: &str) -> Result<reqwest::Response, HttpClientError> {
        let url = format!("{}{}", self.remote_url, path);
        let response = self
            .client
            .delete(&url)
            .headers(self.headers.clone())
            .send()
            .await?;

        Ok(response)
    }

    pub async fn post_multipart_with_bytes(
        &self,
        path: &str,
        fields: std::collections::HashMap<String, MultipartValue>,
        data: Vec<u8>,
        headers: Option<std::collections::HashMap<String, String>>,
    ) -> Result<reqwest::Response, HttpClientError> {
        let url = format!("{}{}", self.remote_url, path);

        let mut form = reqwest::multipart::Form::new();

        for (key, value) in fields {
            match value {
                MultipartValue::Text(text) => {
                    form = form.text(key, text);
                }
                MultipartValue::Bytes(bytes) => {
                    form = form.part(key, reqwest::multipart::Part::bytes(bytes));
                }
                MultipartValue::File(bytes, filename) => {
                    let mut part = reqwest::multipart::Part::bytes(bytes);
                    if let Some(fname) = filename {
                        part = part.file_name(fname);
                    }
                    form = form.part(key, part);
                }
            }
        }

        form = form.part("data", reqwest::multipart::Part::bytes(data));

        let mut request = self.client.post(&url);

        request = request.headers(self.headers.clone());

        if let Some(custom_headers) = headers {
            for (key, value) in custom_headers {
                let header_name = HeaderName::from_bytes(key.as_bytes()).map_err(|e| {
                    HttpClientError::InvalidUrl(format!("Invalid header name: {}", e))
                })?;
                let header_value = HeaderValue::from_str(&value).map_err(|e| {
                    HttpClientError::InvalidUrl(format!("Invalid header value: {}", e))
                })?;
                request = request.header(header_name, header_value);
            }
        }

        let response = request.multipart(form).send().await?;

        Ok(response)
    }

    pub async fn post_protobuf(
        &self,
        path: &str,
        custom_fields: std::collections::HashMap<String, MultipartValue>,
        data: Vec<u8>,
        schema_version: &str,
        schema_descriptor: Option<Vec<u8>>,
        schema_url: Option<&str>,
    ) -> Result<reqwest::Response, HttpClientError> {
        let mut fields = custom_fields;

        if let Some(descriptor) = schema_descriptor {
            fields.insert(
                "schema_descriptor".to_string(),
                MultipartValue::Text(BASE64_STANDARD.encode(descriptor)),
            );
        }

        if let Some(url) = schema_url {
            fields.insert(
                "schema_url".to_string(),
                MultipartValue::Text(url.to_string()),
            );
        }

        let mut headers = std::collections::HashMap::new();
        headers.insert("X-Format".to_string(), "protobuf".to_string());
        headers.insert("X-Schema-Version".to_string(), schema_version.to_string());

        self.post_multipart_with_bytes(path, fields, data, Some(headers))
            .await
    }

    pub fn with_header(
        mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, HttpClientError> {
        let header_name = HeaderName::from_bytes(key.into().as_bytes())
            .map_err(|e| HttpClientError::InvalidUrl(format!("Invalid header name: {}", e)))?;
        let header_value = HeaderValue::from_str(&value.into())
            .map_err(|e| HttpClientError::InvalidUrl(format!("Invalid header value: {}", e)))?;
        self.headers.insert(header_name, header_value);
        Ok(self)
    }
}
