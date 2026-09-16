//! Credential-free MCP endpoint Goose retains with conversation metadata.
//!
//! The optional identity report happens in `main.rs`; this module always
//! remains a valid empty MCP server, including on manual resumes outside
//! Farhelm where no launch markers or session credentials exist.

use serde_json::{Value, json};
use std::io::{BufRead, Read, Write};

const MAX_FRAME_BYTES: u64 = 64 * 1024;

/// Serve bounded newline-delimited MCP messages until EOF or a broken peer.
pub fn serve(mut input: impl BufRead, mut output: impl Write, instructions: Option<&str>) {
    loop {
        let mut line = Vec::new();
        let read = input
            .by_ref()
            .take(MAX_FRAME_BYTES + 1)
            .read_until(b'\n', &mut line);
        if !matches!(read, Ok(n) if n > 0 && n as u64 <= MAX_FRAME_BYTES) {
            return;
        }
        let response = match serde_json::from_slice::<Value>(&line) {
            Ok(request) => reply(&request, instructions),
            Err(_) => Some(error(Value::Null, -32700, "parse error")),
        };
        if let Some(response) = response
            && write_frame(&mut output, response).is_err()
        {
            return;
        }
    }
}

/// Answer only lifecycle and discovery requests; no model-callable API exists.
fn reply(request: &Value, instructions: Option<&str>) -> Option<Value> {
    if request.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
        || !request.get("method").is_some_and(Value::is_string)
    {
        return Some(error(Value::Null, -32600, "invalid request"));
    }
    let id = request.get("id")?;
    if !(id.is_string() || id.is_number() || id.is_null()) {
        return Some(error(Value::Null, -32600, "invalid request"));
    }
    let result = match request["method"].as_str()? {
        "initialize" => {
            let version = request["params"]["protocolVersion"]
                .as_str()
                .filter(|version| matches!(*version, "2024-11-05" | "2025-03-26" | "2025-06-18"))
                .unwrap_or("2025-06-18");
            let mut result = json!({
                "protocolVersion": version,
                "capabilities": {},
                "serverInfo": {"name": "farhelm-conversation", "version": "1"}
            });
            if let Some(instructions) = instructions {
                result["instructions"] = Value::String(instructions.to_owned());
            }
            result
        }
        "ping" => json!({}),
        "tools/list" => json!({"tools": []}),
        "resources/list" => json!({"resources": []}),
        "resources/templates/list" => json!({"resourceTemplates": []}),
        "prompts/list" => json!({"prompts": []}),
        _ => return Some(error(id.clone(), -32601, "method not found")),
    };
    Some(json!({"jsonrpc": "2.0", "id": id, "result": result}))
}

fn error(id: Value, code: i32, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

fn write_frame(output: &mut impl Write, frame: Value) -> std::io::Result<()> {
    let mut bytes = serde_json::to_vec(&frame)?;
    if bytes.len() > MAX_FRAME_BYTES as usize {
        bytes = serde_json::to_vec(&error(Value::Null, -32600, "response exceeds bound"))?;
    }
    bytes.push(b'\n');
    output.write_all(&bytes)?;
    output.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// Manual resumes initialize normally, while only an enabled launch gets
    /// the additive model-visible pointer.
    #[test]
    fn initialization_keeps_instructions_launch_scoped() {
        let request = json!({"jsonrpc":"2.0", "id":7, "method":"initialize",
            "params":{"protocolVersion":"2025-03-26"}});
        let disabled = reply(&request, None).unwrap();
        assert_eq!(disabled["result"]["capabilities"], json!({}));
        assert!(disabled["result"].get("instructions").is_none());
        assert_eq!(
            reply(&request, Some("pointer")).unwrap()["result"]["instructions"],
            "pointer"
        );
    }

    /// Notifications stay silent and malformed input cannot desynchronize the
    /// next complete request.
    #[test]
    fn frames_remain_correlated_after_invalid_input() {
        let input = b"bad json\n{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n{\"jsonrpc\":\"2.0\",\"id\":42,\"method\":\"ping\"}\n";
        let mut output = Vec::new();
        serve(Cursor::new(input), &mut output, None);
        let replies: Vec<Value> = output
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_slice(line).unwrap())
            .collect();
        assert_eq!(replies.len(), 2);
        assert_eq!(replies[0]["error"]["code"], -32700);
        assert_eq!(replies[1]["id"], 42);
    }
}
