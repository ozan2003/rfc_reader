use anyhow::{Context, Result};
use reqwest::Client;
use std::time::Duration;

const RFC_BASE_URL: &str = "https://www.rfc-editor.org/rfc/rfc";

pub struct RfcClient {
    client: Client,
}

impl RfcClient {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");
            
        Self { client }
    }
    
    pub async fn fetch_rfc(&self, rfc_number: u32) -> Result<String> {
        // RFC documents are available in TXT format
        let url = format!("{}{}.txt", RFC_BASE_URL, rfc_number);
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .context(format!("Failed to fetch RFC {}", rfc_number))?;
            
        if !response.status().is_success() {
            anyhow::bail!("RFC {} not found or unavailable (status: {})", 
                rfc_number, response.status());
        }
        
        response.text().await
            .context(format!("Failed to read text content for RFC {}", rfc_number))
    }
    
    pub async fn fetch_rfc_index(&self) -> Result<String> {
        // RFC index is available at a different URL
        let url = "https://www.rfc-editor.org/rfc-index.txt";
        
        let response = self.client
            .get(url)
            .send()
            .await
            .context("Failed to fetch RFC index")?;
            
        if !response.status().is_success() {
            anyhow::bail!("RFC index not available (status: {})", response.status());
        }
        
        response.text().await
            .context("Failed to read RFC index content")
    }
}