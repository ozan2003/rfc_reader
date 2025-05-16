//! Client module for fetching RFCs from the RFC Editor's website.
//!
//! Handles network requests for the RFC reader application.
use anyhow::{Context, Result};
use std::io::Read;
use std::time::Duration;
use ureq::Agent;

const RFC_BASE_URL: &str = "https://www.rfc-editor.org/rfc/rfc";

/// Client for fetching RFCs
///
/// This client is used to fetch RFCs from the RFC Editor's website.
/// It is responsible for fetching the RFC index and RFCs.
pub struct RfcClient
{
    client: Agent,
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
        let client = Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30)))
            .build();

        Self {
            client: client.into(),
        }
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
    pub fn fetch_rfc(&self, rfc_number: u16) -> Result<String>
    {
        // RFC documents are available in TXT format
        let rfc_url = format!("{RFC_BASE_URL}{rfc_number}.txt");

        let response = self
            .client
            .get(rfc_url)
            .call()
            .context(format!("Failed to fetch RFC {rfc_number}"))?;

        let mut response_body = String::new();
        response
            .into_body()
            .into_reader()
            .read_to_string(&mut response_body)
            .context(format!("Failed to read RFC {rfc_number} content"))?;

        Ok(response_body
            .trim()
            .replace('\x0c', "") // Remove the unnecesary form feed.
            .to_string())
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
    pub fn fetch_rfc_index(&self) -> Result<String>
    {
        // RFC index is available at a different URL
        let rfc_url: &'static str = "https://www.rfc-editor.org/rfc-index.txt";

        let response = self
            .client
            .get(rfc_url)
            .call()
            .context("Failed to fetch RFC index")?;

        let mut response_body = String::new();
        response
            .into_body()
            .into_reader()
            .read_to_string(&mut response_body)
            .context("Failed to read RFC index content")?;

        Ok(response_body)
    }
}

impl Default for RfcClient
{
    fn default() -> Self
    {
        Self::new()
    }
}
