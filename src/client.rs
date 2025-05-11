//! Client module for fetching RFCs from the RFC Editor's website.
//!
//! Handles network requests for the RFC reader application.
use anyhow::{Context, Result};
use reqwest::Client;
use std::time::Duration;

const RFC_BASE_URL: &str = "https://www.rfc-editor.org/rfc/rfc";

/// Client for fetching RFCs
///
/// This client is used to fetch RFCs from the RFC Editor's website.
/// It is responsible for fetching the RFC index and RFCs.
#[derive(Default)]
pub struct RfcClient
{
    client: Client,
}

impl RfcClient
{
    /// Create a new RFC client.
    ///
    /// # Returns
    ///
    /// A new RFC client.
    ///
    /// # Panics
    ///
    /// Panics if the HTTP client cannot be created.
    #[must_use]
    pub fn new() -> Self
    {
        let client = Client::builder()
            .timeout(Duration::from_secs(30)) // Default timeout is 30 secs
            .build()
            .expect("Failed to create HTTP client");

        Self { client }
    }

    /// Fetch a specific RFC.
    ///
    /// # Arguments
    ///
    /// * `rfc_number` - The number of the RFC to fetch.
    ///
    /// # Returns
    ///
    /// The RFC content as a text.
    ///
    /// # Errors
    ///
    /// Returns an error if the RFC is not found or unavailable.
    pub async fn fetch_rfc(&self, rfc_number: u32) -> Result<String>
    {
        // RFC documents are available in TXT format
        let rfc_url = format!("{RFC_BASE_URL}{rfc_number}.txt");

        let response = self
            .client
            .get(&rfc_url)
            .send() // Send the GET request to the url.
            .await
            .context(format!("Failed to fetch RFC {rfc_number}"))?;

        if !response.status().is_success()
        {
            anyhow::bail!(
                "RFC {} not found or unavailable (status: {})",
                rfc_number,
                response.status()
            );
        }

        if let Ok(text) = response.text().await
        {
            // Strip the whitespace from the text.
            let text = text.trim();
            Ok(text.to_string())
        }
        else
        {
            anyhow::bail!("Failed to read text content for RFC {rfc_number}");
        }
    }

    /// Fetch the RFC index.
    ///
    /// # Returns
    ///
    /// The RFC index as a text.
    ///
    /// # Errors
    ///
    /// Returns an error if the RFC index is not available or if the request
    /// fails.
    pub async fn fetch_rfc_index(&self) -> Result<String>
    {
        // RFC index is available at a different URL
        let rfc_url: &'static str = "https://www.rfc-editor.org/rfc-index.txt";

        let response = self
            .client
            .get(rfc_url)
            .send()
            .await
            .context("Failed to fetch RFC index")?;

        if !response.status().is_success()
        {
            anyhow::bail!("RFC index not available (status: {})", response.status());
        }

        response
            .text()
            .await
            .context("Failed to read RFC index content")
    }
}
