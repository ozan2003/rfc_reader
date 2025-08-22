//! RFC client for fetching documents.
//!
//! Manages network requests to the RFC Editor's website.
use std::io::Read;
use std::time::Duration;

use anyhow::{Context, Result};
use log::debug;
use ureq::Agent;
use ureq::config::Config;
use ureq::tls::{TlsConfig, TlsProvider};

const RFC_BASE_URL: &str = "https://www.rfc-editor.org/rfc/rfc";
const RFC_INDEX_URL: &str = "https://www.rfc-editor.org/rfc-index.txt";

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
    pub fn new(duration: Duration) -> Self
    {
        let config = Config::builder()
            .timeout_global(Some(duration))
            .tls_config(
                TlsConfig::builder()
                    .provider(TlsProvider::NativeTls)
                    .build(),
            )
            .build();

        Self {
            client: config.new_agent(),
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
    pub fn fetch_rfc(&self, rfc_number: u16) -> Result<Box<str>>
    {
        // RFC documents are available in TXT format
        let rfc_url = format!("{RFC_BASE_URL}{rfc_number}.txt");

        let response = self
            .client
            .get(rfc_url)
            .call()
            .with_context(|| format!("Failed to fetch RFC {rfc_number}"))?;

        debug!("Got response: {response:?}");

        let mut response_body = String::new();
        response
            .into_body()
            .into_reader()
            .read_to_string(&mut response_body)
            .with_context(|| {
                format!("Failed to read RFC {rfc_number} content")
            })?;

        Ok(
            // Remove the unnecesary form feed.
            response_body
                .trim()
                .replace('\x0c', "")
                .into_boxed_str(),
        )
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
    pub fn fetch_rfc_index(&self) -> Result<Box<str>>
    {
        let response = self
            .client
            .get(RFC_INDEX_URL)
            .call()
            .context("Failed to fetch RFC index")?;

        debug!("Got response: {response:?}");

        let mut response_body = String::new();
        response
            .into_body()
            .into_reader()
            .read_to_string(&mut response_body)
            .context("Failed to read RFC index content")?;

        Ok(response_body.into_boxed_str())
    }
}

impl Default for RfcClient
{
    fn default() -> Self
    {
        Self::new(Duration::from_secs(30))
    }
}
