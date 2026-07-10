use axum::{extract::State, response::Html};
use std::sync::Arc;

use crate::AppState;

pub async fn dashboard(State(_state): State<Arc<AppState>>) -> Html<&'static str> {
    Html(include_str!("../templates/dashboard.html"))
}

pub async fn login_page() -> Html<&'static str> {
    Html(include_str!("../templates/login.html"))
}
