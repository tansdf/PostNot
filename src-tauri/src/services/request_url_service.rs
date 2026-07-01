pub fn normalize_request_url(value: &str) -> String {
    let trimmed = value.trim();

    if is_bare_localhost_url(trimmed) {
        format!("http://{trimmed}")
    } else {
        trimmed.to_string()
    }
}

fn is_bare_localhost_url(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    let Some(rest) = lower.strip_prefix("localhost") else {
        return false;
    };

    rest.is_empty()
        || rest.starts_with(':')
        || rest.starts_with('/')
        || rest.starts_with('?')
        || rest.starts_with('#')
}

#[cfg(test)]
mod tests {
    use super::normalize_request_url;

    #[test]
    fn prefixes_bare_localhost_with_http() {
        assert_eq!(normalize_request_url("localhost"), "http://localhost");
        assert_eq!(
            normalize_request_url("localhost:3000/api?ok=true"),
            "http://localhost:3000/api?ok=true"
        );
        assert_eq!(
            normalize_request_url("LOCALHOST/status"),
            "http://LOCALHOST/status"
        );
    }

    #[test]
    fn leaves_existing_schemes_and_other_hosts_alone() {
        assert_eq!(
            normalize_request_url("https://localhost:3000"),
            "https://localhost:3000"
        );
        assert_eq!(normalize_request_url("example.com"), "example.com");
        assert_eq!(
            normalize_request_url("localhost.example.com"),
            "localhost.example.com"
        );
    }
}
