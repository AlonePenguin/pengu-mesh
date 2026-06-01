use anyhow::{Context, Result, anyhow};
use clap::Parser;
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};

use pengu_mesh_core::StageOneRuntime;
use pengu_mesh_mcp::{ToolCallRequest, core_tools, execute_tool, mcp_tools_list};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    json: bool,
    #[arg(long)]
    once_tool: Option<String>,
    #[arg(long, default_value = "{}")]
    once_input: String,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let runtime = StageOneRuntime::new_with_entrypoint("pengu-mesh-mcp")?;
    if let Some(tool) = args.once_tool.as_deref() {
        let input: Value =
            serde_json::from_str(&args.once_input).context("parse once input JSON")?;
        let payload = execute_tool(
            &runtime,
            ToolCallRequest {
                tool: tool.to_string(),
                args: input,
            },
        )?;
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }

    if args.json {
        let payload =
            pengu_mesh_shared::OperationOutcome::success("mcp tool catalog", core_tools());
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }

    run_stdio_server(&runtime)
}

fn run_stdio_server(runtime: &StageOneRuntime) -> Result<()> {
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let stdout = io::stdout();
    let mut writer = stdout.lock();
    loop {
        let Some(message) = read_mcp_message(&mut reader)? else {
            break;
        };
        let request: Value = serde_json::from_slice(&message).context("parse MCP request")?;
        let response = handle_request(runtime, request)?;
        if response != json!({}) {
            write_mcp_message(&mut writer, &response)?;
        }
    }
    Ok(())
}

fn handle_request(runtime: &StageOneRuntime, request: Value) -> Result<Value> {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let method = request["method"]
        .as_str()
        .ok_or_else(|| anyhow!("request missing method"))?;
    let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
    let result = match method {
        "initialize" => json!({
            "protocolVersion": "2024-11-05",
            "serverInfo": {
                "name": "pengu-mesh-mcp",
                "version": env!("CARGO_PKG_VERSION"),
            },
            "capabilities": {
                "tools": {
                    "listChanged": false,
                }
            }
        }),
        "notifications/initialized" => return Ok(json!({})),
        "tools/list" => mcp_tools_list(),
        "tools/call" => {
            let tool_name = params["name"]
                .as_str()
                .ok_or_else(|| anyhow!("tools/call missing name"))?;
            let arguments = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let payload = execute_tool(
                runtime,
                ToolCallRequest {
                    tool: tool_name.to_string(),
                    args: arguments,
                },
            )?;
            json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string_pretty(&payload)?,
                }],
                "structuredContent": serde_json::to_value(&payload)?,
                "isError": !payload.ok,
            })
        }
        "ping" => json!({}),
        _ => {
            return Ok(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {
                    "code": -32601,
                    "message": format!("unsupported method {method}"),
                }
            }));
        }
    };
    Ok(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    }))
}

fn read_mcp_message(reader: &mut impl BufRead) -> Result<Option<Vec<u8>>> {
    let mut content_length = None;
    loop {
        let mut line = String::new();
        let read = reader.read_line(&mut line).context("read MCP header")?;
        if read == 0 {
            return Ok(None);
        }
        if line == "\r\n" {
            break;
        }
        let lower = line.to_ascii_lowercase();
        if let Some(value) = lower.strip_prefix("content-length:") {
            content_length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .context("parse content length")?,
            );
        }
    }
    let length = content_length.ok_or_else(|| anyhow!("missing Content-Length header"))?;
    let mut body = vec![0_u8; length];
    reader.read_exact(&mut body).context("read MCP body")?;
    Ok(Some(body))
}

fn write_mcp_message(writer: &mut impl Write, payload: &Value) -> Result<()> {
    let body = serde_json::to_vec(payload).context("serialize MCP response")?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len()).context("write MCP header")?;
    writer.write_all(&body).context("write MCP body")?;
    writer.flush().context("flush MCP response")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::handle_request;
    use pengu_mesh_core::StageOneRuntime;
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn handles_initialize() {
        let tempdir = tempdir().expect("tempdir");
        let runtime =
            StageOneRuntime::new_in_root(tempdir.path().to_path_buf(), "pengu-mesh-mcp-test")
                .expect("runtime");
        let response = handle_request(
            &runtime,
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {},
            }),
        )
        .expect("response");
        assert_eq!(response["result"]["serverInfo"]["name"], "pengu-mesh-mcp");
    }
}
