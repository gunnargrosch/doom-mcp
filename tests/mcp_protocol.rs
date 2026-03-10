use std::process::{Command, Stdio};
use std::io::{Write, BufRead, BufReader};

// Test that the MCP server starts and responds to initialize
#[test]
fn test_mcp_initialize() {
    // Start the server
    let mut child = Command::new(env!("CARGO_BIN_EXE_doom-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to start doom-mcp");

    let stdin = child.stdin.as_mut().unwrap();
    let stdout = BufReader::new(child.stdout.take().unwrap());

    // Send initialize
    writeln!(stdin, r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"2025-11-25","capabilities":{{}},"clientInfo":{{"name":"test","version":"1.0"}}}}}}"#).unwrap();

    // Read response
    let mut response = String::new();
    let mut reader = stdout;
    reader.read_line(&mut response).unwrap();

    assert!(response.contains("\"jsonrpc\":\"2.0\""));
    assert!(response.contains("doom-mcp"));
    assert!(response.contains("protocolVersion"));

    child.kill().ok();
}

// Test that tools/list returns all expected tools
#[test]
fn test_tools_list() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_doom-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to start doom-mcp");

    let stdin = child.stdin.as_mut().unwrap();
    let stdout = BufReader::new(child.stdout.take().unwrap());

    // Initialize first
    writeln!(stdin, r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"2025-11-25","capabilities":{{}},"clientInfo":{{"name":"test","version":"1.0"}}}}}}"#).unwrap();
    writeln!(stdin, r#"{{"jsonrpc":"2.0","method":"notifications/initialized"}}"#).unwrap();
    // Request tools
    writeln!(stdin, r#"{{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{{}}}}"#).unwrap();

    let mut reader = stdout;
    let mut lines = Vec::new();
    for _ in 0..2 {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        lines.push(line);
    }

    let tools_response = &lines[1]; // second line is tools/list response
    assert!(tools_response.contains("doom_start"));
    assert!(tools_response.contains("doom_action"));
    assert!(tools_response.contains("doom_screenshot"));

    child.kill().ok();
}
