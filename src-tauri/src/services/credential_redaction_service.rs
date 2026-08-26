use crate::{
    domain::{
        realtime::{RawMessageMode, RealtimeConnectionDraft, RealtimeMessageDraft},
        requests::SendRequestPayload,
        workspace_portability::WorkspaceRedaction,
    },
    error::AppResult,
};

const VARIABLE_TOKEN_START: &str = "{{";

pub fn redact_request(
    request: &mut SendRequestPayload,
    resource_kind: &str,
    resource_export_id: &str,
    redactions: &mut Vec<WorkspaceRedaction>,
) {
    redact_url(
        &mut request.url,
        resource_kind,
        resource_export_id,
        "url",
        redactions,
    );

    for (index, row) in request.headers.iter_mut().enumerate() {
        if is_sensitive_key(&row.key) {
            redact_string(
                &mut row.value,
                resource_kind,
                resource_export_id,
                &format!("headers[{index}].value"),
                "The header name looks like a credential field.",
                redactions,
            );
        }
    }
    for (index, row) in request.query_params.iter_mut().enumerate() {
        if is_sensitive_key(&row.key) {
            redact_string(
                &mut row.value,
                resource_kind,
                resource_export_id,
                &format!("queryParams[{index}].value"),
                "The query parameter name looks like a credential field.",
                redactions,
            );
        }
    }
    for (index, row) in request.body.form.iter_mut().enumerate() {
        if is_sensitive_key(&row.key) {
            redact_string(
                &mut row.value,
                resource_kind,
                resource_export_id,
                &format!("body.form[{index}].value"),
                "The body field name looks like a credential field.",
                redactions,
            );
        }
    }
    redact_json_string(
        &mut request.body.raw,
        resource_kind,
        resource_export_id,
        "body.raw",
        redactions,
    );

    let auth = &mut request.auth;
    redact_string(
        &mut auth.basic_password,
        resource_kind,
        resource_export_id,
        "auth.basicPassword",
        "Basic-auth passwords are credentials.",
        redactions,
    );
    redact_string(
        &mut auth.bearer_token,
        resource_kind,
        resource_export_id,
        "auth.bearerToken",
        "Bearer tokens grant API access.",
        redactions,
    );
    redact_string(
        &mut auth.api_key_value,
        resource_kind,
        resource_export_id,
        "auth.apiKeyValue",
        "API key values are credentials.",
        redactions,
    );
    redact_string(
        &mut auth.oauth2_access_token,
        resource_kind,
        resource_export_id,
        "auth.oauth2AccessToken",
        "OAuth2 access tokens grant API access.",
        redactions,
    );
    redact_string(
        &mut auth.oauth2_client_secret,
        resource_kind,
        resource_export_id,
        "auth.oauth2ClientSecret",
        "OAuth2 client secrets are credentials.",
        redactions,
    );
    redact_url(
        &mut auth.oauth2_token_url,
        resource_kind,
        resource_export_id,
        "auth.oauth2TokenUrl",
        redactions,
    );
}

pub fn redact_realtime_connection(
    connection: &mut RealtimeConnectionDraft,
    resource_kind: &str,
    resource_export_id: &str,
    redactions: &mut Vec<WorkspaceRedaction>,
) -> AppResult<()> {
    let mut value = serde_json::to_value(&*connection)?;
    redact_json_value(
        &mut value,
        resource_kind,
        resource_export_id,
        "connection",
        redactions,
    );
    *connection = serde_json::from_value(value)?;
    Ok(())
}

pub fn redact_realtime_message(
    message: &mut RealtimeMessageDraft,
    resource_kind: &str,
    resource_export_id: &str,
    redactions: &mut Vec<WorkspaceRedaction>,
) -> AppResult<()> {
    if let RealtimeMessageDraft::Websocket { composer, .. } = message {
        if composer.mode == RawMessageMode::Json {
            redact_json_string(
                &mut composer.content,
                resource_kind,
                resource_export_id,
                "message.composer.content",
                redactions,
            );
        }
        return Ok(());
    }

    let mut value = serde_json::to_value(&*message)?;
    redact_json_value(
        &mut value,
        resource_kind,
        resource_export_id,
        "message",
        redactions,
    );
    *message = serde_json::from_value(value)?;
    Ok(())
}

pub fn contains_local_files(request: &SendRequestPayload) -> bool {
    request
        .body
        .files
        .iter()
        .any(|file| file.enabled && !file.path.trim().is_empty())
}

pub fn has_scripts(request: &SendRequestPayload) -> bool {
    !request.pre_request_script.trim().is_empty() || !request.test_script.trim().is_empty()
}

fn redact_json_string(
    source: &mut String,
    resource_kind: &str,
    resource_export_id: &str,
    path: &str,
    redactions: &mut Vec<WorkspaceRedaction>,
) {
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(source) else {
        return;
    };
    let before = redactions.len();
    redact_json_value(
        &mut value,
        resource_kind,
        resource_export_id,
        path,
        redactions,
    );
    if redactions.len() > before {
        if let Ok(serialized) = serde_json::to_string_pretty(&value) {
            *source = serialized;
        }
    }
}

fn redact_json_value(
    value: &mut serde_json::Value,
    resource_kind: &str,
    resource_export_id: &str,
    path: &str,
    redactions: &mut Vec<WorkspaceRedaction>,
) {
    match value {
        serde_json::Value::Object(map) => {
            let row_key = map
                .get("key")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string);
            if let (Some(key), Some(row_value)) = (row_key.as_deref(), map.get_mut("value")) {
                if is_sensitive_key(key) {
                    redact_json_scalar(
                        row_value,
                        resource_kind,
                        resource_export_id,
                        &format!("{path}.value"),
                        "The field name looks like a credential field.",
                        redactions,
                    );
                }
            }

            for (key, child) in map.iter_mut() {
                let child_path = format!("{path}.{key}");
                if key == "url" {
                    if let Some(text) = child.as_str() {
                        let mut next = text.to_string();
                        redact_url(
                            &mut next,
                            resource_kind,
                            resource_export_id,
                            &child_path,
                            redactions,
                        );
                        *child = serde_json::Value::String(next);
                    }
                } else if is_sensitive_key(key)
                    && !matches!(
                        child,
                        serde_json::Value::Object(_) | serde_json::Value::Array(_)
                    )
                {
                    redact_json_scalar(
                        child,
                        resource_kind,
                        resource_export_id,
                        &child_path,
                        "The property name looks like a credential field.",
                        redactions,
                    );
                } else {
                    redact_json_value(
                        child,
                        resource_kind,
                        resource_export_id,
                        &child_path,
                        redactions,
                    );
                }
            }
        }
        serde_json::Value::Array(items) => {
            for (index, item) in items.iter_mut().enumerate() {
                redact_json_value(
                    item,
                    resource_kind,
                    resource_export_id,
                    &format!("{path}[{index}]"),
                    redactions,
                );
            }
        }
        _ => {}
    }
}

fn redact_json_scalar(
    value: &mut serde_json::Value,
    resource_kind: &str,
    resource_export_id: &str,
    path: &str,
    reason: &str,
    redactions: &mut Vec<WorkspaceRedaction>,
) {
    let source = match &*value {
        serde_json::Value::String(value) => value.as_str(),
        serde_json::Value::Null => return,
        other => {
            if other.to_string().is_empty() {
                return;
            }
            "non-empty"
        }
    };
    if source.is_empty() || has_variable_token(source) {
        return;
    }
    *value = serde_json::Value::String(String::new());
    push_redaction(resource_kind, resource_export_id, path, reason, redactions);
}

fn redact_url(
    source: &mut String,
    resource_kind: &str,
    resource_export_id: &str,
    path: &str,
    redactions: &mut Vec<WorkspaceRedaction>,
) {
    let Ok(mut url) = url::Url::parse(source) else {
        return;
    };
    let pairs = url
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    if pairs.is_empty() {
        return;
    }
    let mut changed = false;
    let next = pairs
        .iter()
        .enumerate()
        .map(|(index, (key, value))| {
            if is_sensitive_key(key) && !value.is_empty() && !has_variable_token(value) {
                changed = true;
                push_redaction(
                    resource_kind,
                    resource_export_id,
                    &format!("{path}.query[{index}]"),
                    "The URL parameter name looks like a credential field.",
                    redactions,
                );
                (key.as_str(), "")
            } else {
                (key.as_str(), value.as_str())
            }
        })
        .collect::<Vec<_>>();
    if changed {
        url.query_pairs_mut().clear().extend_pairs(next);
        *source = url.to_string();
    }
}

fn redact_string(
    value: &mut String,
    resource_kind: &str,
    resource_export_id: &str,
    path: &str,
    reason: &str,
    redactions: &mut Vec<WorkspaceRedaction>,
) {
    if value.is_empty() || has_variable_token(value) {
        return;
    }
    value.clear();
    push_redaction(resource_kind, resource_export_id, path, reason, redactions);
}

fn push_redaction(
    resource_kind: &str,
    resource_export_id: &str,
    path: &str,
    reason: &str,
    redactions: &mut Vec<WorkspaceRedaction>,
) {
    if redactions.iter().any(|item| {
        item.resource_kind == resource_kind
            && item.resource_export_id == resource_export_id
            && item.path == path
    }) {
        return;
    }
    redactions.push(WorkspaceRedaction {
        resource_kind: resource_kind.to_string(),
        resource_export_id: resource_export_id.to_string(),
        path: path.to_string(),
        reason: reason.to_string(),
    });
}

fn has_variable_token(value: &str) -> bool {
    value.contains(VARIABLE_TOKEN_START) && value.contains("}}")
}

fn is_sensitive_key(value: &str) -> bool {
    let normalized = value
        .trim()
        .to_ascii_lowercase()
        .replace(['_', '-', ' '], "");
    if matches!(
        normalized.as_str(),
        "apikeyname" | "apikeyin" | "oauth2tokenurl" | "tokenurl" | "clientid"
    ) {
        return false;
    }
    !normalized.is_empty()
        && (matches!(
            normalized.as_str(),
            "authorization"
                | "proxyauthorization"
                | "cookie"
                | "setcookie"
                | "apikey"
                | "xapikey"
                | "clientsecret"
                | "password"
                | "passwd"
        ) || normalized.contains("accesstoken")
            || normalized.contains("apikey")
            || normalized.contains("secret")
            || normalized.contains("token")
            || normalized.contains("password")
            || normalized.contains("passwd"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        realtime::{RawWebSocketComposer, RealtimeMessageDraft},
        requests::{KeyValueRow, RequestAuth, RequestBody},
    };

    #[test]
    fn request_redaction_clears_literals_but_keeps_variables() {
        let mut request = SendRequestPayload {
            name: "Demo".into(),
            method: "POST".into(),
            url: "https://example.test/?token=literal&safe=yes".into(),
            query_params: vec![KeyValueRow {
                id: "query".into(),
                key: "api_key".into(),
                value: "{{api_key}}".into(),
                enabled: true,
            }],
            headers: vec![KeyValueRow {
                id: "header".into(),
                key: "Authorization".into(),
                value: "Bearer literal".into(),
                enabled: true,
            }],
            body: RequestBody {
                mode: "json".into(),
                raw: r#"{"password":"literal","nested":{"value":"safe"}}"#.into(),
                form: Vec::new(),
                files: Vec::new(),
            },
            auth: RequestAuth {
                bearer_token: "{{token}}".into(),
                oauth2_client_secret: "literal".into(),
                ..RequestAuth::default()
            },
            pre_request_script: String::new(),
            test_script: String::new(),
        };
        let mut redactions = Vec::new();
        redact_request(&mut request, "httpRequest", "request-1", &mut redactions);

        assert!(request.url.contains("token=&safe=yes"));
        assert!(request.headers[0].value.is_empty());
        assert_eq!(request.query_params[0].value, "{{api_key}}");
        assert_eq!(request.auth.bearer_token, "{{token}}");
        assert!(request.auth.oauth2_client_secret.is_empty());
        assert!(request.body.raw.contains(r#""password": """#));
        assert_eq!(redactions.len(), 4);
    }

    #[test]
    fn realtime_json_message_redacts_credential_properties() {
        let mut message = RealtimeMessageDraft::Websocket {
            name: "authenticate".into(),
            composer: RawWebSocketComposer {
                mode: RawMessageMode::Json,
                content: r#"{"token":"literal","safe":"visible"}"#.into(),
                binary: None,
            },
        };
        let mut redactions = Vec::new();
        redact_realtime_message(
            &mut message,
            "realtimeMessage",
            "message-1",
            &mut redactions,
        )
        .expect("redact message");
        let RealtimeMessageDraft::Websocket { composer, .. } = message else {
            panic!("expected WebSocket message");
        };
        assert!(composer.content.contains(r#""token": """#));
        assert!(composer.content.contains(r#""safe": "visible""#));
        assert_eq!(redactions.len(), 1);
    }
}
