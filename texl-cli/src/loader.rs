//! Local file system loader for the texl engine.
//!
//! The same loader serves both `file://` URLs and scheme-less paths. The engine hands
//! scheme-less paths to the loader already normalized: absolute (`/dir/doc.texl`) and
//! percent-encoded, exactly like the path part of a `file://` URL.

use std::path::PathBuf;
use url::Url;

/// Converts a normalized engine URL (`file:///dir/doc.texl` or `/dir/doc.texl`) into a local path.
pub fn url_to_path(url: &str) -> Result<PathBuf, String> {
    let file_url = match Url::parse(url) {
        Ok(parsed) => parsed,
        // Scheme-less path: read it as the path of a `file://` URL
        Err(_) => Url::parse(&format!("file://{url}")).map_err(|e| format!("{url}: {e}"))?,
    };
    if file_url.scheme() != "file" {
        return Err(format!("{url}: not a file URL"));
    }
    file_url
        .to_file_path()
        .map_err(|_| format!("{url}: not a local file path"))
}

/// Reads the document at `url` from the local file system.
pub async fn load_file(url: String) -> Result<String, String> {
    let path = url_to_path(&url)?;
    tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_to_path_accepts_file_urls_and_plain_paths() {
        assert_eq!(url_to_path("file:///a/b.texl"), Ok(PathBuf::from("/a/b.texl")));
        assert_eq!(url_to_path("/a/b.texl"), Ok(PathBuf::from("/a/b.texl")));
    }

    #[test]
    fn test_url_to_path_decodes_percent_encoding_and_drops_query() {
        assert_eq!(url_to_path("file:///my%20dir/b.texl"), Ok(PathBuf::from("/my dir/b.texl")));
        assert_eq!(url_to_path("/my%20dir/b.texl?v=1"), Ok(PathBuf::from("/my dir/b.texl")));
    }

    #[test]
    fn test_url_to_path_rejects_other_schemes() {
        assert!(url_to_path("http://example.com/b.texl").is_err());
    }
}
