use std::sync::Mutex;

use crate::auth::AuthHandle;

/// A pending key-value operation that needs to be synced to the server.
#[derive(Debug, Clone)]
pub enum SyncOp {
    Set {
        plugin: String,
        key: String,
        value: String,
    },
    Delete {
        plugin: String,
        key: String,
    },
}

/// Best-effort sync client that pushes local DB writes to a remote santui-server.
///
/// - Writes are queued by [`enqueue`](Self::enqueue).
/// - On each [`try_sync`](Self::try_sync) call (driven from the main loop), the
///   queue is drained and pushed to the server via HTTP.
/// - If the server is unreachable or auth fails, the ops stay queued and are
///   retried on the next tick.
#[derive(Debug)]
pub struct SyncClient {
    pub server_url: String,
    jwt: Mutex<Option<String>>,
    pending: Mutex<Vec<SyncOp>>,
}

impl SyncClient {
    pub fn new(server_url: String) -> Self {
        SyncClient {
            server_url,
            jwt: Mutex::new(None),
            pending: Mutex::new(Vec::new()),
        }
    }

    /// Queue an operation for the next sync cycle.
    pub fn enqueue(&self, op: SyncOp) {
        if let Ok(mut pending) = self.pending.lock() {
            pending.push(op);
        }
    }

    /// Attempt to sync all pending operations to the server.
    ///
    /// If no JWT is cached, tries to authenticate using the provided auth
    /// handle's bearer token.  Failures are logged — ops stay queued for
    /// the next call.
    pub fn try_sync(&self, auth: &Option<std::sync::Arc<dyn AuthHandle>>) {
        // Step 1: make sure we have a JWT.
        if self.jwt.lock().map_or(true, |j| j.is_none()) {
            if let Some(auth) = auth {
                if let Some(bearer) = auth.bearer_token() {
                    // Try both providers.
                    for provider in &["github", "google"] {
                        match self.exchange_token(provider, &bearer) {
                            Ok(jwt) => {
                                if let Ok(mut j) = self.jwt.lock() {
                                    *j = Some(jwt);
                                }
                                break;
                            }
                            Err(e) => log::debug!("[sync] {provider} auth failed: {e}"),
                        }
                    }
                }
            }
        }

        // Step 2: drain the queue.
        let ops: Vec<SyncOp> = self
            .pending
            .lock()
            .map_or_else(|_| Vec::new(), |mut p| std::mem::take(&mut *p));

        if ops.is_empty() {
            return;
        }

        let jwt_guard = match self.jwt.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let jwt = match jwt_guard.as_ref() {
            Some(j) => j.clone(),
            None => {
                // No valid JWT — ops stay queued.
                if let Ok(mut pending) = self.pending.lock() {
                    pending.extend(ops);
                }
                return;
            }
        };
        drop(jwt_guard);

        for op in &ops {
            match op {
                SyncOp::Set { plugin, key, value } => {
                    let body = serde_json::json!({
                        "token": &jwt,
                        "values": [{"key": key, "value": value}],
                    });
                    let url = format!("{}/api/v1/data/{}", self.server_url, plugin);
                    let result = ureq::post(&url)
                        .header("Content-Type", "application/json")
                        .send_json(&body);
                    if let Err(e) = result {
                        log::debug!("[sync] push failed for {plugin}/{key}: {e}");
                        if let Ok(mut pending) = self.pending.lock() {
                            pending.push(op.clone());
                        }
                    }
                }
                SyncOp::Delete { plugin, key } => {
                    let url = format!(
                        "{}/api/v1/data/{}/{}?token={}",
                        self.server_url, plugin, key, jwt
                    );
                    if let Err(e) = ureq::delete(&url).call() {
                        log::debug!("[sync] delete failed for {plugin}/{key}: {e}");
                        if let Ok(mut pending) = self.pending.lock() {
                            pending.push(op.clone());
                        }
                    }
                }
            }
        }
    }

    fn exchange_token(
        &self,
        provider: &str,
        bearer: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let url = format!("{}/auth/login", self.server_url);
        let body = serde_json::json!({
            "provider": provider,
            "token": bearer,
        });
        let mut resp = ureq::post(&url)
            .header("Content-Type", "application/json")
            .send_json(&body)?;
        let text = resp.body_mut().read_to_string()?;
        let parsed: serde_json::Value = serde_json::from_str(&text)?;
        parsed["jwt"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "no jwt in response".into())
    }
}
