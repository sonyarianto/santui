pub mod config;
mod download;

pub use config::InstalledPlugin;
pub use download::download_plugin;
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
    /// Publisher name (e.g. "Santui").
    #[serde(default)]
    pub publisher: String,
    /// Declared capabilities (e.g. "background" for audio plugins).
    #[serde(default)]
    pub capabilities: Vec<String>,
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
        let installed = config::RegistryConfig::load(&config_path)
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
    fn manifest_filename() -> String {
        let triple = match (std::env::consts::OS, std::env::consts::ARCH) {
            ("windows", "x86_64") => "x86_64-pc-windows-msvc",
            ("macos", "aarch64") => "aarch64-apple-darwin",
            ("macos", "x86_64") => "x86_64-apple-darwin",
            ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
            _ => return "plugins.json".into(),
        };
        format!("plugins-{triple}.json")
    }

    /// Fetch the plugin manifest from GitHub Releases.
    /// Uses `SANTUI_REPO` env or defaults to `sonyarianto/santui`.
    ///
    /// Tries the direct release download URL first (no rate limit), then
    /// falls back to the GitHub API if the file is not found at that path.
    pub fn fetch_manifest(&mut self) -> Result<(), String> {
        let repo = std::env::var("SANTUI_REPO").unwrap_or_else(|_| "sonyarianto/santui".into());
        let manifest_name = Self::manifest_filename();
        let base_url = format!("https://github.com/{repo}/releases/latest/download");

        // Try direct download URLs first (no rate limit).
        let names_to_try = [&manifest_name as &str, "plugins.json"];
        let mut last_err = String::new();
        for name in &names_to_try {
            let url = format!("{base_url}/{name}");
            match ureq::get(&url).call() {
                Ok(mut resp) => {
                    let body = resp
                        .body_mut()
                        .read_to_string()
                        .map_err(|e| format!("Failed to read manifest: {e}"))?;
                    self.available =
                        parse_manifest(&body).map_err(|e| format!("Invalid manifest JSON: {e}"))?;
                    self.fetched = true;
                    self.status = format!("{} plugin(s) available", self.available.len());
                    return Ok(());
                }
                Err(e) => last_err = format!("{name}: {e}"),
            }
        }

        // Fallback: GitHub Releases API (subject to rate limiting).
        let api_url = format!("https://api.github.com/repos/{repo}/releases/latest");
        let mut resp = ureq::get(&api_url)
            .header("User-Agent", "santui")
            .call()
            .map_err(|e| {
                format!("Failed to fetch release (direct download also failed: {last_err}): {e}")
            })?;

        let body = resp
            .body_mut()
            .read_to_string()
            .map_err(|e| format!("Failed to read response: {e}"))?;

        let release: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| format!("Invalid JSON: {e}"))?;

        let assets = release["assets"]
            .as_array()
            .ok_or_else(|| "No assets in release".to_string())?;

        let plugin_asset = assets
            .iter()
            .find(|a| a["name"].as_str() == Some(&manifest_name))
            .or_else(|| {
                assets
                    .iter()
                    .find(|a| a["name"].as_str() == Some("plugins.json"))
            })
            .ok_or_else(|| format!("No {manifest_name} or plugins.json found in release assets"))?;

        let download_url = plugin_asset["browser_download_url"]
            .as_str()
            .ok_or_else(|| "Missing download_url".to_string())?;

        let mut manifest_resp = ureq::get(download_url)
            .header("User-Agent", "santui")
            .call()
            .map_err(|e| format!("Failed to fetch manifest: {e}"))?;

        let manifest_body = manifest_resp
            .body_mut()
            .read_to_string()
            .map_err(|e| format!("Failed to read manifest: {e}"))?;

        self.available =
            parse_manifest(&manifest_body).map_err(|e| format!("Invalid manifest JSON: {e}"))?;
        self.fetched = true;
        self.status = format!("{} plugin(s) available", self.available.len());
        Ok(())
    }

    /// Download and install a plugin from the manifest (blocking).
    /// Reports progress via `on_progress(downloaded, total)`.
    /// Config is persisted *before* the binary write so a crash mid-install
    /// leaves a recoverable entry rather than a zombie binary.
    pub fn install(
        &mut self,
        manifest: &PluginManifest,
        on_progress: &dyn Fn(u64, u64),
    ) -> Result<(), String> {
        std::fs::create_dir_all(&self.plugins_dir)
            .map_err(|e| format!("Failed to create plugins dir: {e}"))?;

        let target_path = self.plugins_dir.join(plugin_filename(&manifest.id));

        self.installed.retain(|p| p.id != manifest.id);
        self.installed.push(InstalledPlugin {
            enabled: true,
            version: manifest.version.clone(),
            path: target_path.clone(),
            id: manifest.id.clone(),
            name: manifest.name.clone(),
            capabilities: manifest.capabilities.clone(),
        });
        self.save_config()?;

        let result = if self.dev_mode {
            let src = Path::new(&manifest.download_url);
            std::fs::copy(src, &target_path)
                .map_err(|e| format!("Failed to copy plugin binary from {}: {e}", src.display()))?;
            self.copy_native_deps(src)?;
            Ok(())
        } else {
            download_plugin(
                &manifest.download_url,
                &manifest.sha256,
                &target_path,
                on_progress,
            )
        };

        if let Err(e) = result {
            self.installed.pop();
            self.save_config()?;
            return Err(e);
        }

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
            parse_manifest(&text).map_err(|e| format!("Invalid manifest JSON: {e}"))?;
        self.fetched = true;
        self.status = format!("{} plugin(s) available", self.available.len());
        Ok(())
    }

    /// Remove an installed plugin by index (deletes binary + config entry).
    pub fn remove_installed(&mut self, idx: usize) -> Result<(), String> {
        if idx >= self.installed.len() {
            return Err("Invalid plugin index".into());
        }
        let path = self.installed[idx].path.clone();
        self.installed.remove(idx);
        self.save_config()?;
        let _ = std::fs::remove_file(&path);
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

    /// Add a newly installed plugin entry and persist config.
    pub fn add_installed(
        &mut self,
        id: &str,
        name: &str,
        version: &str,
        target_path: PathBuf,
        capabilities: &[String],
    ) -> Result<(), String> {
        self.installed.retain(|p| p.id != id);
        self.installed.push(InstalledPlugin {
            enabled: true,
            version: version.to_string(),
            path: target_path,
            id: id.to_string(),
            name: name.to_string(),
            capabilities: capabilities.to_vec(),
        });
        self.save_config()
    }

    pub fn save_config(&self) -> Result<(), String> {
        let cfg = config::RegistryConfig {
            plugins: self.installed.clone(),
        };
        cfg.save(&self.config_path)
    }
}

/// Parse a manifest JSON that may be either an array or a single object.
///
/// Some releases (notably from the PowerShell CI step) may produce a bare
/// object instead of a single-element array due to a `ConvertTo-Json` bug.
fn parse_manifest(text: &str) -> Result<Vec<PluginManifest>, String> {
    // Fast path: try array first.
    if let Ok(v) = serde_json::from_str::<Vec<PluginManifest>>(text) {
        return Ok(v);
    }

    // Fallback: single PluginManifest object (PowerShell single-element bug).
    if let Ok(m) = serde_json::from_str::<PluginManifest>(text) {
        return Ok(vec![m]);
    }

    // Last resort: object wrapping a "plugins" key.
    #[derive(serde::Deserialize)]
    struct Wrapper {
        plugins: Vec<PluginManifest>,
    }
    let w: Wrapper = serde_json::from_str(text)
        .map_err(|e| format!("expected array, object, or {{plugins: […]}}: {e}"))?;
    Ok(w.plugins)
}

/// Return the filename for a plugin binary on the current platform.
pub fn plugin_filename(id: &str) -> String {
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
        let name = plugin_filename("test-plugin");
        if cfg!(target_os = "windows") {
            assert!(name.ends_with(".exe"));
        } else {
            assert!(!name.ends_with(".exe"));
        }
    }

    #[test]
    fn manifest_filename_returns_non_empty() {
        let name = Registry::manifest_filename();
        assert!(!name.is_empty());
        assert!(name.ends_with(".json"));
    }

    #[test]
    fn parse_manifest_array() {
        let json = r#"[
            {"id":"p1","name":"Plugin 1","description":"Desc","version":"1.0","download_url":"https://example.com/p1","sha256":"abcd","size":100,"publisher":"Santui"}
        ]"#;
        let result = parse_manifest(json).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "p1");
        assert_eq!(result[0].publisher, "Santui");
    }

    #[test]
    fn parse_manifest_single_object() {
        let json = r#"{"id":"p1","name":"Plugin 1","description":"Desc","version":"1.0","download_url":"https://example.com/p1","sha256":"abcd","size":100,"publisher":"Santui"}"#;
        let result = parse_manifest(json).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "p1");
    }

    #[test]
    fn parse_manifest_wrapper_object() {
        let json = r#"{"plugins":[{"id":"p1","name":"Plugin 1","description":"Desc","version":"1.0","download_url":"https://example.com/p1","sha256":"abcd","size":100,"publisher":"Santui"}]}"#;
        let result = parse_manifest(json).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "p1");
    }

    #[test]
    fn parse_manifest_defaults_publisher() {
        let json = r#"[
            {"id":"p1","name":"P1","description":"D","version":"1","download_url":"https://ex.com/p1","sha256":"a","size":1}
        ]"#;
        let result = parse_manifest(json).unwrap();
        assert_eq!(result[0].publisher, "");
    }

    #[test]
    fn parse_manifest_empty_array() {
        let result = parse_manifest("[]").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn parse_manifest_invalid_json() {
        let result = parse_manifest("not json");
        assert!(result.is_err());
    }

    #[test]
    fn parse_manifest_multiple_plugins() {
        let json = r#"[
            {"id":"p1","name":"P1","description":"D","version":"1","download_url":"https://ex.com/p1","sha256":"a","size":1,"publisher":"Santui"},
            {"id":"p2","name":"P2","description":"D","version":"2","download_url":"https://ex.com/p2","sha256":"b","size":2,"publisher":"Santui"}
        ]"#;
        let result = parse_manifest(json).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id, "p1");
        assert_eq!(result[1].id, "p2");
    }

    #[test]
    fn plugin_manifest_serde_roundtrip() {
        let m = PluginManifest {
            id: "test".into(),
            name: "Test".into(),
            description: "A test".into(),
            version: "0.1".into(),
            download_url: "https://example.com/p".into(),
            sha256: "abc123".into(),
            size: 42,
            publisher: "Santui".into(),
            capabilities: vec!["background".into()],
        };
        let json = serde_json::to_string(&m).unwrap();
        let back: PluginManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, m.id);
        assert_eq!(back.name, m.name);
        assert_eq!(back.size, m.size);
        assert_eq!(back.publisher, m.publisher);
        assert_eq!(back.capabilities, vec!["background".to_string()]);
    }

    #[test]
    fn installed_plugin_serde_roundtrip() {
        let p = InstalledPlugin {
            enabled: true,
            version: "1.0".into(),
            path: PathBuf::from("/tmp/plugin.exe"),
            id: "test-plugin".into(),
            name: "Test Plugin".into(),
            capabilities: vec!["background".into()],
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: InstalledPlugin = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, p.id);
        assert!(back.enabled);
        assert_eq!(back.capabilities, vec!["background".to_string()]);
    }

    #[test]
    fn installed_plugin_defaults_id_and_name_when_missing() {
        let json = r#"{"enabled":true,"version":"1","path":"/x"}"#;
        let p: InstalledPlugin = serde_json::from_str(json).unwrap();
        assert!(p.enabled);
        assert_eq!(p.id, "");
        assert_eq!(p.name, "");
        assert_eq!(p.version, "1");
    }

    #[test]
    fn registry_new_creates_empty_registry() {
        let dir = std::env::temp_dir().join("santui-reg-test-new");
        let _ = std::fs::create_dir_all(&dir);
        let r = Registry::new(dir.clone());
        assert!(r.available.is_empty());
        assert!(!r.fetched);
        assert!(!r.dev_mode);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn registry_set_enabled_toggles_state() {
        let dir = std::env::temp_dir().join("santui-reg-test-toggle");
        let _ = std::fs::create_dir_all(&dir);
        let mut r = Registry::new(dir.clone());

        r.installed.push(InstalledPlugin {
            enabled: false,
            version: "1".into(),
            path: PathBuf::from("test.exe"),
            id: "test".into(),
            name: "Test".into(),
            capabilities: Vec::new(),
        });

        r.set_enabled(0, true).unwrap();
        assert!(r.installed[0].enabled);

        r.set_enabled(0, false).unwrap();
        assert!(!r.installed[0].enabled);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn registry_add_installed_appends_and_persists() {
        let dir = std::env::temp_dir().join("santui-reg-test-add");
        let _ = std::fs::create_dir_all(&dir);
        let mut r = Registry::new(dir.clone());

        r.add_installed("p1", "Plugin 1", "1.0", PathBuf::from("p1.exe"), &[])
            .unwrap();
        assert_eq!(r.installed.len(), 1);
        assert_eq!(r.installed[0].id, "p1");
        assert!(r.installed[0].enabled);

        // Adding the same id replaces rather than duplicates
        r.add_installed("p1", "Plugin 1", "2.0", PathBuf::from("p1.exe"), &[])
            .unwrap();
        assert_eq!(
            r.installed.len(),
            1,
            "duplicate id should replace, not append"
        );
        assert_eq!(r.installed[0].version, "2.0");

        // Config file was written
        let config_path = dir.join("registry.toml");
        assert!(config_path.exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn registry_set_dev_mode() {
        let dir = std::env::temp_dir().join("santui-reg-test-dev");
        let _ = std::fs::create_dir_all(&dir);
        let mut r = Registry::new(dir.clone());
        assert!(!r.dev_mode);

        r.set_dev_mode(true);
        assert!(r.dev_mode);

        r.set_dev_mode(false);
        assert!(!r.dev_mode);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn registry_persists_and_reloads_installed_plugins() {
        let dir = std::env::temp_dir().join("santui-reg-test-persist");
        let _ = std::fs::create_dir_all(&dir);

        // First instance — add a plugin
        {
            let mut r = Registry::new(dir.clone());
            r.add_installed("p1", "P1", "1.0", PathBuf::from("p1.exe"), &[])
                .unwrap();
        }

        // Second instance — should reload from config
        {
            let r = Registry::new(dir.clone());
            assert_eq!(r.installed.len(), 1);
            assert_eq!(r.installed[0].id, "p1");
            assert_eq!(r.installed[0].version, "1.0");
        }

        let _ = std::fs::remove_dir_all(&dir);
    }
}
