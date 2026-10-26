//! Local, side-effect-free MCP server for the opt-in Claude permission audit.
use serde_json::{Value, json};
use std::{
    error::Error,
    io::{BufRead, Write},
    path::Path,
};

pub fn run(args: &[String]) -> Result<(), Box<dyn Error>> {
    let control = Path::new(args.get(2).ok_or("missing control directory")?);
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(control.join("mcp.lock"))?;
    fs2::FileExt::lock_exclusive(&lock)?;
    for line in std::io::stdin().lock().lines() {
        let request: Value = serde_json::from_str(&line?)?;
        if request["id"] == "audit-question" {
            std::fs::write(control.join("question-response.json"), request.to_string())?;
        }
        let result = match request["method"].as_str() {
            Some("initialize") => json!({"protocolVersion":request["params"]["protocolVersion"],
                "capabilities":{"tools":{}}, "serverInfo":{"name":"codeconvoy-audit","version":"1"}}),
            Some("tools/list") => json!({"tools":[
                {"name":"confirm", "description":"Return a test marker after a person confirms.",
                    "inputSchema":{"type":"object","properties":{}},
                    "_meta":{"anthropic/requiresUserInteraction":true}},
                {"name":"elicit", "description":"Ask a test question; no files or external services are accessed.",
                    "inputSchema":{"type":"object","properties":{}}, "annotations":{"readOnlyHint":true}}
            ]}),
            Some("tools/call") if request["params"]["name"] == "confirm" => {
                std::fs::write(control.join("confirm.called"), "unexpected")?;
                json!({"content":[{"type":"text","text":"CONFIRMED"}]})
            }
            Some("tools/call") => {
                // Leave the call pending after the client cancels the form,
                // exercising the CLI's MCP timeout and blocked cancellation.
                send(
                    json!({"jsonrpc":"2.0","id":"audit-question","method":"elicitation/create",
                    "params":{"mode":"form","message":"Confirm this audit probe?",
                        "requestedSchema":{"type":"object","properties":{"confirm":{"type":"boolean"}},"required":["confirm"]}}}),
                )?;
                std::fs::write(control.join("elicitation.ready"), "ready")?;
                continue;
            }
            Some("ping") => json!({}),
            _ => continue,
        };
        send(json!({"jsonrpc":"2.0","id":request["id"],"result":result}))?;
    }
    Ok(())
}

fn send(message: Value) -> Result<(), Box<dyn Error>> {
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer(&mut stdout, &message)?;
    writeln!(stdout)?;
    stdout.flush()?;
    Ok(())
}
