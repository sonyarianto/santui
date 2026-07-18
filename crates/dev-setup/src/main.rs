use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::Read;
use std::path::Path;

#[derive(Deserialize)]
struct ManifestPlugin {
    id: String,
    name: String,
    description: String,
    #[serde(default)]
    capabilities: Vec<String>,
}

#[derive(Serialize)]
struct PluginEntry {
    id: String,
    name: String,
    publisher: String,
    description: String,
    version: String,
    download_url: String,
    sha256: String,
    size: u64,
    capabilities: Vec<String>,
}

fn main() {
    let args: Vec<String> = env::args().collect();
    match args.get(1).map(|s| s.as_str()) {
        Some("release-json") => cmd_release_json(),
        Some("list-ids") => cmd_list_ids(),
        Some("list-paths") => cmd_list_paths(),
        Some("list-globs") => cmd_list_globs(),
        _ => cmd_dev_json(),
    }
}

fn load_manifest() -> Vec<ManifestPlugin> {
    let json =
        fs::read_to_string("plugins-manifest.json").expect("Failed to read plugins-manifest.json");
    serde_json::from_str(&json).expect("Failed to parse plugins-manifest.json")
}

fn compute_sha256(path: &Path) -> String {
    let mut file = fs::File::open(path).expect("Failed to open binary");
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        match file.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => hasher.update(&buffer[..n]),
            Err(e) => panic!("Failed to read binary: {e}"),
        }
    }
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
}

fn cmd_dev_json() {
    let root = env::var("ROOT").expect("ROOT must be set");
    let outdir = env::var("OUTDIR").expect("OUTDIR must be set");
    let version = env::var("VERSION").expect("VERSION must be set");

    let manifest_path = Path::new(&root).join("plugins-manifest.json");
    let manifest_json =
        fs::read_to_string(&manifest_path).expect("Failed to read plugins-manifest.json");
    let manifest_list: Vec<ManifestPlugin> =
        serde_json::from_str(&manifest_json).expect("Failed to parse plugins-manifest.json");
    let manifest: HashMap<String, ManifestPlugin> = manifest_list
        .into_iter()
        .map(|p| (p.id.clone(), p))
        .collect();

    let outdir_path = Path::new(&outdir);
    let mut entries: Vec<_> = fs::read_dir(outdir_path)
        .expect("Failed to read output directory")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            let name = match p.file_name().and_then(|n| n.to_str()) {
                Some(n) => n,
                None => return false,
            };
            if !name.starts_with("santui-") {
                return false;
            }
            if name == "santui" || name == "santui.exe" {
                return false;
            }
            if name.contains("-scraper") || name.contains("registry-plugin") {
                return false;
            }
            true
        })
        .collect();
    entries.sort();

    let plugins: Vec<PluginEntry> = entries
        .par_iter()
        .filter_map(|path| {
            let name = path.file_name().unwrap().to_str().unwrap();
            let stem = name.strip_suffix(".exe").unwrap_or(name);
            let pid = stem.strip_prefix("santui-").unwrap();
            let p = manifest.get(pid)?;

            let size = fs::metadata(path).ok()?.len();

            Some(PluginEntry {
                id: pid.to_string(),
                name: p.name.clone(),
                publisher: "Santui".to_string(),
                description: p.description.clone(),
                version: version.clone(),
                download_url: format!("target/debug/{name}"),
                sha256: String::new(),
                size,
                capabilities: p.capabilities.clone(),
            })
        })
        .collect();

    for p in &plugins {
        println!("  [OK] {}  ({} bytes)", p.id, p.size);
    }

    let plugins_json_path = Path::new(&root).join("plugins.json");
    let json = serde_json::to_string_pretty(&plugins).expect("Failed to serialize plugins.json");
    fs::write(&plugins_json_path, json).expect("Failed to write plugins.json");

    let s = if plugins.len() == 1 { "" } else { "s" };
    println!(
        "[OK] plugins.json generated ({} plugin{})",
        plugins.len(),
        s
    );
}

fn cmd_release_json() {
    let version = env::var("VERSION").expect("VERSION must be set");
    let target = env::var("TARGET").expect("TARGET must be set");
    let ext = env::var("EXT").expect("EXT must be set");
    let repo = env::var("REPO").expect("REPO must be set");
    let tag = env::var("TAG").expect("TAG must be set");
    let binary_dir = env::var("BINARY_DIR").unwrap_or_else(|_| "target/release".to_string());

    let manifest = load_manifest();

    let plugins: Vec<PluginEntry> = manifest
        .par_iter()
        .filter_map(|p| {
            let binpath = Path::new(&binary_dir).join(format!("santui-{}{}", p.id, ext));
            if !binpath.exists() {
                return None;
            }
            let sha256 = compute_sha256(&binpath);
            let size = fs::metadata(&binpath).ok()?.len();
            let download_url = format!(
                "https://github.com/{repo}/releases/download/{tag}/{}-{target}{ext}",
                p.id
            );

            Some(PluginEntry {
                id: p.id.clone(),
                name: p.name.clone(),
                publisher: "Santui".to_string(),
                description: p.description.clone(),
                version: version.clone(),
                download_url,
                sha256,
                size,
                capabilities: p.capabilities.clone(),
            })
        })
        .collect();

    let out_path = format!("plugins-{target}.json");
    let json = serde_json::to_string_pretty(&plugins).expect("Failed to serialize plugins.json");
    fs::write(&out_path, json).expect("Failed to write plugins.json");
    println!("Generated {out_path} with {} plugin(s)", plugins.len());
}

fn cmd_list_ids() {
    let manifest = load_manifest();
    let ids: Vec<&str> = manifest.iter().map(|p| p.id.as_str()).collect();
    println!("{}", ids.join(" "));
}

fn cmd_list_paths() {
    let binary_dir = env::var("BINARY_DIR").unwrap_or_else(|_| "target/release".to_string());
    let ext = env::var("EXT").unwrap_or_default();
    let manifest = load_manifest();
    let paths: Vec<String> = manifest
        .iter()
        .map(|p| format!("{binary_dir}/santui-{}{ext}", p.id))
        .collect();
    println!("{}", paths.join(" "));
}

fn cmd_list_globs() {
    let manifest = load_manifest();
    let globs: Vec<String> = manifest.iter().map(|p| format!("{}-*", p.id)).collect();
    println!("{}", globs.join(" "));
}
