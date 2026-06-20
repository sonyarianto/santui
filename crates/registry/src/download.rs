use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

/// Download a file from `url`, verify its SHA-256 matches `expected_hex`,
/// and write it to `dest`. Returns an error on mismatch or I/O failure.
pub(super) fn download_plugin(url: &str, expected_hex: &str, dest: &Path) -> Result<(), String> {
    let resp = ureq::get(url)
        .header("User-Agent", "santui")
        .call()
        .map_err(|e| format!("Download failed: {e}"))?;

    let mut body = Vec::new();
    resp.into_body()
        .as_reader()
        .read_to_end(&mut body)
        .map_err(|e| format!("Read response: {e}"))?;

    // SHA-256 verification.
    let mut hasher = Sha256::new();
    hasher.update(&body);
    let actual = format!("{:x}", hasher.finalize());
    if actual != expected_hex.to_lowercase() {
        return Err(format!(
            "SHA-256 mismatch: expected {expected_hex}, got {actual}"
        ));
    }

    std::fs::write(dest, &body).map_err(|e| format!("Write binary: {e}"))?;

    // Make executable on Unix.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dest, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("Set permissions: {e}"))?;
    }

    Ok(())
}
