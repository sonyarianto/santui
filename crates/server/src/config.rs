use clap::Parser;
use std::path::PathBuf;

fn platform_data_dir() -> PathBuf {
    if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share"))
            })
            .unwrap_or_else(|| PathBuf::from("."))
    }
    .join("santui")
}

#[derive(Parser, Debug, Clone)]
#[command(name = "santui-server", about = "Santui backend server")]
pub struct CliArgs {
    #[arg(short = 'p', long, default_value = "9876")]
    pub port: u16,

    #[arg(short = 'H', long)]
    pub host: Option<String>,

    #[arg(short = 'd', long = "data-dir")]
    pub data_dir: Option<PathBuf>,

    #[arg(short = 's', long = "jwt-secret")]
    pub jwt_secret: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub port: u16,
    pub host: String,
    pub data_dir: PathBuf,
    pub jwt_secret: String,
}

fn default_data_dir() -> PathBuf {
    platform_data_dir().join("server")
}

fn generate_secret() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("santui-srv-{nanos:x}")
}

impl ServerConfig {
    pub fn load() -> Self {
        let args = CliArgs::parse();

        let data_dir = args
            .data_dir
            .or_else(|| {
                std::env::var("SANTUI_SERVER_DATA_DIR")
                    .ok()
                    .map(PathBuf::from)
            })
            .unwrap_or_else(default_data_dir);

        std::fs::create_dir_all(&data_dir).ok();

        let secret_path = data_dir.join(".jwt_secret");
        let jwt_secret = args
            .jwt_secret
            .or_else(|| {
                std::env::var("SANTUI_JWT_SECRET").ok().or_else(|| {
                    if secret_path.exists() {
                        std::fs::read_to_string(&secret_path)
                            .ok()
                            .map(|s| s.trim().to_string())
                    } else {
                        let secret = generate_secret();
                        std::fs::write(&secret_path, &secret).ok();
                        Some(secret)
                    }
                })
            })
            .unwrap_or_else(generate_secret);

        let port = std::env::var("SANTUI_SERVER_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(args.port);

        let host = args
            .host
            .or_else(|| std::env::var("SANTUI_SERVER_HOST").ok())
            .unwrap_or_else(|| "127.0.0.1".to_string());

        ServerConfig {
            port,
            host,
            data_dir,
            jwt_secret,
        }
    }
}
