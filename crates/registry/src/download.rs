use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

/// Download a file from `url`, verify its SHA-256 matches `expected_hex`,
/// and write it to `dest`. Reports progress via `on_progress(downloaded, total)`.
pub fn download_plugin(
    url: &str,
    expected_hex: &str,
    dest: &Path,
    on_progress: &dyn Fn(u64, u64),
) -> Result<(), String> {
    let resp = crate::AGENT
        .get(url)
        .header("User-Agent", "santui")
        .call()
        .map_err(|e| format!("Download failed: {e}"))?;

    let total = resp
        .headers()
        .get("Content-Length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);

    let mut reader = resp.into_body().into_reader();
    let mut body = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("Read response: {e}"))?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&buf[..n]);
        on_progress(body.len() as u64, total);
    }

    // SHA-256 verification.
    let mut hasher = Sha256::new();
    hasher.update(&body);
    let actual: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect();
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
