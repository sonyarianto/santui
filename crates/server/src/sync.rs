use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::auth::{extract_user, AuthError};
use crate::db::DataRow;
use crate::AppState;

#[derive(Deserialize)]
pub struct SyncQuery {
    since: Option<i64>,
    token: String,
}

#[derive(Deserialize)]
pub struct UpsertRequest {
    token: String,
    values: Vec<UpsertItem>,
}

#[derive(Deserialize)]
pub struct UpsertItem {
    key: String,
    value: String,
}

#[derive(Deserialize)]
pub struct DeleteQuery {
    token: String,
}

#[derive(Serialize)]
pub struct SyncResponse {
    values: Vec<DataRow>,
}

pub async fn get_values(
    State(state): State<Arc<AppState>>,
    Path(plugin): Path<String>,
    Query(query): Query<SyncQuery>,
) -> Result<Json<SyncResponse>, AuthError> {
    let auth_user = extract_user(&query.token, &state.config.jwt_secret)?;
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
    Path(plugin): Path<String>,
    Json(req): Json<UpsertRequest>,
) -> Result<impl IntoResponse, AuthError> {
    let auth_user = extract_user(&req.token, &state.config.jwt_secret)?;
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
    Path((plugin, key)): Path<(String, String)>,
    Query(query): Query<DeleteQuery>,
) -> Result<impl IntoResponse, AuthError> {
    let auth_user = extract_user(&query.token, &state.config.jwt_secret)?;
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
