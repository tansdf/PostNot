use std::{
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
};

#[test]
fn headless_mode_negotiates_and_lists_authoring_tools() {
    let data_dir = std::env::temp_dir().join(format!("postnot-mcp-test-{}", uuid::Uuid::new_v4()));
    let mut child = Command::new(env!("CARGO_BIN_EXE_postnot"))
        .args(["--mcp", "--data-dir"])
        .arg(&data_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start PostNot MCP");
    let mut stdin = child.stdin.take().expect("MCP stdin");
    let mut stdout = BufReader::new(child.stdout.take().expect("MCP stdout"));

    writeln!(stdin, r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"2025-06-18","capabilities":{{}},"clientInfo":{{"name":"integration-test","version":"1"}}}}}}"#)
        .expect("write initialize");
    stdin.flush().expect("flush initialize");
    let mut line = String::new();
    stdout.read_line(&mut line).expect("read initialize");
    let initialized: serde_json::Value = serde_json::from_str(&line).expect("initialize JSON");
    assert_eq!(initialized["result"]["serverInfo"]["name"], "postnot");

    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","method":"notifications/initialized"}}"#
    )
    .expect("write initialized");
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{{}}}}"#
    )
    .expect("write tool list");
    stdin.flush().expect("flush tool list");
    line.clear();
    stdout.read_line(&mut line).expect("read tool list");
    let tools: serde_json::Value = serde_json::from_str(&line).expect("tool-list JSON");
    let names: Vec<&str> = tools["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    assert_eq!(names.len(), 21);
    assert!(names.contains(&"create_requests"));
    assert!(names.contains(&"preview_saved_request"));
    for realtime_tool in [
        "list_realtime_connections",
        "get_realtime_connection",
        "create_realtime_connection",
        "update_realtime_connection",
        "delete_realtime_connection",
        "list_realtime_messages",
        "get_realtime_message",
        "create_realtime_message",
        "update_realtime_message",
        "delete_realtime_message",
    ] {
        assert!(names.contains(&realtime_tool));
    }
    assert!(!names.iter().any(|name| name.contains("send")));

    child.kill().expect("stop MCP");
    child.wait().expect("wait for MCP");
    let _ = std::fs::remove_dir_all(data_dir);
}
