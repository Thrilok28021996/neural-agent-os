fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Neural Agent OS — local personal assistant CLI");
        eprintln!();
        eprintln!("Usage: neural-cli <command> [options]");
        eprintln!();
        eprintln!("Commands:");
        eprintln!("  health                     Check if the local API is running");
        eprintln!("  workspaces                 List all workspaces");
        eprintln!("  search <workspace> <q>     Search a workspace");
        eprintln!("  tasks <workspace>          List tasks in a workspace");
        eprintln!("  meetings <workspace>       List meetings in a workspace");
        eprintln!("  notes <workspace>          List notes in a workspace");
        eprintln!("  transcripts <workspace> <q> Search meeting transcripts");
        eprintln!("  ask <workspace> <prompt>   Ask the assistant");
        eprintln!("  tool <name> <workspace> <json> Call a permission-gated tool (API-key auth)");
        eprintln!("        e.g. neural-cli tool list_tasks personal");
        eprintln!();
        eprintln!("Write/tool commands authenticate with the NEURAL_API_KEY environment variable.");
        eprintln!("The desktop application must be running for CLI commands to work.");
        std::process::exit(1);
    }

    let base = "http://127.0.0.1:8787";
    let command = &args[1];

    match command.as_str() {
        "health" => {
            let result = reqwest::blocking::get(format!("{base}/health"));
            match result {
                Ok(response) => println!("{}", response.text().unwrap_or_default()),
                Err(_) => { eprintln!("Neural Agent OS is not running. Start the desktop application first."); std::process::exit(1); }
            }
        }
        "workspaces" => {
            let result = reqwest::blocking::get(format!("{base}/v1/workspaces"));
            match result {
                Ok(response) => println!("{}", response.text().unwrap_or_default()),
                Err(error) => eprintln!("Error: {error}"),
            }
        }
        "search" => {
            let workspace = args.get(2).cloned().unwrap_or_else(|| "personal".into());
            let query = args.get(3).cloned().unwrap_or_default();
            if query.is_empty() { eprintln!("Provide a search query"); std::process::exit(1); }
            let url = format!("{base}/v1/search?workspace_id={workspace}&q={query}");
            let result = reqwest::blocking::get(&url);
            match result {
                Ok(response) => println!("{}", response.text().unwrap_or_default()),
                Err(error) => eprintln!("Error: {error}"),
            }
        }
        "tasks" => {
            let workspace = args.get(2).cloned().unwrap_or_else(|| "personal".into());
            let result = reqwest::blocking::get(format!("{base}/v1/tasks?workspace_id={workspace}"));
            match result {
                Ok(response) => println!("{}", response.text().unwrap_or_default()),
                Err(error) => eprintln!("Error: {error}"),
            }
        }
        "meetings" => {
            let workspace = args.get(2).cloned().unwrap_or_else(|| "personal".into());
            let result = reqwest::blocking::get(format!("{base}/v1/meetings?workspace_id={workspace}"));
            match result {
                Ok(response) => println!("{}", response.text().unwrap_or_default()),
                Err(error) => eprintln!("Error: {error}"),
            }
        }
        "notes" => {
            let workspace = args.get(2).cloned().unwrap_or_else(|| "personal".into());
            let result = reqwest::blocking::get(format!("{base}/v1/notes?workspace_id={workspace}"));
            match result { Ok(response) => println!("{}", response.text().unwrap_or_default()), Err(error) => eprintln!("Error: {error}") }
        }
        "transcripts" => {
            let workspace = args.get(2).cloned().unwrap_or_else(|| "personal".into());
            let query = args.get(3).cloned().unwrap_or_default();
            if query.is_empty() { eprintln!("Provide a transcript search query"); std::process::exit(1); }
            let url = format!("{base}/v1/transcripts?workspace_id={workspace}&q={query}");
            let result = reqwest::blocking::get(&url);
            match result { Ok(response) => println!("{}", response.text().unwrap_or_default()), Err(error) => eprintln!("Error: {error}") }
        }
        "ask" => {
            let workspace = args.get(2).cloned().unwrap_or_else(|| "personal".into());
            let query: String = args.iter().skip(3).cloned().collect::<Vec<_>>().join(" ");
            if query.is_empty() { eprintln!("Provide a prompt"); std::process::exit(1); }
            let body = serde_json::json!({ "workspace_id": workspace, "prompt": query });
            let result = reqwest::blocking::Client::new().post(format!("{base}/v1/ask")).json(&body).send();
            match result {
                Ok(response) => {
                    let parsed: serde_json::Value = response.json().unwrap_or_default();
                    if let Some(content) = parsed.get("result").and_then(|r| r.get("content")).and_then(|c| c.get(0)).and_then(|c| c.get("text")).and_then(|t| t.as_str()) {
                        println!("{content}");
                    } else if let Some(error) = parsed.get("error") {
                        eprintln!("Error: {}", error.get("message").and_then(|m| m.as_str()).unwrap_or("unknown"));
                    } else {
                        println!("{}", parsed.to_string());
                    }
                }
                Err(error) => eprintln!("Error: {error}"),
            }
        }
        "tool" => {
            // Permission-gated tool call through /v1/tools/call (#9): the CLI
            // has no special powers — it is just another authenticated client.
            let name = args.get(2).cloned().unwrap_or_default();
            let workspace = args.get(3).cloned().unwrap_or_default();
            let json_args = args.get(4).cloned().unwrap_or_else(|| "{}".into());
            if name.is_empty() || workspace.is_empty() {
                eprintln!("Usage: neural-cli tool <name> <workspace> <json-args>");
                std::process::exit(1);
            }
            let api_key = std::env::var("NEURAL_API_KEY").unwrap_or_default();
            if api_key.is_empty() {
                eprintln!("NEURAL_API_KEY is not set. Generate one in the app (Data → API keys) and export it.");
                std::process::exit(1);
            }
            let mut user_args: serde_json::Value = serde_json::from_str(&json_args).unwrap_or_else(|_| serde_json::json!({}));
            if let Some(obj) = user_args.as_object_mut() {
                obj.insert("workspace_id".into(), serde_json::Value::String(workspace.clone()));
                obj.insert("agent_id".into(), serde_json::Value::String("cli".into()));
            }
            let body = serde_json::json!({ "tool": name, "arguments": user_args });
            let client = reqwest::blocking::Client::new();
            let result = client
                .post(format!("{base}/v1/tools/call"))
                .bearer_auth(&api_key)
                .json(&body)
                .send();
            match result {
                Ok(response) => {
                    let status = response.status();
                    let text = response.text().unwrap_or_default();
                    if status.is_success() {
                        println!("{text}");
                    } else {
                        eprintln!("Tool call failed (HTTP {status}): {text}");
                        std::process::exit(1);
                    }
                }
                Err(error) => { eprintln!("Error: {error}"); std::process::exit(1); }
            }
        }
        _ => eprintln!("Unknown command: {command}"),
    }
}
