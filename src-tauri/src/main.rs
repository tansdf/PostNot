#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let mut args = std::env::args().skip(1);
    if matches!(args.next().as_deref(), Some("--mcp")) {
        let mut options = postnot_lib::mcp::McpOptions::default();
        while let Some(argument) = args.next() {
            if argument == "--data-dir" {
                options.data_dir = args.next().map(std::path::PathBuf::from);
            }
        }
        let runtime = tokio::runtime::Runtime::new().expect("create MCP runtime");
        if let Err(error) = runtime.block_on(postnot_lib::mcp::run(options)) {
            eprintln!("PostNot MCP failed: {error}");
            std::process::exit(1);
        }
        return;
    }

    if let Err(error) = postnot_lib::run() {
        postnot_lib::report_startup_failure(&error);
    }
}
