//! Cache module for storing RFC documents locally.
//!
//! Provides functionality to read and write RFCs to disk,
//! reducing the need for repeated network requests.
use anyhow::{Context, Result};
use directories::ProjectDirs;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;

/// Cache for storing RFC documents locally.
///
/// Provides functionality to read and write RFCs to disk,
/// reducing the need for repeated network requests.
pub struct RfcCache
{
    /// Directory where cache files are stored
    cache_dir: PathBuf,
}

impl RfcCache
{
    /// Creates a new `RfcCache` instance.
    ///
    /// Creates the cache directory if it doesn't already exist.
    ///
    /// # Returns
    ///
    /// A Result containing the new `RfcCache` or an error if the cache
    /// directory could not be determined or created.
    ///
    /// # Errors
    ///
    /// Returns an error if the cache directory cannot be determined or created.
    pub fn new() -> Result<Self>
    {
        let project_dirs = ProjectDirs::from("", "ozan", "rfc_reader")
            .context("Failed to determine project directories")?;

        let cache_dir = project_dirs.cache_dir().to_path_buf();
        // Create if cache_dir doesn't exist.
        fs::create_dir_all(&cache_dir).context("Failed to create cache directory")?;

        Ok(Self { cache_dir })
    }

    /// Retrieves an RFC from the cache.
    ///
    /// # Arguments
    ///
    /// * `rfc_number` - The RFC number to retrieve
    ///
    /// # Returns
    ///
    /// Some(String) containing the RFC content if it exists in the cache,
    /// or None if the RFC is not cached or cannot be read.
    #[must_use]
    pub fn get_cached_rfc(&self, rfc_number: u32) -> Option<String>
    {
        let rfc_path = self.format_cache_path(rfc_number);

        if !rfc_path.exists()
        {
            return None;
        }

        fs::read_to_string(&rfc_path).ok()
    }

    /// Stores an RFC in the cache.
    ///
    /// # Arguments
    ///
    /// * `rfc_number` - The RFC number to cache
    /// * `content` - The content of the RFC to store
    ///
    /// # Returns
    ///
    /// A Result indicating success or an error if writing to the cache failed.
    ///
    /// # Errors
    ///
    /// Returns an error if the cache file cannot be created or written to.
    pub fn cache_rfc(&self, rfc_number: u32, content: &str) -> Result<()>
    {
        let rfc_path = self.format_cache_path(rfc_number);

        let mut file = File::create(&rfc_path)
            .context(format!("Failed to create cache file for RFC {rfc_number}"))?;

        // Write the contents.
        file.write_all(content.as_bytes())
            .context(format!(
                "Failed to write content to cache for RFC {rfc_number}",
            ))?;

        Ok(())
    }

    /// Retrieves the RFC index from the cache.
    ///
    /// # Returns
    ///
    /// Some(String) containing the RFC index if it exists in the cache,
    /// or None if the index is not cached or cannot be read.
    #[must_use]
    pub fn get_cached_index(&self) -> Option<String>
    {
        let path = self.get_index_cache_path();

        if !path.exists()
        {
            return None;
        }

        fs::read_to_string(&path).ok()
    }

    /// Stores the RFC index in the cache.
    ///
    /// # Arguments
    ///
    /// * `content` - The content of the RFC index to store
    ///
    /// # Returns
    ///
    /// A Result indicating success or an error if writing to the cache failed.
    ///
    /// # Errors
    ///
    /// Returns an error if the cache file cannot be created or written to.
    pub fn cache_index(&self, content: &str) -> Result<()>
    {
        let path = self.get_index_cache_path();

        let mut file = File::create(&path).context("Failed to create cache file for RFC index")?;

        file.write_all(content.as_bytes())
            .context("Failed to write RFC index to cache")?;

        Ok(())
    }

    /// Format the file path for a specific RFC in the cache.
    ///
    /// # Arguments
    ///
    /// * `rfc_number` - The RFC number
    ///
    /// # Returns
    ///
    /// The path where the RFC should be cached
    fn format_cache_path(&self, rfc_number: u32) -> PathBuf
    {
        self.cache_dir
            .join(format!("rfc{rfc_number}.txt"))
    }

    /// Gets the file path for the RFC index in the cache.
    ///
    /// # Returns
    ///
    /// The path where the RFC index should be cached
    fn get_index_cache_path(&self) -> PathBuf
    {
        self.cache_dir.join("rfc-index.txt")
    }

    /// Clears all cached RFCs and the index.
    ///
    /// # Returns
    ///
    /// A Result indicating success or an error if clearing the cache failed.
    ///
    /// # Errors
    ///
    /// Returns an error if removing files from the cache directory fails.
    pub fn clear(&self) -> Result<()>
    {
        // Read the directory entries
        let entries = fs::read_dir(&self.cache_dir).context("Failed to read cache directory")?;

        // Remove each file in the cache directory
        for entry in entries
        {
            let entry = entry.context("Failed to read cache directory entry")?;
            let path = entry.path();

            if path.is_file()
            {
                fs::remove_file(&path)
                    .context(format!("Failed to remove cache file: {}", path.display()))?;
            }
        }

        // Remove the directory if it is empty.
        let is_empty = self
            .cache_dir
            .read_dir()
            .context("Failed to check if cache directory is empty")?
            .next()
            .is_none();

        if is_empty
        {
            fs::remove_dir(&self.cache_dir).context("Failed to remove empty cache directory")?;
        }

        Ok(())
    }

    /// Get the cache directory.
    ///
    /// # Returns
    ///
    /// The cache directory.
    #[must_use]
    pub fn cache_dir(&self) -> &PathBuf
    {
        &self.cache_dir
    }
}
