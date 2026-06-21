use santui_core::auth::{AuthHandle, User};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredToken {
    id: String,
    email: String,
    name: String,
    avatar_url: Option<String>,
    provider: String,
    access_token: String,
    refresh_token: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub auth_uri: String,
    pub token_uri: String,
    pub scopes: Vec<String>,
    pub redirect_port: u16,
}

impl AuthConfig {
    pub fn google(client_id: String, client_secret: Option<String>) -> Self {
        AuthConfig {
            client_id,
            client_secret,
            auth_uri: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            token_uri: "https://oauth2.googleapis.com/token".into(),
            scopes: vec!["openid".into(), "email".into(), "profile".into()],
            redirect_port: 9842,
        }
    }

    pub fn github(client_id: String) -> Self {
        AuthConfig {
            client_id,
            client_secret: None,
            auth_uri: String::new(),
            token_uri: "https://github.com/login/oauth/access_token".into(),
            scopes: vec!["read:user".into(), "user:email".into()],
            redirect_port: 0,
        }
    }
}

#[cfg(target_os = "windows")]
fn open_browser(url: &str) {
    let _ = std::process::Command::new("cmd")
        .args(["/c", "start", &url.replace('&', "^&")])
        .spawn();
}

#[cfg(target_os = "linux")]
fn open_browser(url: &str) {
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

#[cfg(target_os = "macos")]
fn open_browser(url: &str) {
    let _ = std::process::Command::new("open").arg(url).spawn();
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn open_browser(url: &str) {
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

fn bind_with_fallback() -> Result<(TcpListener, u16), Box<dyn std::error::Error>> {
    for port in 9842..9850 {
        if let Ok(listener) = TcpListener::bind(("127.0.0.1", port)) {
            return Ok((listener, port));
        }
    }
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    Ok((listener, port))
}

fn handle_redirect(
    listener: TcpListener,
) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let (stream, _) = listener.accept()?;
    stream.set_read_timeout(Some(Duration::from_secs(120)))?;
    let mut reader = BufReader::new(&stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;

    let params = request_line
        .split_whitespace()
        .nth(1)
        .and_then(|path| {
            let full_url = format!("http://localhost{path}");
            Url::parse(&full_url).ok().map(|u| {
                u.query_pairs()
                    .map(|(k, v)| (k.into_owned(), v.into_owned()))
                    .collect::<HashMap<String, String>>()
            })
        })
        .ok_or_else(|| "No query parameters in redirect".to_string())?;

    let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"UTF-8\"><script src=\"https://cdn.tailwindcss.com\"></script><title>Santui — Signed In</title></head><body class=\"bg-gradient-to-br from-gray-900 via-slate-800 to-gray-900 min-h-screen flex items-center justify-center font-sans\"><div class=\"bg-white/10 backdrop-blur-lg rounded-lg shadow-2xl border border-white/20 p-8 max-w-md w-full mx-4 text-center\"><div class=\"text-emerald-400 mb-4\"><svg class=\"w-16 h-16 mx-auto mb-4\" fill=\"none\" stroke=\"currentColor\" viewBox=\"0 0 24 24\"><path stroke-linecap=\"round\" stroke-linejoin=\"round\" stroke-width=\"1.5\" d=\"M9 12.75L11.25 15 15 9.75M21 12a9 9 0 11-18 0 9 9 0 0118 0z\"/></svg><h1 class=\"text-2xl font-bold mb-1\">Signed In!</h1><p class=\"text-gray-400 text-sm\">You can close this window.</p></div></div></body></html>";
    let mut stream = stream;
    let _ = stream.write_all(response.as_bytes());

    if let Some(err) = params.get("error") {
        return Err(format!("OAuth error from server: {err}").into());
    }

    Ok(params)
}

#[derive(Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    #[allow(dead_code)]
    verification_uri: String,
    interval: Option<u64>,
}

#[derive(Deserialize)]
struct DeviceTokenResponse {
    access_token: Option<String>,
    error: Option<String>,
}

fn request_device_code(
    config: &AuthConfig,
) -> Result<DeviceCodeResponse, Box<dyn std::error::Error>> {
    let scope = config.scopes.join(" ");
    let mut resp = ureq::post("https://github.com/login/device/code")
        .header("Accept", "application/json")
        .send_form([
            ("client_id", config.client_id.as_str()),
            ("scope", scope.as_str()),
        ])?;
    let text = resp.body_mut().read_to_string()?;
    Ok(serde_json::from_str(&text)?)
}

fn poll_device_token(
    config: &AuthConfig,
    device_code: &str,
    interval: u64,
) -> Result<String, Box<dyn std::error::Error>> {
    loop {
        std::thread::sleep(std::time::Duration::from_secs(interval));
        let mut resp = ureq::post(&config.token_uri)
            .header("Accept", "application/json")
            .send_form([
                ("client_id", config.client_id.as_str()),
                ("device_code", device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])?;
        let text = resp.body_mut().read_to_string()?;
        let body: DeviceTokenResponse = serde_json::from_str(&text)?;
        if let Some(token) = body.access_token {
            return Ok(token);
        }
        match body.error.as_deref() {
            Some("authorization_pending") => continue,
            Some("slow_down") => continue,
            Some(err) => return Err(format!("device flow error: {err}").into()),
            None => return Err("unexpected device flow response".into()),
        }
    }
}

fn user_from_token(provider: &str, access_token: &str) -> Result<User, Box<dyn std::error::Error>> {
    match provider {
        "github" => {
            let mut resp = ureq::get("https://api.github.com/user")
                .header("Authorization", &format!("Bearer {access_token}"))
                .header("Accept", "application/vnd.github.v3+json")
                .call()?;
            let body: serde_json::Value = serde_json::from_str(&resp.body_mut().read_to_string()?)?;
            Ok(User {
                id: body["id"].to_string(),
                email: body["email"].as_str().unwrap_or("").into(),
                name: body["login"].as_str().unwrap_or("").into(),
                avatar_url: body["avatar_url"].as_str().map(|s| s.into()),
                provider: provider.into(),
            })
        }
        _ => Err("unsupported provider".into()),
    }
}

pub struct AuthClient {
    providers: HashMap<String, AuthConfig>,
    user: Arc<Mutex<Option<User>>>,
    pending_sign_in: Arc<Mutex<Option<Result<User, String>>>>,
    auth_msg: Arc<Mutex<Option<String>>>,
    token_path: PathBuf,
    vercel_url: String,
}

impl AuthClient {
    pub fn new(providers: Vec<(String, AuthConfig)>) -> Self {
        let token_path = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("santui")
            .join("auth-tokens.json");
        let user = Self::load_tokens(&token_path);
        AuthClient {
            providers: providers.into_iter().collect(),
            user: Arc::new(Mutex::new(user)),
            pending_sign_in: Arc::new(Mutex::new(None)),
            auth_msg: Arc::new(Mutex::new(None)),
            token_path,
            vercel_url: String::new(),
        }
    }

    pub fn with_vercel(mut self, url: String) -> Self {
        self.vercel_url = url;
        self
    }

    fn load_tokens(path: &PathBuf) -> Option<User> {
        let data = std::fs::read_to_string(path).ok()?;
        let stored: StoredToken = serde_json::from_str(&data).ok()?;
        Some(User {
            id: stored.id,
            email: stored.email,
            name: stored.name,
            avatar_url: stored.avatar_url,
            provider: stored.provider,
        })
    }

    fn clear_tokens(&self) {
        let _ = std::fs::remove_file(&self.token_path);
    }

    fn run_google_redirect_flow(
        vercel_url: &str,
        token_path: &PathBuf,
        user_lock: &Arc<Mutex<Option<User>>>,
        pending: &Arc<Mutex<Option<Result<User, String>>>>,
        auth_msg: &Arc<Mutex<Option<String>>>,
    ) {
        let vercel = if vercel_url.is_empty() {
            "https://santuiapp.vercel.app".to_string()
        } else {
            vercel_url.to_string()
        };

        let (listener, port) = match bind_with_fallback() {
            Ok(v) => v,
            Err(e) => {
                *pending.lock().unwrap_or_else(|e| e.into_inner()) = Some(Err(e.to_string()));
                *auth_msg.lock().unwrap_or_else(|e| e.into_inner()) = None;
                return;
            }
        };
        let auth_url = format!("{vercel}/api/auth/google?port={port}");
        *auth_msg.lock().unwrap_or_else(|e| e.into_inner()) =
            Some("Google: waiting for browser…".into());
        open_browser(&auth_url);

        let params = match handle_redirect(listener) {
            Ok(p) => p,
            Err(e) => {
                *pending.lock().unwrap_or_else(|e| e.into_inner()) = Some(Err(e.to_string()));
                *auth_msg.lock().unwrap_or_else(|e| e.into_inner()) = None;
                return;
            }
        };

        let access_token = match params.get("access_token") {
            Some(t) => t.clone(),
            None => {
                *pending.lock().unwrap_or_else(|e| e.into_inner()) =
                    Some(Err("No access_token in redirect".into()));
                *auth_msg.lock().unwrap_or_else(|e| e.into_inner()) = None;
                return;
            }
        };

        let user = User {
            id: params.get("id").cloned().unwrap_or_default(),
            email: params.get("email").cloned().unwrap_or_default(),
            name: params.get("name").cloned().unwrap_or_default(),
            avatar_url: params.get("avatar_url").cloned(),
            provider: "google".into(),
        };

        let stored = StoredToken {
            id: user.id.clone(),
            email: user.email.clone(),
            name: user.name.clone(),
            avatar_url: user.avatar_url.clone(),
            provider: user.provider.clone(),
            access_token,
            refresh_token: None,
        };
        save_tokens_to_path(token_path, &stored);
        *user_lock.lock().unwrap_or_else(|e| e.into_inner()) = Some(user.clone());
        *auth_msg.lock().unwrap_or_else(|e| e.into_inner()) = None;
        *pending.lock().unwrap_or_else(|e| e.into_inner()) = Some(Ok(user));
    }

    fn sign_in_google(&self) -> Result<User, Box<dyn std::error::Error>> {
        let vercel_url = self.vercel_url.clone();
        Self::run_google_redirect_flow(
            &vercel_url,
            &self.token_path,
            &self.user,
            &self.pending_sign_in,
            &self.auth_msg,
        );

        // Block until the flow completes
        loop {
            if let Some(result) = self
                .pending_sign_in
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take()
            {
                *self.auth_msg.lock().unwrap_or_else(|e| e.into_inner()) = None;
                return result.map_err(|e| e.into());
            }
            thread::sleep(Duration::from_millis(100));
        }
    }

    fn start_sign_in_google(&self) -> Result<(), Box<dyn std::error::Error>> {
        let vercel_url = self.vercel_url.clone();
        let token_path = self.token_path.clone();
        let user_lock = Arc::clone(&self.user);
        let pending = Arc::clone(&self.pending_sign_in);
        let auth_msg = Arc::clone(&self.auth_msg);

        thread::spawn(move || {
            Self::run_google_redirect_flow(
                &vercel_url,
                &token_path,
                &user_lock,
                &pending,
                &auth_msg,
            );
        });

        Ok(())
    }

    fn run_github_device_flow(
        config: &AuthConfig,
        token_path: &PathBuf,
        user_lock: &Arc<Mutex<Option<User>>>,
        pending: &Arc<Mutex<Option<Result<User, String>>>>,
        auth_msg: &Arc<Mutex<Option<String>>>,
    ) {
        let device = match request_device_code(config) {
            Ok(d) => d,
            Err(e) => {
                *pending.lock().unwrap_or_else(|e| e.into_inner()) = Some(Err(e.to_string()));
                *auth_msg.lock().unwrap_or_else(|e| e.into_inner()) = None;
                return;
            }
        };
        let user_code = device.user_code.clone();
        let interval = device.interval.unwrap_or(5);
        let activation_url = format!("https://github.com/login/device?user_code={user_code}");
        *auth_msg.lock().unwrap_or_else(|e| e.into_inner()) = Some(format!(
            "GitHub: enter code {user_code} at github.com/login/device"
        ));
        open_browser(&activation_url);

        let access_token = match poll_device_token(config, &device.device_code, interval) {
            Ok(t) => t,
            Err(e) => {
                *pending.lock().unwrap_or_else(|e| e.into_inner()) = Some(Err(e.to_string()));
                *auth_msg.lock().unwrap_or_else(|e| e.into_inner()) = None;
                return;
            }
        };

        let user = match user_from_token("github", &access_token) {
            Ok(u) => u,
            Err(e) => {
                *pending.lock().unwrap_or_else(|e| e.into_inner()) = Some(Err(e.to_string()));
                *auth_msg.lock().unwrap_or_else(|e| e.into_inner()) = None;
                return;
            }
        };

        let stored = StoredToken {
            id: user.id.clone(),
            email: user.email.clone(),
            name: user.name.clone(),
            avatar_url: user.avatar_url.clone(),
            provider: user.provider.clone(),
            access_token,
            refresh_token: None,
        };
        save_tokens_to_path(token_path, &stored);
        *user_lock.lock().unwrap_or_else(|e| e.into_inner()) = Some(user.clone());
        *auth_msg.lock().unwrap_or_else(|e| e.into_inner()) = None;
        *pending.lock().unwrap_or_else(|e| e.into_inner()) = Some(Ok(user));
    }

    fn sign_in_github(&self) -> Result<User, Box<dyn std::error::Error>> {
        let config = self
            .providers
            .get("github")
            .ok_or_else(|| "GitHub auth not configured".to_string())?;

        let clone = config.clone();
        Self::run_github_device_flow(
            &clone,
            &self.token_path,
            &self.user,
            &self.pending_sign_in,
            &self.auth_msg,
        );

        // Block until the flow completes (read from pending)
        loop {
            if let Some(result) = self
                .pending_sign_in
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take()
            {
                return result.map_err(|e| e.into());
            }
            thread::sleep(Duration::from_millis(100));
        }
    }

    fn start_sign_in_github(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config = self
            .providers
            .get("github")
            .ok_or_else(|| "GitHub auth not configured".to_string())?
            .clone();
        let token_path = self.token_path.clone();
        let user_lock = Arc::clone(&self.user);
        let pending = Arc::clone(&self.pending_sign_in);
        let msg = Arc::clone(&self.auth_msg);

        thread::spawn(move || {
            Self::run_github_device_flow(&config, &token_path, &user_lock, &pending, &msg);
        });

        Ok(())
    }
}

fn save_tokens_to_path(token_path: &PathBuf, stored: &StoredToken) {
    if let Some(parent) = token_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(data) = serde_json::to_string_pretty(stored) {
        let _ = std::fs::write(token_path, data);
    }
}

impl AuthHandle for AuthClient {
    fn current_user(&self) -> Option<User> {
        self.user.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn bearer_token(&self) -> Option<String> {
        let data = std::fs::read_to_string(&self.token_path).ok()?;
        let stored: StoredToken = serde_json::from_str(&data).ok()?;
        Some(stored.access_token)
    }

    fn sign_in(&self, provider: &str) -> Result<User, Box<dyn std::error::Error>> {
        match provider {
            "google" => self.sign_in_google(),
            "github" => self.sign_in_github(),
            _ => Err("unsupported provider".into()),
        }
    }

    fn start_sign_in(&self, provider: &str) -> Result<(), Box<dyn std::error::Error>> {
        match provider {
            "github" => self.start_sign_in_github(),
            "google" => self.start_sign_in_google(),
            _ => Err("unsupported provider".into()),
        }
    }

    fn drain_pending_sign_in(&self) -> Option<Result<User, Box<dyn std::error::Error>>> {
        let mut guard = self
            .pending_sign_in
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        guard.take().map(|r| r.map_err(|e| e.into()))
    }

    fn auth_message(&self) -> Option<String> {
        self.auth_msg
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn sign_out(&self) {
        self.clear_tokens();
        *self.auth_msg.lock().unwrap_or_else(|e| e.into_inner()) = None;
        *self.user.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}
