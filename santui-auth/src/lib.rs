use oauth2::basic::BasicClient;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge, RedirectUrl,
    Scope, TokenResponse, TokenUrl,
};
use santui_core::auth::{AuthHandle, User};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Mutex;

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
    pub client_secret: String,
    pub auth_uri: String,
    pub token_uri: String,
    pub scopes: Vec<String>,
    pub redirect_port: u16,
}

impl AuthConfig {
    pub fn google(client_id: String, client_secret: String) -> Self {
        AuthConfig {
            client_id,
            client_secret,
            auth_uri: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            token_uri: "https://oauth2.googleapis.com/token".into(),
            scopes: vec!["openid".into(), "email".into(), "profile".into()],
            redirect_port: 9842,
        }
    }

    pub fn github(client_id: String, client_secret: String) -> Self {
        AuthConfig {
            client_id,
            client_secret,
            auth_uri: "https://github.com/login/oauth/authorize".into(),
            token_uri: "https://github.com/login/oauth/access_token".into(),
            scopes: vec!["read:user".into(), "user:email".into()],
            redirect_port: 9843,
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

fn handle_redirect(listener: TcpListener) -> Result<String, Box<dyn std::error::Error>> {
    let (stream, _) = listener.accept()?;
    let mut reader = BufReader::new(&stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;

    let code = request_line
        .split_whitespace()
        .nth(1)
        .and_then(|path| {
            let path = path.trim_start_matches("/callback?");
            for pair in path.split('&') {
                if let Some(val) = pair.strip_prefix("code=") {
                    return Some(val.to_string());
                }
            }
            None
        })
        .ok_or_else(|| "No authorization code in redirect".to_string())?;

    let response =
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<html><body><h1>Signed in! You can close this window.</h1></body></html>";
    let mut stream = stream;
    let _ = stream.write_all(response.as_bytes());

    Ok(code)
}

fn user_from_token(provider: &str, access_token: &str) -> Result<User, Box<dyn std::error::Error>> {
    // Fetch user info from the provider's userinfo endpoint
    match provider {
        "google" => {
            let resp = ureq::get("https://www.googleapis.com/oauth2/v2/userinfo")
                .set("Authorization", &format!("Bearer {access_token}"))
                .call()?;
            let body: serde_json::Value = serde_json::from_str(&resp.into_string()?)?;
            Ok(User {
                id: body["id"].as_str().unwrap_or("").into(),
                email: body["email"].as_str().unwrap_or("").into(),
                name: body["name"].as_str().unwrap_or("").into(),
                avatar_url: body["picture"].as_str().map(|s| s.into()),
                provider: provider.into(),
            })
        }
        "github" => {
            let resp = ureq::get("https://api.github.com/user")
                .set("Authorization", &format!("Bearer {access_token}"))
                .set("Accept", "application/vnd.github.v3+json")
                .call()?;
            let body: serde_json::Value = serde_json::from_str(&resp.into_string()?)?;
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
    config: AuthConfig,
    user: Mutex<Option<User>>,
    token_path: PathBuf,
}

impl AuthClient {
    pub fn new(config: AuthConfig) -> Self {
        let token_path = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("santui")
            .join("auth-tokens.json");
        let user = Self::load_tokens(&token_path);
        AuthClient {
            config,
            user: Mutex::new(user),
            token_path,
        }
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

    fn save_tokens(&self, stored: &StoredToken) {
        if let Some(parent) = self.token_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(data) = serde_json::to_string_pretty(stored) {
            let _ = std::fs::write(&self.token_path, data);
        }
    }

    fn clear_tokens(&self) {
        let _ = std::fs::remove_file(&self.token_path);
    }

    fn build_oauth_client(
        &self,
        provider: &str,
    ) -> Result<BasicClient, Box<dyn std::error::Error>> {
        let (client_id, client_secret, auth_uri, token_uri, port) = match provider {
            "google" => (
                self.config.client_id.clone(),
                self.config.client_secret.clone(),
                self.config.auth_uri.clone(),
                self.config.token_uri.clone(),
                self.config.redirect_port,
            ),
            "github" => (
                self.config.client_id.clone(),
                self.config.client_secret.clone(),
                self.config.auth_uri.clone(),
                self.config.token_uri.clone(),
                self.config.redirect_port,
            ),
            _ => return Err("unsupported provider".into()),
        };

        let client = BasicClient::new(
            ClientId::new(client_id),
            Some(ClientSecret::new(client_secret)),
            AuthUrl::new(auth_uri)?,
            Some(TokenUrl::new(token_uri)?),
        )
        .set_redirect_uri(RedirectUrl::new(format!(
            "http://127.0.0.1:{port}/callback"
        ))?);

        Ok(client)
    }
}

impl AuthHandle for AuthClient {
    fn current_user(&self) -> Option<User> {
        self.user.lock().unwrap().clone()
    }

    fn bearer_token(&self) -> Option<String> {
        let data = std::fs::read_to_string(&self.token_path).ok()?;
        let stored: StoredToken = serde_json::from_str(&data).ok()?;
        Some(stored.access_token)
    }

    fn sign_in(&self, provider: &str) -> Result<User, Box<dyn std::error::Error>> {
        let client = self.build_oauth_client(provider)?;
        let port = self.config.redirect_port;

        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
        let scopes: Vec<Scope> = self
            .config
            .scopes
            .iter()
            .map(|s| Scope::new(s.clone()))
            .collect();

        let mut auth_req = client.authorize_url(CsrfToken::new_random);
        for scope in &scopes {
            auth_req = auth_req.add_scope(scope.clone());
        }
        let (auth_url, _csrf_token) = auth_req.set_pkce_challenge(pkce_challenge).url();

        open_browser(auth_url.as_str());

        let listener = TcpListener::bind(("127.0.0.1", port))?;
        let code = handle_redirect(listener)?;

        let token = client
            .exchange_code(AuthorizationCode::new(code))
            .set_pkce_verifier(pkce_verifier)
            .request(oauth2::ureq::http_client)?;

        let access_token = token.access_token().secret().clone();
        let refresh_token = token.refresh_token().map(|t| t.secret().clone());

        let user = user_from_token(provider, &access_token)?;

        let stored = StoredToken {
            id: user.id.clone(),
            email: user.email.clone(),
            name: user.name.clone(),
            avatar_url: user.avatar_url.clone(),
            provider: user.provider.clone(),
            access_token,
            refresh_token,
        };

        self.save_tokens(&stored);
        *self.user.lock().unwrap() = Some(user.clone());

        Ok(user)
    }

    fn sign_out(&self) {
        self.clear_tokens();
        *self.user.lock().unwrap() = None;
    }
}
