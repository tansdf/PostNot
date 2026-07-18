use reqwest::Method;
use url::Url;

use crate::{
    domain::requests::{FileRow, KeyValueRow, SendRequestPayload},
    error::{AppError, AppResult},
    services::request_url_service::normalize_request_url,
};

pub(crate) struct PreparedRequest {
    pub method: Method,
    pub base_url: Url,
    pub url: Url,
    pub query_params: Vec<KeyValueRow>,
    pub headers: Vec<KeyValueRow>,
    pub body_mode: PreparedBodyMode,
    pub body_raw: String,
    pub body_fields: Vec<(String, String)>,
    pub body_files: Vec<FileRow>,
}

enum PreparedAuthType {
    None,
    Basic,
    Bearer,
    OAuth2,
    ApiKey,
}

enum PreparedApiKeyLocation {
    Header,
    Query,
}

#[derive(Clone, Copy)]
pub(crate) enum PreparedBodyMode {
    None,
    Raw,
    Json,
    FormUrlencoded,
    Multipart,
}

pub(crate) fn prepare_request(payload: &SendRequestPayload) -> AppResult<PreparedRequest> {
    let method = Method::from_bytes(payload.method.as_bytes())
        .map_err(|error| AppError::Message(error.to_string()))?;
    let base_url = Url::parse(&normalize_request_url(&payload.url))?;
    let mut url = base_url.clone();
    let mut query_params = enabled_rows(&payload.query_params);
    let mut headers = enabled_rows(&payload.headers);
    let auth_type = match payload.auth.auth_type.as_str() {
        "" | "none" => PreparedAuthType::None,
        "basic" => PreparedAuthType::Basic,
        "bearer" => PreparedAuthType::Bearer,
        "oauth2" => PreparedAuthType::OAuth2,
        "api-key" => PreparedAuthType::ApiKey,
        value => return Err(invalid_mode("auth", value)),
    };
    let api_key_location = match payload.auth.api_key_in.as_str() {
        "header" => PreparedApiKeyLocation::Header,
        "query" => PreparedApiKeyLocation::Query,
        value => return Err(invalid_mode("API key location", value)),
    };

    match auth_type {
        PreparedAuthType::Basic => set_generated_header(
            &mut headers,
            "preview-auth-basic",
            "authorization",
            &format!(
                "Basic {}",
                encode_base64(&format!(
                    "{}:{}",
                    payload.auth.basic_username, payload.auth.basic_password
                ))
            ),
        ),
        PreparedAuthType::Bearer => set_generated_header(
            &mut headers,
            "preview-auth-bearer",
            "authorization",
            &format!("Bearer {}", payload.auth.bearer_token),
        ),
        PreparedAuthType::OAuth2 => {
            let token = if payload.auth.oauth2_access_token.trim().is_empty() {
                &payload.auth.bearer_token
            } else {
                &payload.auth.oauth2_access_token
            };
            set_generated_header(
                &mut headers,
                "preview-auth-oauth2",
                "authorization",
                &format!("Bearer {token}"),
            );
        }
        PreparedAuthType::ApiKey
            if matches!(api_key_location, PreparedApiKeyLocation::Query)
                && !payload.auth.api_key_name.trim().is_empty() =>
        {
            query_params.push(KeyValueRow {
                id: "preview-auth-api-key-query".into(),
                key: payload.auth.api_key_name.clone(),
                value: payload.auth.api_key_value.clone(),
                enabled: true,
            });
        }
        PreparedAuthType::ApiKey
            if matches!(api_key_location, PreparedApiKeyLocation::Header)
                && !payload.auth.api_key_name.trim().is_empty() =>
        {
            set_generated_header(
                &mut headers,
                "preview-auth-api-key-header",
                &payload.auth.api_key_name,
                &payload.auth.api_key_value,
            );
        }
        PreparedAuthType::None | PreparedAuthType::ApiKey => {}
    }

    for query in &query_params {
        url.query_pairs_mut().append_pair(&query.key, &query.value);
    }

    let body_mode = match payload.body.mode.as_str() {
        "" | "none" => PreparedBodyMode::None,
        "raw" => PreparedBodyMode::Raw,
        "json" => {
            set_generated_header(
                &mut headers,
                "preview-content-type-json",
                "content-type",
                "application/json",
            );
            PreparedBodyMode::Json
        }
        "form-urlencoded" => {
            set_generated_header(
                &mut headers,
                "preview-content-type-form",
                "content-type",
                "application/x-www-form-urlencoded",
            );
            PreparedBodyMode::FormUrlencoded
        }
        "multipart" => PreparedBodyMode::Multipart,
        value => return Err(invalid_mode("body", value)),
    };
    let body_fields = payload
        .body
        .form
        .iter()
        .filter(|row| row.enabled && !row.key.trim().is_empty())
        .map(|row| (row.key.clone(), row.value.clone()))
        .collect();
    let body_files = payload
        .body
        .files
        .iter()
        .filter(|file| file.enabled && !file.path.trim().is_empty())
        .cloned()
        .collect();

    Ok(PreparedRequest {
        method,
        base_url,
        url,
        query_params,
        headers,
        body_mode,
        body_raw: payload.body.raw.clone(),
        body_fields,
        body_files,
    })
}

fn enabled_rows(rows: &[KeyValueRow]) -> Vec<KeyValueRow> {
    rows.iter()
        .filter(|row| row.enabled && !row.key.trim().is_empty())
        .cloned()
        .collect()
}

fn set_generated_header(headers: &mut Vec<KeyValueRow>, id: &str, key: &str, value: &str) {
    headers.retain(|header| !header.key.eq_ignore_ascii_case(key));
    headers.push(KeyValueRow {
        id: id.into(),
        key: key.into(),
        value: value.into(),
        enabled: true,
    });
}

fn invalid_mode(kind: &str, value: &str) -> AppError {
    AppError::Message(format!("Unsupported request {kind} mode: {value}"))
}

fn encode_base64(value: &str) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(value.len().div_ceil(3) * 4);
    for chunk in value.as_bytes().chunks(3) {
        let bits = u32::from(chunk[0]) << 16
            | u32::from(*chunk.get(1).unwrap_or(&0)) << 8
            | u32::from(*chunk.get(2).unwrap_or(&0));
        encoded.push(TABLE[((bits >> 18) & 63) as usize] as char);
        encoded.push(TABLE[((bits >> 12) & 63) as usize] as char);
        encoded.push(if chunk.len() > 1 {
            TABLE[((bits >> 6) & 63) as usize] as char
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            TABLE[(bits & 63) as usize] as char
        } else {
            '='
        });
    }
    encoded
}

impl PreparedRequest {
    #[cfg(test)]
    pub(crate) fn header_values<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a str> {
        self.headers
            .iter()
            .filter(move |header| header.key.eq_ignore_ascii_case(name))
            .map(|header| header.value.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::prepare_request;
    use crate::domain::requests::{KeyValueRow, RequestAuth, RequestBody, SendRequestPayload};

    #[test]
    fn json_api_key_request_has_one_canonical_shape() {
        let mut payload = payload();
        payload.url = "https://example.test/items".into();
        payload.body.mode = "json".into();
        payload.auth.auth_type = "api-key".into();
        payload.auth.api_key_in = "query".into();
        payload.auth.api_key_name = "key".into();
        payload.auth.api_key_value = "secret".into();
        let plan = prepare_request(&payload).expect("prepare request");
        assert_eq!(plan.url.as_str(), "https://example.test/items?key=secret");
        assert_eq!(
            plan.header_values("content-type").collect::<Vec<_>>(),
            ["application/json"]
        );
    }

    #[test]
    fn generated_headers_replace_user_duplicates() {
        let mut payload = payload();
        payload.body.mode = "json".into();
        payload.headers = vec![row("h", "Content-Type", "text/plain", true)];
        payload.auth.auth_type = "api-key".into();
        payload.auth.api_key_name = "x-key".into();
        payload.auth.api_key_value = "generated".into();
        payload.headers.push(row("a", "X-Key", "user", true));
        let plan = prepare_request(&payload).unwrap();
        assert_eq!(
            plan.header_values("content-type").collect::<Vec<_>>(),
            ["application/json"]
        );
        assert_eq!(
            plan.header_values("x-key").collect::<Vec<_>>(),
            ["generated"]
        );
    }

    #[test]
    fn oauth_falls_back_to_bearer_and_api_key_can_use_header() {
        let mut payload = payload();
        payload.auth.auth_type = "oauth2".into();
        payload.auth.bearer_token = "fallback".into();
        assert_eq!(
            prepare_request(&payload)
                .unwrap()
                .header_values("authorization")
                .next(),
            Some("Bearer fallback")
        );
        payload.auth.oauth2_access_token = "access".into();
        assert_eq!(
            prepare_request(&payload)
                .unwrap()
                .header_values("authorization")
                .next(),
            Some("Bearer access")
        );
        payload.auth.auth_type = "api-key".into();
        payload.auth.api_key_name = "x-api-key".into();
        payload.auth.api_key_value = "value".into();
        assert_eq!(
            prepare_request(&payload)
                .unwrap()
                .header_values("x-api-key")
                .next(),
            Some("value")
        );
    }

    #[test]
    fn disabled_rows_are_removed_and_localhost_is_normalized() {
        let mut payload = payload();
        payload.url = "localhost:8080/path".into();
        payload.query_params = vec![row("on", "a", "1", true), row("off", "b", "2", false)];
        payload.headers = vec![row("off", "x-off", "2", false)];
        let plan = prepare_request(&payload).unwrap();
        assert_eq!(plan.url.as_str(), "http://localhost:8080/path?a=1");
        assert_eq!(plan.query_params.len(), 1);
        assert!(plan.headers.is_empty());
    }

    #[test]
    fn unknown_modes_are_rejected() {
        let mut payload = payload();
        payload.body.mode = "surprise".into();
        assert!(prepare_request(&payload).is_err());
        payload.body.mode = "none".into();
        payload.auth.auth_type = "surprise".into();
        assert!(prepare_request(&payload).is_err());
        payload.auth.auth_type = "none".into();
        payload.auth.api_key_in = "surprise".into();
        assert!(prepare_request(&payload).is_err());
    }

    fn payload() -> SendRequestPayload {
        SendRequestPayload {
            name: "request".into(),
            method: "GET".into(),
            url: "https://example.test".into(),
            query_params: Vec::new(),
            headers: Vec::new(),
            body: RequestBody {
                mode: "none".into(),
                raw: String::new(),
                form: Vec::new(),
                files: Vec::new(),
            },
            auth: RequestAuth {
                auth_type: "none".into(),
                basic_username: String::new(),
                basic_password: String::new(),
                bearer_token: String::new(),
                api_key_name: String::new(),
                api_key_value: String::new(),
                api_key_in: "header".into(),
                oauth2_access_token: String::new(),
                oauth2_token_url: String::new(),
                oauth2_client_id: String::new(),
                oauth2_client_secret: String::new(),
                oauth2_scope: String::new(),
            },
            pre_request_script: String::new(),
            test_script: String::new(),
        }
    }

    fn row(id: &str, key: &str, value: &str, enabled: bool) -> KeyValueRow {
        KeyValueRow {
            id: id.into(),
            key: key.into(),
            value: value.into(),
            enabled,
        }
    }
}
