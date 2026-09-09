use axum::{
    extract::{Json, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

use std::sync::Arc;

use crate::AppState;
use crate::db::UserRow;

// ─── JWT Claims ───

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
}

pub(crate) fn create_jwt(
    user_id: &str,
    secret: &str,
) -> Result<String, jsonwebtoken::errors::Error> {
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

/// Canonical token source: `Authorization: Bearer <jwt>`.
///
/// Tokens in URLs/bodies get written to access logs and proxies; the header
/// does not. Query/body `token` fields remain accepted as a fallback for old
/// clients (see `resolve_token`), but new clients must use the header.
pub fn extract_bearer(headers: &HeaderMap) -> Option<String> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, credentials) = value.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }
    let token = credentials.trim().to_string();
    (!token.is_empty()).then_some(token)
}

/// Resolve the caller's JWT: header first, legacy `token` field as fallback.
pub fn resolve_token(
    headers: &HeaderMap,
    fallback: Option<&str>,
    secret: &str,
) -> Result<AuthUser, AuthError> {
    let token = extract_bearer(headers)
        .or_else(|| fallback.map(|s| s.to_string()))
        .ok_or(AuthError::InvalidToken)?;
    extract_user(&token, secret)
}

// ─── Error type ───

#[derive(Debug)]
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

fn verify_github_token(token: &str, app_creds: Option<(&str, &str)>) -> Result<UserInfo, String> {
    if let Some((client_id, client_secret)) = app_creds {
        // Bind the token to OUR OAuth App: without this, a token issued to
        // any other GitHub App (or a pasted PAT) would be accepted here.
        // The applications API answers 200 only for our own tokens.
        use base64::Engine as _;
        let credentials = base64::engine::general_purpose::STANDARD
            .encode(format!("{client_id}:{client_secret}"));
        let owner_url = format!("https://api.github.com/applications/{client_id}/token");
        let body = serde_json::json!({ "access_token": token });
        ureq::post(&owner_url)
            .header("Accept", "application/vnd.github.v3+json")
            .header("Authorization", format!("Basic {credentials}"))
            .send_json(&body)
            .map_err(|_| "GitHub token not issued to this app".to_string())?;
    }
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

fn verify_google_token(token: &str, expected_aud: Option<&str>) -> Result<UserInfo, String> {
    if let Some(expected) = expected_aud {
        // Bind the token to OUR OAuth client: without this, a token issued
        // to any other Google app would be accepted here (cross-app replay).
        let mut resp = ureq::get(&format!(
            "https://oauth2.googleapis.com/tokeninfo?access_token={token}"
        ))
        .call()
        .map_err(|e| format!("Google tokeninfo error: {e}"))?;
        let text = resp
            .body_mut()
            .read_to_string()
            .map_err(|e| format!("read error: {e}"))?;
        let info: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("parse error: {e}"))?;
        if !token_aud_matches(&info, expected) {
            return Err("Google token audience mismatch".to_string());
        }
    }
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

/// Pure audience check over a tokeninfo document (kept separate for tests).
fn token_aud_matches(info: &serde_json::Value, expected: &str) -> bool {
    info.get("aud").and_then(|v| v.as_str()) == Some(expected)
}

// ─── Route handlers ───

pub async fn post_login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AuthError> {
    let user_info = match req.provider.as_str() {
        "github" => {
            let creds = match (
                state.config.github_client_id.as_deref(),
                state.config.github_client_secret.as_deref(),
            ) {
                (Some(id), Some(secret)) => Some((id, secret)),
                _ => None,
            };
            verify_github_token(&req.token, creds).map_err(|_| AuthError::WrongCredentials)?
        }
        "google" => verify_google_token(&req.token, state.config.google_client_id.as_deref())
            .map_err(|_| AuthError::WrongCredentials)?,
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
    headers: HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<UserInfo>, AuthError> {
    let query_token = params.get("token").map(|s| s.as_str());
    let auth_user = resolve_token(&headers, query_token, &state.config.jwt_secret)?;
    let user_row = state
        .db
        .get_user(&auth_user.user_id)
        .map_err(|_| AuthError::InvalidToken)?
        .ok_or(AuthError::InvalidToken)?;
    Ok(Json(user_row.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "test-secret-for-unit-tests";

    fn bearer(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            format!("Bearer {value}").parse().unwrap(),
        );
        headers
    }

    #[test]
    fn extract_bearer_parses_valid_header() {
        assert_eq!(
            extract_bearer(&bearer("abc.def.ghi")).as_deref(),
            Some("abc.def.ghi")
        );
    }

    #[test]
    fn extract_bearer_rejects_malformed() {
        assert_eq!(extract_bearer(&HeaderMap::new()), None);
        let mut wrong_scheme = HeaderMap::new();
        wrong_scheme.insert(header::AUTHORIZATION, "Basic abc".parse().unwrap());
        assert_eq!(extract_bearer(&wrong_scheme), None);
        // Scheme is case-insensitive per RFC 9110, empty credentials rejected.
        let mut lower = HeaderMap::new();
        lower.insert(header::AUTHORIZATION, "bearer xyz".parse().unwrap());
        assert_eq!(extract_bearer(&lower).as_deref(), Some("xyz"));
        let mut empty = HeaderMap::new();
        empty.insert(header::AUTHORIZATION, "Bearer ".parse().unwrap());
        assert_eq!(extract_bearer(&empty), None);
        let mut nospace = HeaderMap::new();
        nospace.insert(header::AUTHORIZATION, "Bearerabc".parse().unwrap());
        assert_eq!(extract_bearer(&nospace), None);
    }

    #[test]
    fn resolve_token_prefers_header_over_fallback() {
        let jwt = create_jwt("user-1", SECRET).unwrap();
        // Valid header wins even with a garbage fallback.
        let user = resolve_token(&bearer(&jwt), Some("garbage"), SECRET).unwrap();
        assert_eq!(user.user_id, "user-1");
        // Valid fallback still works when the header is absent/invalid.
        let user = resolve_token(&HeaderMap::new(), Some(&jwt), SECRET).unwrap();
        assert_eq!(user.user_id, "user-1");
        // Nothing usable anywhere is rejected.
        assert!(resolve_token(&HeaderMap::new(), None, SECRET).is_err());
        assert!(resolve_token(&HeaderMap::new(), Some("garbage"), SECRET).is_err());
    }

    #[test]
    fn token_aud_matches_expected_client() {
        let info = serde_json::json!({"aud": "my-client-id.apps.googleusercontent.com"});
        assert!(token_aud_matches(
            &info,
            "my-client-id.apps.googleusercontent.com"
        ));
        assert!(!token_aud_matches(&info, "other-client-id"));
        assert!(!token_aud_matches(&serde_json::json!({}), "my-client-id"));
        assert!(!token_aud_matches(&serde_json::json!({"aud": 123}), "123"));
    }
}
