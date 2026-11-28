use crate::{ApiError, ClientConfig, NetError};
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct ApiClient {
    client: reqwest::Client,
    base_url: String,
    session_token: Option<String>,
}

impl ApiClient {
    pub fn new(cfg: &ClientConfig) -> Result<Self, NetError> {
        let client = Client::builder()
            .build()
            .map_err(|e| NetError::Network(e.to_string()))?;
        
        Ok(Self {
            client,
            base_url: cfg.base_url.clone(),
            session_token: cfg.session_token.clone(),
        })
    }
    
    pub async fn get<T>(&self, path: &str, _q: &[(&str, String)]) -> Result<T, ApiError>
    where
        T: DeserializeOwned,
    {
        let url = format!("{}/{}", self.base_url, path);
        
        let resp = self.client.get(&url)
            .send()
            .await
            .map_err(|e| ApiError::Http(500, e.to_string()))?;
        
        let status = resp.status().as_u16();
        let body = resp.text().await.map_err(|e| ApiError::Http(status, e.to_string()))?;
        
        if status != 200 {
            return Err(ApiError::Http(status, body));
        }
        
        serde_json::from_str(&body)
            .map_err(|e| ApiError::Serialization(e.to_string()))
    }
    
    pub async fn post<B, T>(&self, path: &str, body: &B) -> Result<T, ApiError>
    where
        B: Serialize + ?Sized,
        T: DeserializeOwned,
    {
        let url = format!("{}/{}", self.base_url, path);
        
        let resp = self.client.post(&url)
            .json(body)
            .send()
            .await
            .map_err(|e| ApiError::Http(500, e.to_string()))?;
        
        let status = resp.status().as_u16();
        let body = resp.text().await.map_err(|e| ApiError::Http(status, e.to_string()))?;
        
        if status != 200 {
            return Err(ApiError::Http(status, body));
        }
        
        serde_json::from_str(&body)
            .map_err(|e| ApiError::Serialization(e.to_string()))
    }
    
    pub async fn put_bytes(&self, _path: &str, _body: Vec<u8>, _sha256_hex: &str) -> Result<(), ApiError> {
        Ok(())
    }
    
    pub async fn get_bytes_range(&self, _path: &str, _from: u64, _to: u64) -> Result<Vec<u8>, ApiError> {
        Ok(Vec::new())
    }
}

use serde::de::DeserializeOwned;

impl From<reqwest::Error> for ApiError {
    fn from(e: reqwest::Error) -> Self {
        ApiError::Serialization(e.to_string())
    }
}
