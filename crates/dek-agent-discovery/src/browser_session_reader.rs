use anyhow::Result;
use std::path::{Path, PathBuf};

/// Check if "bytes of a session/tab file" contains this domain
/// Chromium SNSS stores URLs in both UTF-8 (std::string) and UTF-16 (string16) -> check both
pub fn bytes_contain_domain(bytes: &[u8], domain: &str) -> bool {
    // 1) UTF-8 / ASCII
    if find_subslice(bytes, domain.as_bytes()).is_some() {
        return true;
    }
    // 2) UTF-16LE: "chatgpt.com" -> [c,0,h,0,a,0,...]
    let utf16: Vec<u8> = domain
        .encode_utf16()
        .flat_map(|u| u.to_le_bytes())
        .collect();
    find_subslice(bytes, &utf16).is_some()
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Firefox sessionstore: file starts with magic "mozLz40\0" followed by LZ4 block (raw) with 4-byte LE size
/// recovery.jsonlz4 = list of "currently open tabs" -> readable even if tabs are idle
pub fn read_mozlz4(path: &Path) -> Result<Vec<u8>> {
    let raw = std::fs::read(path)?;
    const MAGIC: &[u8] = b"mozLz40\0";
    if raw.len() < MAGIC.len() + 4 || &raw[..MAGIC.len()] != MAGIC {
        // Not mozlz4 -> return as is (in case sessionstore.js is old raw JSON)
        return Ok(raw);
    }
    let size_off = MAGIC.len();
    let decompressed_size =
        u32::from_le_bytes(raw[size_off..size_off + 4].try_into().unwrap_or([0; 4])) as usize;
    let compressed = &raw[size_off + 4..];
    // Use lz4_flex (raw block)
    let out = lz4_flex::block::decompress(compressed, decompressed_size)
        .map_err(|e| anyhow::anyhow!("mozlz4 decompress failed: {e}"))?;
    Ok(out)
}

/// Read session/tab file into searchable bytes (decompress if Firefox)
pub fn read_session_bytes(path: &Path) -> Result<Vec<u8>> {
    match path.extension().and_then(|e| e.to_str()) {
        Some("jsonlz4") | Some("baklz4") | Some("mozlz4") => read_mozlz4(path),
        _ => Ok(std::fs::read(path)?), // Chromium SNSS / raw JSON
    }
}

/// path to Firefox sessionstore
pub fn firefox_session_paths() -> Vec<PathBuf> {
    let mut out = vec![];
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    let home = dirs::home_dir();
    let profiles_roots: Vec<PathBuf> = {
        #[cfg(target_os = "windows")]
        {
            vec![dirs::data_dir().map(|d| d.join("Mozilla/Firefox/Profiles"))]
                .into_iter()
                .flatten()
                .collect()
        }
        #[cfg(target_os = "macos")]
        {
            home.iter()
                .map(|h| h.join("Library/Application Support/Firefox/Profiles"))
                .collect()
        }
        #[cfg(target_os = "linux")]
        {
            home.iter().map(|h| h.join(".mozilla/firefox")).collect()
        }
    };
    for root in profiles_roots {
        if let Ok(entries) = std::fs::read_dir(&root) {
            for p in entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
                out.push(p.join("sessionstore-backups/recovery.jsonlz4"));
                out.push(p.join("sessionstore-backups/previous.jsonlz4"));
                out.push(p.join("sessionstore.jsonlz4"));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_utf16_session_match_chatgpt() {
        let needle = "chatgpt.com";
        // construct UTF-16LE bytes
        let utf16: Vec<u8> = needle
            .encode_utf16()
            .flat_map(|u| u.to_le_bytes())
            .collect();

        let mut haystack = vec![0x00, 0x01, 0x02];
        haystack.extend_from_slice(&utf16);
        haystack.extend_from_slice(&[0x03, 0x04]);

        assert!(bytes_contain_domain(&haystack, needle));

        // UTF-8 test
        let mut haystack_utf8 = vec![0x00, 0x01];
        haystack_utf8.extend_from_slice(needle.as_bytes());
        assert!(bytes_contain_domain(&haystack_utf8, needle));

        // Not found
        assert!(!bytes_contain_domain(&[0, 1, 2, 3], needle));
    }
}
