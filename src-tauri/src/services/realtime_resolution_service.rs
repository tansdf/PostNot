use crate::{
    domain::{
        environments::EnvironmentDetail,
        realtime::{
            BinaryPayloadSource, RawWebSocketComposer, RealtimeRequestCommon, RealtimeRequestDraft,
            SocketIoComposer,
        },
        requests::{KeyValueRow, RequestAuth},
    },
    services::environments_service,
};

#[derive(Debug, Clone)]
pub struct ResolvedRealtimeRequest {
    pub request: RealtimeRequestDraft,
    pub secret_values: Vec<String>,
}

pub fn resolve_request(
    request: &RealtimeRequestDraft,
    active_environment: Option<&EnvironmentDetail>,
) -> ResolvedRealtimeRequest {
    let request = match request {
        RealtimeRequestDraft::Websocket {
            common,
            subprotocols,
            composer,
        } => RealtimeRequestDraft::Websocket {
            common: resolve_common(common, active_environment),
            subprotocols: subprotocols
                .iter()
                .map(|value| resolve(value, active_environment))
                .collect(),
            composer: resolve_raw_composer(composer, active_environment),
        },
        RealtimeRequestDraft::Socketio {
            common,
            path,
            namespace,
            auth_payload,
            transport,
            composer,
        } => RealtimeRequestDraft::Socketio {
            common: resolve_common(common, active_environment),
            path: resolve(path, active_environment),
            namespace: resolve(namespace, active_environment),
            auth_payload: resolve_json(auth_payload, active_environment),
            transport: *transport,
            composer: resolve_socketio_composer(composer, active_environment),
        },
    };

    ResolvedRealtimeRequest {
        request,
        secret_values: environments_service::active_environment_secret_values(active_environment),
    }
}

pub fn resolve_raw_composer(
    composer: &RawWebSocketComposer,
    active_environment: Option<&EnvironmentDetail>,
) -> RawWebSocketComposer {
    resolve_raw_composer_with_usage(composer, active_environment).0
}

pub fn resolve_raw_composer_with_usage(
    composer: &RawWebSocketComposer,
    active_environment: Option<&EnvironmentDetail>,
) -> (RawWebSocketComposer, bool) {
    let (content, content_used_secret) =
        environments_service::resolve_realtime_template(&composer.content, active_environment);
    let (binary, binary_used_secret) = composer.binary.as_ref().map_or((None, false), |source| {
        let (source, used_secret) = resolve_binary_source_with_usage(source, active_environment);
        (Some(source), used_secret)
    });
    (
        RawWebSocketComposer {
            mode: composer.mode.clone(),
            content,
            binary,
        },
        content_used_secret || binary_used_secret,
    )
}

pub fn resolve_socketio_composer(
    composer: &SocketIoComposer,
    active_environment: Option<&EnvironmentDetail>,
) -> SocketIoComposer {
    resolve_socketio_composer_with_usage(composer, active_environment).0
}

pub fn resolve_socketio_composer_with_usage(
    composer: &SocketIoComposer,
    active_environment: Option<&EnvironmentDetail>,
) -> (SocketIoComposer, bool) {
    let (event, event_used_secret) =
        environments_service::resolve_realtime_template(&composer.event, active_environment);
    let (binary, binary_used_secret) = composer.binary.as_ref().map_or((None, false), |source| {
        let (source, used_secret) = resolve_binary_source_with_usage(source, active_environment);
        (Some(source), used_secret)
    });
    (
        SocketIoComposer {
            event,
            arguments: resolve_json(&composer.arguments, active_environment),
            binary,
            wait_for_ack: composer.wait_for_ack,
            ack_timeout_ms: composer.ack_timeout_ms,
        },
        event_used_secret || binary_used_secret,
    )
}

pub fn sanitize_error(message: &str, secret_values: &[String]) -> String {
    let mut values = secret_values
        .iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    values.sort_by_key(|value| std::cmp::Reverse(value.len()));

    values
        .into_iter()
        .fold(message.to_string(), |safe, secret| {
            safe.replace(secret, "***")
        })
}

fn resolve_common(
    common: &RealtimeRequestCommon,
    active_environment: Option<&EnvironmentDetail>,
) -> RealtimeRequestCommon {
    RealtimeRequestCommon {
        name: resolve(&common.name, active_environment),
        url: resolve(&common.url, active_environment),
        query_params: common
            .query_params
            .iter()
            .map(|row| resolve_row(row, active_environment))
            .collect(),
        headers: common
            .headers
            .iter()
            .map(|row| resolve_row(row, active_environment))
            .collect(),
        auth: resolve_auth(&common.auth, active_environment),
        reconnect: common.reconnect.clone(),
    }
}

fn resolve_row(row: &KeyValueRow, active_environment: Option<&EnvironmentDetail>) -> KeyValueRow {
    KeyValueRow {
        id: row.id.clone(),
        key: resolve(&row.key, active_environment),
        value: resolve(&row.value, active_environment),
        enabled: row.enabled,
    }
}

fn resolve_auth(auth: &RequestAuth, active_environment: Option<&EnvironmentDetail>) -> RequestAuth {
    RequestAuth {
        auth_type: auth.auth_type.clone(),
        basic_username: resolve(&auth.basic_username, active_environment),
        basic_password: resolve(&auth.basic_password, active_environment),
        bearer_token: resolve(&auth.bearer_token, active_environment),
        api_key_name: resolve(&auth.api_key_name, active_environment),
        api_key_value: resolve(&auth.api_key_value, active_environment),
        api_key_in: auth.api_key_in.clone(),
        oauth2_access_token: resolve(&auth.oauth2_access_token, active_environment),
        oauth2_token_url: resolve(&auth.oauth2_token_url, active_environment),
        oauth2_client_id: resolve(&auth.oauth2_client_id, active_environment),
        oauth2_client_secret: resolve(&auth.oauth2_client_secret, active_environment),
        oauth2_scope: resolve(&auth.oauth2_scope, active_environment),
    }
}

fn resolve_binary_source_with_usage(
    source: &BinaryPayloadSource,
    active_environment: Option<&EnvironmentDetail>,
) -> (BinaryPayloadSource, bool) {
    match source {
        BinaryPayloadSource::File { path } => {
            let (path, used_secret) =
                environments_service::resolve_realtime_template(path, active_environment);
            (BinaryPayloadSource::File { path }, used_secret)
        }
        BinaryPayloadSource::Hex { value } => {
            let (value, used_secret) =
                environments_service::resolve_realtime_template(value, active_environment);
            (BinaryPayloadSource::Hex { value }, used_secret)
        }
        BinaryPayloadSource::Base64 { value } => {
            let (value, used_secret) =
                environments_service::resolve_realtime_template(value, active_environment);
            (BinaryPayloadSource::Base64 { value }, used_secret)
        }
    }
}

fn resolve_json(
    value: &serde_json::Value,
    active_environment: Option<&EnvironmentDetail>,
) -> serde_json::Value {
    match value {
        serde_json::Value::String(value) => {
            serde_json::Value::String(resolve(value, active_environment))
        }
        serde_json::Value::Array(values) => serde_json::Value::Array(
            values
                .iter()
                .map(|value| resolve_json(value, active_environment))
                .collect(),
        ),
        serde_json::Value::Object(values) => serde_json::Value::Object(
            values
                .iter()
                .map(|(key, value)| {
                    (
                        resolve(key, active_environment),
                        resolve_json(value, active_environment),
                    )
                })
                .collect(),
        ),
        value => value.clone(),
    }
}

fn resolve(input: &str, active_environment: Option<&EnvironmentDetail>) -> String {
    environments_service::resolve_realtime_template(input, active_environment).0
}

#[cfg(test)]
mod tests {
    use crate::domain::environments::EnvironmentVariable;

    use super::*;

    #[test]
    fn resolves_nested_json_and_masks_secret_errors() {
        let environment = EnvironmentDetail {
            id: "env".to_string(),
            name: "Test".to_string(),
            is_active: true,
            variables: vec![EnvironmentVariable {
                id: "token".to_string(),
                key: "token".to_string(),
                value: "secret-value".to_string(),
                enabled: true,
                is_secret: true,
            }],
            updated_at: String::new(),
        };
        let value = resolve_json(
            &serde_json::json!({"nested": ["{{token}}"]}),
            Some(&environment),
        );
        assert_eq!(value, serde_json::json!({"nested": ["secret-value"]}));
        assert_eq!(
            sanitize_error("failed for secret-value", &["secret-value".to_string()]),
            "failed for ***"
        );

        let composer = RawWebSocketComposer {
            mode: crate::domain::realtime::RawMessageMode::Binary,
            content: String::new(),
            binary: Some(BinaryPayloadSource::Base64 {
                value: "{{token}}".to_string(),
            }),
        };
        let (resolved, used_secret) =
            resolve_raw_composer_with_usage(&composer, Some(&environment));
        assert!(used_secret);
        assert!(matches!(
            resolved.binary,
            Some(BinaryPayloadSource::Base64 { ref value }) if value == "secret-value"
        ));
    }
}
