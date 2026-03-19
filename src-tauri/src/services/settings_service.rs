use crate::domain::settings::AppSettings;

pub fn default_settings() -> AppSettings {
    AppSettings {
        theme: "system".to_string(),
        request_timeout_ms: 30_000,
        follow_redirects: true,
        validate_tls: true,
        history_limit: 200,
    }
}
