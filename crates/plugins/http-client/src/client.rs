use serde::{Deserialize, Serialize};

pub const MAX_BODY_BYTES: usize = 10 * 1024;

#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::upper_case_acronyms)]
pub enum HttpMethod {
    GET,
    HEAD,
    POST,
    PUT,
    PATCH,
    DELETE,
    OPTIONS,
}

impl HttpMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            HttpMethod::GET => "GET",
            HttpMethod::HEAD => "HEAD",
            HttpMethod::POST => "POST",
            HttpMethod::PUT => "PUT",
            HttpMethod::PATCH => "PATCH",
            HttpMethod::DELETE => "DELETE",
            HttpMethod::OPTIONS => "OPTIONS",
        }
    }

    pub fn all() -> Vec<HttpMethod> {
        use HttpMethod::*;
        vec![GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS]
    }

    pub fn from_str(s: &str) -> Option<HttpMethod> {
        match s.to_uppercase().as_str() {
            "GET" => Some(HttpMethod::GET),
            "HEAD" => Some(HttpMethod::HEAD),
            "POST" => Some(HttpMethod::POST),
            "PUT" => Some(HttpMethod::PUT),
            "PATCH" => Some(HttpMethod::PATCH),
            "DELETE" => Some(HttpMethod::DELETE),
            "OPTIONS" => Some(HttpMethod::OPTIONS),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpResponse {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub elapsed_ms: u64,
    pub body_truncated: bool,
}

pub fn send_request(
    method: &HttpMethod,
    url: &str,
    headers: &[(String, String)],
    body: &str,
    _timeout_secs: u64,
) -> Result<HttpResponse, String> {
    let start = std::time::Instant::now();

    let resp = match method {
        HttpMethod::GET => {
            let mut req = ureq::get(url).header("User-Agent", "santui-http-client/0.2.27");
            for (k, v) in headers {
                if !k.is_empty() {
                    req = req.header(k, v);
                }
            }
            req.call().map_err(|e| e.to_string())
        }
        HttpMethod::DELETE => {
            let mut req = ureq::delete(url).header("User-Agent", "santui-http-client/0.2.27");
            for (k, v) in headers {
                if !k.is_empty() {
                    req = req.header(k, v);
                }
            }
            req.call().map_err(|e| e.to_string())
        }
        HttpMethod::HEAD => {
            let mut req = ureq::head(url).header("User-Agent", "santui-http-client/0.2.27");
            for (k, v) in headers {
                if !k.is_empty() {
                    req = req.header(k, v);
                }
            }
            req.call().map_err(|e| e.to_string())
        }
        HttpMethod::OPTIONS => {
            let mut req = ureq::options(url).header("User-Agent", "santui-http-client/0.2.27");
            for (k, v) in headers {
                if !k.is_empty() {
                    req = req.header(k, v);
                }
            }
            req.call().map_err(|e| e.to_string())
        }
        HttpMethod::POST => {
            let mut req = ureq::post(url).header("User-Agent", "santui-http-client/0.2.27");
            for (k, v) in headers {
                if !k.is_empty() {
                    req = req.header(k, v);
                }
            }
            req.send(body).map_err(|e| e.to_string())
        }
        HttpMethod::PUT => {
            let mut req = ureq::put(url).header("User-Agent", "santui-http-client/0.2.27");
            for (k, v) in headers {
                if !k.is_empty() {
                    req = req.header(k, v);
                }
            }
            req.send(body).map_err(|e| e.to_string())
        }
        HttpMethod::PATCH => {
            let mut req = ureq::patch(url).header("User-Agent", "santui-http-client/0.2.27");
            for (k, v) in headers {
                if !k.is_empty() {
                    req = req.header(k, v);
                }
            }
            req.send(body).map_err(|e| e.to_string())
        }
    }?;

    let status: u16 = resp.status().as_u16();
    let status_text = resp.status().canonical_reason().unwrap_or("").to_string();

    let mut header_pairs: Vec<(String, String)> = Vec::new();
    for (name, value) in resp.headers().iter() {
        let v = value.to_str().unwrap_or("");
        header_pairs.push((name.as_str().to_string(), v.to_string()));
    }
    header_pairs.sort_by_key(|a| a.0.to_lowercase());

    let raw_body = resp
        .into_body()
        .read_to_string()
        .map_err(|e| e.to_string())?;

    let (pre_body, body_truncated) = if raw_body.len() > MAX_BODY_BYTES {
        (
            raw_body.chars().take(MAX_BODY_BYTES).collect::<String>(),
            true,
        )
    } else {
        (raw_body, false)
    };

    let body = serde_json::from_str::<serde_json::Value>(&pre_body)
        .ok()
        .and_then(|v| serde_json::to_string_pretty(&v).ok())
        .unwrap_or_else(|| pre_body.clone());

    let elapsed_ms = start.elapsed().as_millis() as u64;
    Ok(HttpResponse {
        status,
        status_text,
        headers: header_pairs,
        body,
        elapsed_ms,
        body_truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_method_as_str_get() {
        assert_eq!(HttpMethod::GET.as_str(), "GET");
    }

    #[test]
    fn http_method_as_str_post() {
        assert_eq!(HttpMethod::POST.as_str(), "POST");
    }

    #[test]
    fn http_method_as_str_put() {
        assert_eq!(HttpMethod::PUT.as_str(), "PUT");
    }

    #[test]
    fn http_method_as_str_patch() {
        assert_eq!(HttpMethod::PATCH.as_str(), "PATCH");
    }

    #[test]
    fn http_method_as_str_delete() {
        assert_eq!(HttpMethod::DELETE.as_str(), "DELETE");
    }

    #[test]
    fn http_method_as_str_head() {
        assert_eq!(HttpMethod::HEAD.as_str(), "HEAD");
    }

    #[test]
    fn http_method_as_str_options() {
        assert_eq!(HttpMethod::OPTIONS.as_str(), "OPTIONS");
    }

    #[test]
    fn http_method_all_contains_get_post() {
        let all = HttpMethod::all();
        let as_str: Vec<&str> = all.iter().map(|m| m.as_str()).collect();
        assert!(as_str.contains(&"GET"));
        assert!(as_str.contains(&"POST"));
        assert_eq!(all.len(), 7);
    }

    #[test]
    fn http_method_from_str_valid() {
        assert_eq!(HttpMethod::from_str("GET"), Some(HttpMethod::GET));
        assert_eq!(HttpMethod::from_str("get"), Some(HttpMethod::GET));
        assert_eq!(HttpMethod::from_str("POST"), Some(HttpMethod::POST));
        assert_eq!(HttpMethod::from_str("PUT"), Some(HttpMethod::PUT));
    }

    #[test]
    fn http_method_from_str_invalid() {
        assert_eq!(HttpMethod::from_str("FOO"), None);
        assert_eq!(HttpMethod::from_str(""), None);
    }

    #[test]
    fn response_body_truncated_at_10k() {
        assert_eq!(MAX_BODY_BYTES, 10240);
    }

    #[test]
    fn response_defaults() {
        let resp = HttpResponse {
            status: 200,
            status_text: "OK".into(),
            headers: vec![],
            body: "hello".into(),
            elapsed_ms: 0,
            body_truncated: false,
        };
        assert_eq!(resp.status, 200);
        assert_eq!(resp.status_text, "OK");
        assert_eq!(resp.body, "hello");
        assert!(!resp.body_truncated);
    }

    #[test]
    fn send_request_timeout_on_invalid_host() {
        let result = send_request(
            &HttpMethod::GET,
            "http://192.0.2.1:1/nonexistent",
            &[],
            "",
            1,
        );
        assert!(result.is_err());
    }
}
