use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::AppState;
use crate::auth::{AuthError, resolve_token};
use crate::db::DataRow;

#[derive(Deserialize)]
pub struct SyncQuery {
    since: Option<i64>,
    /// Legacy fallback — new clients send `Authorization: Bearer` instead.
    token: Option<String>,
}

#[derive(Deserialize)]
pub struct UpsertRequest {
    /// Legacy fallback — new clients send `Authorization: Bearer` instead.
    token: Option<String>,
    values: Vec<UpsertItem>,
}

#[derive(Deserialize)]
pub struct UpsertItem {
    key: String,
    value: String,
}

#[derive(Deserialize)]
pub struct DeleteQuery {
    /// Legacy fallback — new clients send `Authorization: Bearer` instead.
    token: Option<String>,
}

#[derive(Serialize)]
pub struct SyncResponse {
    values: Vec<DataRow>,
}

pub async fn get_values(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(plugin): Path<String>,
    Query(query): Query<SyncQuery>,
) -> Result<Json<SyncResponse>, AuthError> {
    let auth_user = resolve_token(&headers, query.token.as_deref(), &state.config.jwt_secret)?;
    state.db.ensure_user(&auth_user.user_id).ok();
    let rows = state
        .db
        .list_values(&plugin, &auth_user.user_id, query.since)
        .map_err(|e| {
            tracing::error!("db list_values error: {e}");
            AuthError::InvalidToken
        })?;
    Ok(Json(SyncResponse { values: rows }))
}

pub async fn upsert_values(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(plugin): Path<String>,
    Json(req): Json<UpsertRequest>,
) -> Result<impl IntoResponse, AuthError> {
    let auth_user = resolve_token(&headers, req.token.as_deref(), &state.config.jwt_secret)?;
    state.db.ensure_user(&auth_user.user_id).map_err(|e| {
        tracing::error!("db ensure_user error: {e}");
        AuthError::InvalidToken
    })?;
    for item in &req.values {
        state
            .db
            .upsert_value(&plugin, &auth_user.user_id, &item.key, &item.value)
            .map_err(|e| {
                tracing::error!("db upsert_value error: {e}");
                AuthError::InvalidToken
            })?;
    }
    Ok(StatusCode::OK)
}

pub async fn delete_value(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path((plugin, key)): Path<(String, String)>,
    Query(query): Query<DeleteQuery>,
) -> Result<impl IntoResponse, AuthError> {
    let auth_user = resolve_token(&headers, query.token.as_deref(), &state.config.jwt_secret)?;
    state.db.ensure_user(&auth_user.user_id).ok();
    let deleted = state
        .db
        .delete_value(&plugin, &auth_user.user_id, &key)
        .map_err(|e| {
            tracing::error!("db delete_value error: {e}");
            AuthError::InvalidToken
        })?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Ok(StatusCode::NOT_FOUND)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::create_jwt;
    use crate::config::ServerConfig;
    use std::path::PathBuf;

    const SECRET: &str = "test-secret-for-sync-tests";

    fn test_state() -> (Arc<AppState>, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "santui-sync-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let state = Arc::new(AppState {
            config: ServerConfig {
                port: 0,
                host: String::new(),
                data_dir: PathBuf::new(),
                jwt_secret: SECRET.to_string(),
                stations_db: None,
                google_client_id: None,
            },
            db: crate::db::Database::open(&dir).unwrap(),
            stations: crate::stations::StationsDb::empty(),
        });
        (state, dir)
    }

    fn bearer_headers(jwt: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            format!("Bearer {jwt}").parse().unwrap(),
        );
        headers
    }

    #[tokio::test]
    async fn upsert_and_get_roundtrip_via_header() {
        let (state, dir) = test_state();
        let jwt = create_jwt("user-9", SECRET).unwrap();
        let upsert = upsert_values(
            State(state.clone()),
            bearer_headers(&jwt),
            Path("radio-stream-player".to_string()),
            Json(UpsertRequest {
                token: None,
                values: vec![UpsertItem {
                    key: "favorites".into(),
                    value: "[\"http://a\"]".into(),
                }],
            }),
        )
        .await;
        assert!(upsert.is_ok());

        let got = get_values(
            State(state.clone()),
            bearer_headers(&jwt),
            Path("radio-stream-player".to_string()),
            Query(SyncQuery {
                since: None,
                token: None,
            }),
        )
        .await
        .unwrap();
        assert_eq!(got.values.len(), 1);
        assert_eq!(got.values[0].key, "favorites");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn missing_and_wrong_tokens_rejected() {
        let (state, dir) = test_state();
        let no_auth = get_values(
            State(state.clone()),
            HeaderMap::new(),
            Path("p".to_string()),
            Query(SyncQuery {
                since: None,
                token: None,
            }),
        )
        .await;
        assert!(no_auth.is_err());
        let wrong = get_values(
            State(state.clone()),
            HeaderMap::new(),
            Path("p".to_string()),
            Query(SyncQuery {
                since: None,
                token: Some("garbage".into()),
            }),
        )
        .await;
        assert!(wrong.is_err());
        std::fs::remove_dir_all(&dir).ok();
    }
}
