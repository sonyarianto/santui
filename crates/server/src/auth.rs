use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use std::sync::Arc;

use crate::db::UserRow;
use crate::AppState;

// ─── JWT Claims ───

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
}

fn create_jwt(user_id: &str, secret: &str) -> Result<String, jsonwebtoken::errors::Error> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as usize;
    let claims = Claims {
        sub: user_id.to_string(),
        exp: now + 86400 * 7,
        iat: now,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_ref()),
    )
}

fn verify_jwt(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_ref()),
        &Validation::default(),
    )?;
    Ok(token_data.claims)
}

// ─── Extractors ───

pub struct AuthUser {
    pub user_id: String,
}

pub fn extract_user(token: &str, secret: &str) -> Result<AuthUser, AuthError> {
    let claims = verify_jwt(token, secret).map_err(|_| AuthError::InvalidToken)?;
    Ok(AuthUser {
        user_id: claims.sub,
    })
}

// ─── Error type ───

pub enum AuthError {
    InvalidToken,
    WrongCredentials,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            AuthError::InvalidToken => (StatusCode::UNAUTHORIZED, "invalid token"),
            AuthError::WrongCredentials => (StatusCode::UNAUTHORIZED, "wrong credentials"),
        };
        (status, Json(serde_json::json!({"error": msg}))).into_response()
    }
}

// ─── Request / Response types ───

#[derive(Deserialize)]
pub struct LoginRequest {
    pub provider: String,
    pub token: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub jwt: String,
    pub user: UserInfo,
}

#[derive(Serialize)]
pub struct UserInfo {
    pub id: String,
    pub provider: String,
    pub email: String,
    pub name: String,
    pub avatar_url: Option<String>,
}

impl From<UserRow> for UserInfo {
    fn from(u: UserRow) -> Self {
        UserInfo {
            id: u.id,
            provider: u.provider,
            email: u.email,
            name: u.name,
            avatar_url: u.avatar_url,
        }
    }
}

// ─── Provider verification ───

fn verify_github_token(token: &str) -> Result<UserInfo, String> {
    let mut resp = ureq::get("https://api.github.com/user")
        .header("Authorization", &format!("Bearer {token}"))
        .header("Accept", "application/vnd.github.v3+json")
        .call()
        .map_err(|e| format!("GitHub API error: {e}"))?;
    let text = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("read error: {e}"))?;
    let body: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("parse error: {e}"))?;
    Ok(UserInfo {
        id: format!(
            "gh_{}",
            body["id"]
                .as_u64()
                .map(|n| n.to_string())
                .unwrap_or_default()
        ),
        email: body["email"].as_str().unwrap_or("").to_string(),
        name: body["login"].as_str().unwrap_or("").to_string(),
        avatar_url: body["avatar_url"].as_str().map(|s| s.to_string()),
        provider: "github".to_string(),
    })
}

fn verify_google_token(token: &str) -> Result<UserInfo, String> {
    let mut resp = ureq::get("https://www.googleapis.com/oauth2/v3/userinfo")
        .header("Authorization", &format!("Bearer {token}"))
        .call()
        .map_err(|e| format!("Google API error: {e}"))?;
    let text = resp
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("read error: {e}"))?;
    let body: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("parse error: {e}"))?;
    Ok(UserInfo {
        id: format!("google_{}", body["sub"].as_str().unwrap_or_default()),
        email: body["email"].as_str().unwrap_or("").to_string(),
        name: body["name"].as_str().unwrap_or("").to_string(),
        avatar_url: body["picture"].as_str().map(|s| s.to_string()),
        provider: "google".to_string(),
    })
}

// ─── Route handlers ───

pub async fn post_login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AuthError> {
    let user_info = match req.provider.as_str() {
        "github" => verify_github_token(&req.token).map_err(|_| AuthError::WrongCredentials)?,
        "google" => verify_google_token(&req.token).map_err(|_| AuthError::WrongCredentials)?,
        _ => return Err(AuthError::WrongCredentials),
    };

    let db_row = UserRow {
        id: user_info.id.clone(),
        provider: user_info.provider.clone(),
        email: user_info.email.clone(),
        name: user_info.name.clone(),
        avatar_url: user_info.avatar_url.clone(),
        created_at: String::new(),
    };

    state
        .db
        .upsert_user(&db_row)
        .map_err(|_| AuthError::WrongCredentials)?;

    let jwt =
        create_jwt(&user_info.id, &state.config.jwt_secret).map_err(|_| AuthError::InvalidToken)?;

    Ok(Json(LoginResponse {
        jwt,
        user: user_info,
    }))
}

pub async fn me(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<UserInfo>, AuthError> {
    let token = params.get("token").ok_or(AuthError::InvalidToken)?;
    let auth_user = extract_user(token, &state.config.jwt_secret)?;
    let user_row = state
        .db
        .get_user(&auth_user.user_id)
        .map_err(|_| AuthError::InvalidToken)?
        .ok_or(AuthError::InvalidToken)?;
    Ok(Json(user_row.into()))
}
