mod config;
mod download;

use config::RegistryConfig;
use download::download_plugin;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A plugin entry advertised in the registry manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    /// GitHub release asset download URL for the current platform.
    pub download_url: String,
    /// SHA-256 hex digest of the binary.
    pub sha256: String,
    /// File size in bytes.
    pub size: u64,
}

/// A plugin the user has installed (either enabled or disabled).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPlugin {
    pub enabled: bool,
    pub version: String,
    pub path: PathBuf,
}

/// Top-level state: fetched manifest + local installed set.
pub struct Registry {
    /// Manifest fetched from GitHub Releases.
    pub available: Vec<PluginManifest>,
    /// Locally installed plugins (keyed by plugin id).
    pub installed: Vec<InstalledPlugin>,
    /// Whether we've already fetched the manifest this session.
    pub fetched: bool,
    /// Human-readable status for the UI.
    pub status: String,
    /// Dev mode flag — if true, install() copies local files instead of HTTP download.
    pub dev_mode: bool,
    config_path: PathBuf,
    plugins_dir: PathBuf,
}

impl Registry {
    /// Create a new registry rooted at `base_dir` (e.g. `~/.santui`).
    pub fn new(base_dir: PathBuf) -> Self {
        let plugins_dir = base_dir.join("plugins");
        let config_path = base_dir.join("registry.toml");
        let installed = RegistryConfig::load(&config_path)
            .map(|cfg| cfg.plugins)
            .unwrap_or_default();
        Registry {
            available: Vec::new(),
            installed,
            fetched: false,
            status: String::new(),
            dev_mode: false,
            config_path,
            plugins_dir,
        }
    }

    /// Return the platform-specific manifest filename (e.g. `plugins-x86_64-pc-windows-msvc.json`).
    fn manifest_filename() -> &'static str {
        if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
            "plugins-x86_64-pc-windows-msvc.json"
        } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            "plugins-aarch64-apple-darwin.json"
        } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            "plugins-x86_64-unknown-linux-gnu.json"
        } else {
            // Fallback — try the generic name.
            "plugins.json"
        }
    }

    /// Fetch the plugin manifest from GitHub Releases.
    /// Uses `SANTUI_REPO` env or defaults to `sony-ak/santui`.
    pub fn fetch_manifest(&mut self) -> Result<(), String> {
        let repo = std::env::var("SANTUI_REPO").unwrap_or_else(|_| "sonyarianto/santui".into());
        let manifest_name = Self::manifest_filename();

        // Use the GitHub Releases API to get the latest release's plugins.json asset.
        let api_url = format!("https://api.github.com/repos/{repo}/releases/latest");
        let resp = ureq::get(&api_url)
            .header("User-Agent", "santui")
            .call()
            .map_err(|e| format!("Failed to fetch release: {e}"))?;

        let body = resp
            .into_body()
            .read_to_string()
            .map_err(|e| format!("Failed to read response: {e}"))?;

        // The release JSON has an `assets` array. Find the platform-specific manifest.
        let release: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| format!("Invalid JSON: {e}"))?;

        let assets = release["assets"]
            .as_array()
            .ok_or_else(|| "No assets in release".to_string())?;

        let plugin_asset = assets
            .iter()
            .find(|a| a["name"].as_str() == Some(manifest_name))
            .or_else(|| {
                assets
                    .iter()
                    .find(|a| a["name"].as_str() == Some("plugins.json"))
            })
            .ok_or_else(|| format!("No {manifest_name} or plugins.json found in release assets"))?;

        let download_url = plugin_asset["browser_download_url"]
            .as_str()
            .ok_or_else(|| "Missing download_url".to_string())?;

        // Download and parse the manifest.
        let manifest_resp = ureq::get(download_url)
            .header("User-Agent", "santui")
            .call()
            .map_err(|e| format!("Failed to fetch manifest: {e}"))?;

        let manifest_body = manifest_resp
            .into_body()
            .read_to_string()
            .map_err(|e| format!("Failed to read manifest: {e}"))?;

        self.available = serde_json::from_str(&manifest_body)
            .map_err(|e| format!("Invalid manifest JSON: {e}"))?;
        self.fetched = true;
        self.status = format!("{} plugin(s) available", self.available.len());
        Ok(())
    }

    /// Download and install a plugin from the manifest.
    /// In dev mode, copies the binary locally instead of HTTP download.
    pub fn install(&mut self, manifest: &PluginManifest) -> Result<(), String> {
        std::fs::create_dir_all(&self.plugins_dir)
            .map_err(|e| format!("Failed to create plugins dir: {e}"))?;

        let target_path = self.plugins_dir.join(plugin_filename(&manifest.id));

        if self.dev_mode {
            // In dev mode, the download_url points to a local file path.
            let src = Path::new(&manifest.download_url);
            std::fs::copy(src, &target_path)
                .map_err(|e| format!("Failed to copy plugin binary from {}: {e}", src.display()))?;

            self.copy_native_deps(src)?;
        } else {
            download_plugin(&manifest.download_url, &manifest.sha256, &target_path)?;
        }

        self.installed.push(InstalledPlugin {
            enabled: true,
            version: manifest.version.clone(),
            path: target_path,
        });
        self.save_config()?;
        Ok(())
    }

    /// Copy native/ dependencies from the same directory as `src` to the plugins dir.
    fn copy_native_deps(&self, src: &Path) -> Result<(), String> {
        let native_src = src.parent().map(|p| p.join("native"));
        if let Some(ref native_dir) = native_src {
            if native_dir.is_dir() {
                let native_dst = self.plugins_dir.join("native");
                std::fs::create_dir_all(&native_dst)
                    .map_err(|e| format!("Failed to create native dir: {e}"))?;
                for entry in std::fs::read_dir(native_dir)
                    .map_err(|e| format!("Failed to read native dir: {e}"))?
                {
                    let entry = entry.map_err(|e| format!("Failed to read entry: {e}"))?;
                    let dst = native_dst.join(entry.file_name());
                    std::fs::copy(entry.path(), &dst)
                        .map_err(|e| format!("Failed to copy native dep: {e}"))?;
                }
            }
        }
        Ok(())
    }

    /// In dev mode, sync native deps for all already-installed plugins.
    pub fn sync_all_native_deps(&self) -> Result<(), String> {
        if !self.dev_mode {
            return Ok(());
        }
        for installed in &self.installed {
            let id = installed
                .path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.trim_end_matches(".exe"));
            if let Some(id) = id {
                if let Some(manifest) = self.available.iter().find(|m| m.id == id) {
                    let src = Path::new(&manifest.download_url);
                    self.copy_native_deps(src)?;
                }
            }
        }
        Ok(())
    }

    /// Enable dev mode — install will copy local files instead of HTTP download.
    pub fn set_dev_mode(&mut self, enabled: bool) {
        self.dev_mode = enabled;
    }

    /// Load a local `plugins.json` manifest file instead of fetching from GitHub.
    /// Used for local development/testing.
    pub fn load_local_manifest(&mut self, path: &std::path::Path) -> Result<(), String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read local manifest: {e}"))?;
        self.available =
            serde_json::from_str(&text).map_err(|e| format!("Invalid manifest JSON: {e}"))?;
        self.fetched = true;
        self.status = format!("[DEV] {} plugin(s) available", self.available.len());
        Ok(())
    }

    /// Enable or disable an installed plugin by index.
    pub fn set_enabled(&mut self, idx: usize, enabled: bool) -> Result<(), String> {
        if let Some(p) = self.installed.get_mut(idx) {
            p.enabled = enabled;
            self.save_config()?;
        }
        Ok(())
    }

    fn save_config(&self) -> Result<(), String> {
        let cfg = RegistryConfig {
            plugins: self.installed.clone(),
        };
        cfg.save(&self.config_path)
    }
}

/// Return the filename for a plugin binary on the current platform.
pub(crate) fn plugin_filename(id: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{id}.exe")
    } else {
        id.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_filename_windows() {
        // We can't easily test platform-specific logic, but the function
        // should return something with .exe on Windows.
        let name = plugin_filename("test-plugin");
        if cfg!(target_os = "windows") {
            assert!(name.ends_with(".exe"));
        } else {
            assert!(!name.ends_with(".exe"));
        }
    }
}
