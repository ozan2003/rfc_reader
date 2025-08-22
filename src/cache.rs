//! Manages local caching of RFC documents.
//!
//! Stores document content on disk to minimize redundant network requests.
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use directories::ProjectDirs;

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
        let project_dirs = ProjectDirs::from("", "", env!("CARGO_PKG_NAME"))
            .context("Failed to determine project directories")?;

        let cache_dir = project_dirs.cache_dir().to_path_buf();
        // Create if cache_dir doesn't exist.
        fs::create_dir_all(&cache_dir)
            .context("Failed to create cache directory")?;

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
    /// A Result containing the content of the RFC if it exists in the cache,
    /// or an error if the RFC is not cached or cannot be read.
    ///
    /// # Errors
    ///
    /// Returns an error if the cached RFC does not exist or cannot be read.
    pub fn get_cached_rfc(&self, rfc_number: u16) -> Result<Box<str>>
    {
        let rfc_path = self.format_cache_path(rfc_number);

        if !rfc_path.exists()
        {
            bail!(
                "Cached RFC {rfc_number} does not exist at {}",
                rfc_path.display()
            );
        }

        let content = fs::read_to_string(&rfc_path).with_context(|| {
            format!(
                "Failed to read cached RFC {rfc_number} from {}",
                rfc_path.display()
            )
        })?;

        Ok(content.into_boxed_str())
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
    pub fn cache_rfc(&self, rfc_number: u16, content: &str) -> Result<()>
    {
        let rfc_path = self.format_cache_path(rfc_number);

        let mut file = File::create(&rfc_path).with_context(|| {
            format!("Failed to create cache file for RFC {rfc_number}")
        })?;

        // Write the contents.
        if let Err(write_err) = file.write_all(content.as_bytes())
        {
            // We already messed up but the empty file remains.
            // Attempt cleanup, but don't let the cleanup errors override the
            // original error
            let _ = fs::remove_file(&rfc_path);

            bail!(write_err);
        }

        Ok(())
    }

    /// Retrieves the RFC index from the cache.
    ///
    /// # Returns
    ///
    /// A Result containing the content of the RFC index if it exists in the
    /// cache, or an error if the index is not cached or cannot be read.
    ///
    /// # Errors
    ///
    /// Returns an error if the cached index does not exist or cannot be read.
    pub fn get_cached_index(&self) -> Result<Box<str>>
    {
        let path = self.get_index_cache_path();

        if !path.exists()
        {
            bail!("Cached RFC index does not exist at {}", path.display());
        }

        let content = fs::read_to_string(&path).with_context(|| {
            format!("Failed to read cached RFC index from {}", path.display())
        })?;

        Ok(content.into_boxed_str())
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

        let mut file = File::create(&path)
            .context("Failed to create cache file for RFC index")?;

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
    fn format_cache_path(&self, rfc_number: u16) -> PathBuf
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
        let entries = fs::read_dir(&self.cache_dir)
            .context("Failed to read cache directory")?;

        // Remove each file or directory in the cache directory
        for entry in entries.filter_map(Result::ok)
        {
            let path = entry.path();

            if path.is_file()
            {
                fs::remove_file(&path).with_context(|| {
                    format!("Failed to remove cache file: {}", path.display())
                })?;
            }
            else if path.is_dir()
            {
                fs::remove_dir_all(&path).with_context(|| {
                    format!(
                        "Failed to remove cache directory: {}",
                        path.display()
                    )
                })?;
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
            fs::remove_dir(&self.cache_dir)
                .context("Failed to remove empty cache directory")?;
        }

        Ok(())
    }

    /// Get the cache directory.
    ///
    /// # Returns
    ///
    /// The cache directory.
    #[must_use]
    pub const fn cache_dir(&self) -> &PathBuf
    {
        &self.cache_dir
    }

    /// List the cached RFCs.
    ///
    /// # Panics
    ///
    /// Panics if the cache directory cannot be read.
    pub fn print_list(&self)
    {
        // Read the directory entries.
        let entries: Vec<_> = fs::read_dir(&self.cache_dir)
            .expect("Failed to read cache directory")
            .filter_map(Result::ok)
            .collect();

        if entries.is_empty()
        {
            println!("No cached RFCs found.");
            return;
        }

        println!("List of cached RFCs:");

        for entry in entries
        {
            let path = entry.path();
            if path.is_file()
            {
                let file_name = path
                    .file_name()
                    .expect("Failed to get file name")
                    .to_string_lossy();

                // Skip the index file
                if file_name == "rfc-index.txt"
                {
                    println!("- RFC Index");
                }
                // Extract the RFC number from the file name.
                else if let Some(rfc_num) = file_name
                    .strip_prefix("rfc")
                    .and_then(|name| name.strip_suffix(".txt"))
                {
                    println!("- RFC {rfc_num}");
                }
                else
                {
                    // Warn the user for stray files
                    println!("{} (not a valid RFC document)", file_name);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests
{
    use std::fs::File;
    use std::io::Write;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_clear_with_files() -> Result<()>
    {
        // Create a temporary directory for testing
        let temp_dir = TempDir::new()?;
        let cache_dir = temp_dir.path().to_path_buf();

        // Create an instance.
        let cache = RfcCache {
            cache_dir: cache_dir.clone(),
        };

        // Create test files in the temporary directory
        let file_paths = vec!["file1.txt", "file2.txt", "file3.txt"];
        for file_name in &file_paths
        {
            let file_path = cache_dir.join(file_name);
            let mut file = File::create(&file_path)?;
            writeln!(file, "test content")?;
        }

        // Verify files exist before clearing
        for file_name in &file_paths
        {
            assert!(cache_dir.join(file_name).exists());
        }

        // Call the clear function
        cache.clear()?;

        // Verify all files have been deleted
        for file_name in &file_paths
        {
            assert!(!cache_dir.join(file_name).exists());
        }

        // Verify the directory has been removed since it should be empty
        assert!(!cache_dir.exists());

        // The temp_dir will be automatically cleaned up when it goes out of
        // scope
        Ok(())
    }

    #[test]
    fn test_clear_with_no_files() -> Result<()>
    {
        // Create a temporary directory for testing
        let temp_dir = TempDir::new()?;
        let cache_dir = temp_dir.path().to_path_buf();

        // Create an instance of your struct with the temp directory
        let cache = RfcCache {
            cache_dir: cache_dir.clone(),
        };

        // Call the clear function on an empty directory
        cache.clear()?;

        // Verify the directory has been removed
        assert!(!cache_dir.exists());

        Ok(())
    }

    #[test]
    fn test_clear_with_mixed_content() -> Result<()>
    {
        // Create a temporary directory for testing
        let temp_dir = TempDir::new()?;
        let cache_dir = temp_dir.path().to_path_buf();

        // Create an instance of your struct with the temp directory
        let cache = RfcCache {
            cache_dir: cache_dir.clone(),
        };

        // Create a file
        let file_path = cache_dir.join("file.txt");
        let mut file = File::create(&file_path)?;
        writeln!(file, "test content")?;

        // Create a subdirectory.
        let subdir_path = cache_dir.join("subdir");
        std::fs::create_dir(&subdir_path)?;

        // Call the clear function
        cache.clear()?;

        // Verify the file is gone
        assert!(!file_path.exists());

        // Verify the cache directory is gone
        assert!(!cache_dir.exists());

        // The subdirectory should be removed
        assert!(!subdir_path.exists());

        Ok(())
    }
}
