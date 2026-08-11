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
        eprintln!();
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
        _ => eprintln!("Unknown command: {command}"),
    }
}
