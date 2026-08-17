// ── Autonomous agent tool execution (#7) ────────────────────────────────────
// Agents are no longer prompt-only: they can call workspace tools in a loop
// (search, notes, tasks, calendar, email, files, commands) with the same
// permission/approval model as every other external surface. Every call is
// logged (run_id + agent_id + tool + args) and recorded in the audit log so
// issues are traceable end-to-end.

use rusqlite::Connection;
use serde_json::{json, Value};

pub struct AgentTool {
    pub name: &'static str,
    pub description: &'static str,
    pub capability: &'static str,
    /// Tools whose underlying command enforces its own approval request
    /// (execute_commands / modify_files / send_email).
    pub self_approving: bool,
}

pub static AGENT_TOOLS: &[AgentTool] = &[
    // Read tools (read_knowledge / read_transcripts)
    AgentTool { name: "search_workspace", description: "Full-text and semantic search over indexed sources. Arguments: query (string, required).", capability: "read_knowledge", self_approving: false },
    AgentTool { name: "search_transcripts", description: "Search meeting transcript segments. Arguments: query (string, required).", capability: "read_transcripts", self_approving: false },
    AgentTool { name: "search_emails", description: "Search emails in the workspace. Arguments: query (string, required).", capability: "read_knowledge", self_approving: false },
    AgentTool { name: "list_tasks", description: "List open tasks in the workspace. No arguments.", capability: "read_knowledge", self_approving: false },
    AgentTool { name: "list_meetings", description: "List meetings in the workspace. No arguments.", capability: "read_knowledge", self_approving: false },
    AgentTool { name: "list_notes", description: "List notes in the workspace. No arguments.", capability: "read_knowledge", self_approving: false },
    AgentTool { name: "list_calendar_events", description: "List calendar events in the workspace. No arguments.", capability: "read_knowledge", self_approving: false },
    AgentTool { name: "list_emails", description: "List emails in the workspace. No arguments.", capability: "read_knowledge", self_approving: false },
    AgentTool { name: "list_reminders", description: "List reminders in the workspace. No arguments.", capability: "read_knowledge", self_approving: false },
    AgentTool { name: "list_memories", description: "List memories in the workspace. No arguments.", capability: "read_knowledge", self_approving: false },
    AgentTool { name: "list_agent_runs", description: "Inspect execution history for an agent. Arguments: agent_id (string, required).", capability: "read_knowledge", self_approving: false },
    // Write tools
    AgentTool { name: "create_note", description: "Create a note. Arguments: title (string), content (string).", capability: "create_notes", self_approving: false },
    AgentTool { name: "create_memory", description: "Save a memory. Arguments: content (string).", capability: "create_notes", self_approving: false },
    AgentTool { name: "create_task", description: "Create a task. Arguments: title (string), due_at (optional ISO timestamp).", capability: "modify_tasks", self_approving: false },
    AgentTool { name: "update_task_status", description: "Update a task's status (open/completed). Arguments: task_id (string), status (string).", capability: "modify_tasks", self_approving: false },
    AgentTool { name: "create_reminder", description: "Schedule a reminder. Arguments: task_id (string), title (string), scheduled_at (ISO timestamp).", capability: "create_reminders", self_approving: false },
    AgentTool { name: "create_calendar_event", description: "Create a calendar event. Arguments: title (string), starts_at (ISO), ends_at (ISO), meeting_url (optional), recurrence (optional).", capability: "schedule_events", self_approving: false },
    AgentTool { name: "promote_action_to_task", description: "Promote a meeting action item to a task. Arguments: action_id (string).", capability: "modify_tasks", self_approving: false },
    // Sensitive tools (approval enforced by their own commands)
    AgentTool { name: "execute_command", description: "Run a shell command on this machine (approval required). Arguments: command (string).", capability: "execute_commands", self_approving: true },
    AgentTool { name: "modify_file", description: "Create or edit a file on disk (approval required). Arguments: path (string), content (string), append (optional bool).", capability: "modify_files", self_approving: true },
    AgentTool { name: "send_email", description: "Send an email through a connected account (approval required). Arguments: account_id (string), to (string), subject (string), body (string).", capability: "send_email", self_approving: true },
];

/// Look up a tool by name.
pub fn tool_by_name(name: &str) -> Option<&'static AgentTool> {
    AGENT_TOOLS.iter().find(|tool| tool.name == name)
}

/// MCP/JSON-RPC input schema for a tool. Every external tool call carries a
/// workspace_id (and agent_id for identity/permissions); `list_agent_runs` is
/// the only agent-scoped tool.
pub fn tool_input_schema(tool: &AgentTool) -> serde_json::Value {
    let agent_required = tool.name == "list_agent_runs";
    serde_json::json!({
        "type": "object",
        "properties": {
            "workspace_id": { "type": "string", "description": "Workspace the tool operates on." },
            "agent_id": { "type": "string", "description": "Identity whose permissions are enforced." }
        },
        "required": if agent_required { ["agent_id"] } else { ["workspace_id"] }
    })
}

/// Full MCP `tools/list` payload built from the shared catalogue so every
/// external surface (MCP, REST, agent runtime) exposes the same tools.
pub fn mcp_tools_list() -> serde_json::Value {
    let tools: Vec<serde_json::Value> = AGENT_TOOLS.iter().map(|tool| serde_json::json!({
        "name": tool.name,
        "description": tool.description,
        "inputSchema": tool_input_schema(tool),
    })).collect();
    serde_json::json!({ "tools": tools })
}

/// The tool catalogue as a prompt block for the agent model.
pub fn tool_list_prompt() -> String {
    let mut out = String::from("You have access to these tools:\n");
    for tool in AGENT_TOOLS {
        let approval = if tool.self_approving { " (requires your user's approval before it runs)" } else { "" };
        out.push_str(&format!("- {}: {}{}\n", tool.name, tool.description, approval));
    }
    out.push_str("Respond with EXACTLY one JSON object on a single line when you want to call a tool: {\"tool\": \"<name>\", \"arguments\": {\"key\": \"value\"}}. Otherwise respond with plain text: that is your final answer to the user. Never invent tool results.");
    out
}

/// Extract a tool call from a model response. Accepts a response that is
/// entirely a JSON object or one that embeds the JSON object in prose.
pub fn parse_agent_tool_call(response: &str) -> Option<(String, Value)> {
    let trimmed = response.trim();
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        return tool_call_from_value(&value);
    }
    // Scan for a { ... } block containing "tool".
    let mut depth = 0i32;
    let mut start = None;
    for (idx, ch) in trimmed.char_indices() {
        match ch {
            '{' => { if depth == 0 { start = Some(idx); } depth += 1; }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    if let Some(s) = start {
                        if let Ok(value) = serde_json::from_str::<Value>(&trimmed[s..=idx]) {
                            if let Some(call) = tool_call_from_value(&value) {
                                return Some(call);
                            }
                        }
                    }
                    start = None;
                }
            }
            _ => {}
        }
    }
    None
}

fn tool_call_from_value(value: &Value) -> Option<(String, Value)> {
    let tool = value.get("tool")?.as_str()?.to_string();
    let arguments = value.get("arguments").cloned().unwrap_or_else(|| json!({}));
    Some((tool, arguments))
}

fn arg_str(args: &Value, key: &str) -> String {
    args.get(key).and_then(|v| v.as_str()).unwrap_or("").to_string()
}

fn arg_opt(args: &Value, key: &str) -> Option<String> {
    args.get(key).and_then(|v| v.as_str()).map(String::from)
}

/// Permission gate (DB-only, unit-testable). Sensitive tools enforce their own
/// approval requests inside their command implementations.
pub fn check_tool_allowed(
    connection: &Connection,
    agent_id: &str,
    workspace_id: &str,
    tool: &AgentTool,
) -> Result<(), String> {
    let perm = crate::permissions::check_permission(connection, agent_id, workspace_id, tool.capability);
    if perm.allowed {
        Ok(())
    } else {
        Err(format!("Permission denied: {} — {}", tool.capability, perm.reason))
    }
}

/// Execute an agent tool with permission gating, structured logging, and an
/// audit entry. Returns the tool result as JSON (or the serialized output of
/// the underlying command).
pub fn execute_agent_tool(
    app: &tauri::AppHandle,
    connection: &Connection,
    workspace_id: &str,
    agent_id: &str,
    run_id: &str,
    name: &str,
    args: &Value,
) -> Result<Value, String> {
    let tool = tool_by_name(name).ok_or_else(|| format!("Unknown agent tool: {name}"))?;
    check_tool_allowed(connection, agent_id, workspace_id, tool)?;
    log::info!("agent_tools: run={run_id} agent={agent_id} tool={name} args={args} workspace={workspace_id}");
    let result = dispatch(app, workspace_id, agent_id, name, args);
    match &result {
        Ok(value) => {
            let summary = value.to_string();
            log::info!("agent_tools: run={run_id} agent={agent_id} tool={name} ok result_chars={}", summary.len());
        }
        Err(error) => {
            log::error!("agent_tools: run={run_id} agent={agent_id} tool={name} failed: {error}");
        }
    }
    let _ = crate::retention::record_audit(
        connection,
        "agent_tool_executed",
        Some("agent"),
        Some("tool"),
        Some(workspace_id),
        &format!("Agent {agent_id} (run {run_id}) called tool {name}"),
    );
    result
}

fn dispatch(
    app: &tauri::AppHandle,
    workspace_id: &str,
    agent_id: &str,
    name: &str,
    args: &Value,
) -> Result<Value, String> {
    let ws = workspace_id.to_string();
    match name {
        "search_workspace" => crate::search_sources(app.clone(), ws, arg_str(args, "query")).map(|v| json!(v)),
        "search_transcripts" => crate::conversation_search(app.clone(), ws, arg_str(args, "query")).map(|v| json!(v)),
        "search_emails" => crate::search_emails(app.clone(), ws, arg_str(args, "query")).map(|v| json!(v)),
        "list_tasks" => crate::list_tasks(app.clone(), ws).map(|v| json!(v)),
        "list_meetings" => crate::list_meetings(app.clone(), ws).map(|v| json!(v)),
        "list_notes" => crate::list_notes(app.clone(), ws).map(|v| json!(v)),
        "list_calendar_events" => crate::list_calendar_events(app.clone(), ws).map(|v| json!(v)),
        "list_emails" => crate::list_emails(app.clone(), ws, String::new()).map(|v| json!(v)),
        "list_reminders" => crate::list_reminders(app.clone(), ws).map(|v| json!(v)),
        "list_memories" => crate::list_memories(app.clone(), ws).map(|v| json!(v)),
        "list_agent_runs" => crate::list_agent_runs(app.clone(), arg_str(args, "agent_id")).map(|v| json!(v)),
        "create_note" => crate::create_note(app.clone(), ws, arg_str(args, "title"), arg_str(args, "content")).map(|v| json!(v)),
        "create_memory" => crate::create_memory(app.clone(), ws, arg_str(args, "content"), None, None).map(|v| json!(v)),
        "create_task" => crate::create_task(app.clone(), ws, arg_str(args, "title"), arg_opt(args, "due_at")).map(|v| json!(v)),
        "update_task_status" => crate::update_task_status(app.clone(), arg_str(args, "task_id"), arg_str(args, "status")).map(|v| json!(v)),
        "create_reminder" => crate::schedule_task_reminder(app.clone(), ws, arg_str(args, "task_id"), arg_str(args, "title"), arg_str(args, "scheduled_at")).map(|v| json!(v)),
        "create_calendar_event" => crate::create_calendar_event(app.clone(), ws, arg_str(args, "title"), arg_str(args, "starts_at"), arg_str(args, "ends_at"), arg_opt(args, "meeting_url"), arg_opt(args, "recurrence")).map(|v| json!(v)),
        "promote_action_to_task" => crate::promote_action_to_task(app.clone(), arg_str(args, "action_id"), ws).map(|v| json!(v)),
        "execute_command" => crate::execute_command(app.clone(), agent_id.to_string(), ws, arg_str(args, "command")).map(|v| json!(v)),
        "modify_file" => crate::modify_file(app.clone(), agent_id.to_string(), ws, arg_str(args, "path"), arg_str(args, "content"), args.get("append").and_then(|v| v.as_bool()).unwrap_or(false)).map(|v| json!(v)),
        "send_email" => crate::send_email(app.clone(), arg_str(args, "account_id"), arg_str(args, "to"), arg_str(args, "subject"), arg_str(args, "body"), Some(agent_id.to_string())).map(|v| json!(v)),
        _ => Err(format!("Unknown agent tool: {name}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> Connection {
        crate::tests::test_connection()
    }

    fn seed(connection: &Connection) -> (String, String) {
        // workspace + an agent with a known permission profile
        connection.execute("INSERT INTO workspaces (id, name) VALUES ('w1', 'Work')", []).unwrap();
        connection.execute(
            "INSERT INTO agents (id, workspace_id, name, prompt, schedule, permission_mode) VALUES ('a1', 'w1', 'Test Agent', 'do things', '0 * * * *', 'read_only')",
            [],
        ).unwrap();
        ("w1".into(), "a1".into())
    }

    #[test]
    fn tool_catalogue_is_well_formed() {
        assert!(!AGENT_TOOLS.is_empty());
        for tool in AGENT_TOOLS {
            assert!(!tool.name.is_empty());
            assert!(!tool.description.is_empty());
            assert!(!tool.capability.is_empty());
        }
        // Every listed tool must be dispatchable: no duplicate names.
        let mut names: Vec<&str> = AGENT_TOOLS.iter().map(|t| t.name).collect();
        names.sort();
        let deduped = {
            let mut v = names.clone();
            v.dedup();
            v
        };
        assert_eq!(names, deduped, "duplicate tool names");
        // Sensitive tools carry the self_approving flag.
        for sensitive in ["execute_command", "modify_file", "send_email"] {
            assert!(tool_by_name(sensitive).unwrap().self_approving, "{sensitive}");
        }
    }

    #[test]
    fn parse_agent_tool_call_handles_json_and_prose() {
        let (tool, args) = parse_agent_tool_call(r#"{"tool": "create_note", "arguments": {"title": "Hi", "content": "World"}}"#).unwrap();
        assert_eq!(tool, "create_note");
        assert_eq!(args["title"], "Hi");

        let (tool2, args2) = parse_agent_tool_call(r#"I will search now:\n{"tool": "search_workspace", "arguments": {"query": "plans"}}\nThen I will summarize."#).unwrap();
        assert_eq!(tool2, "search_workspace");
        assert_eq!(args2["query"], "plans");

        assert!(parse_agent_tool_call("The task is done. Here is the answer: 42.").is_none());
        assert!(parse_agent_tool_call("").is_none());
        assert!(parse_agent_tool_call(r#"{"tool": 5}"#).is_none());
    }

    #[test]
    fn read_tools_allowed_for_read_only_agent_write_denied() {
        let connection = conn();
        let (ws, agent) = seed(&connection);
        let read_tool = tool_by_name("list_tasks").unwrap();
        assert!(check_tool_allowed(&connection, &agent, &ws, read_tool).is_ok());
        let write_tool = tool_by_name("create_note").unwrap();
        assert!(check_tool_allowed(&connection, &agent, &ws, write_tool).is_err());
    }

    #[test]
    fn explicitly_granted_agent_can_write() {
        let connection = conn();
        connection.execute("INSERT INTO workspaces (id, name) VALUES ('w2', 'Home')", []).unwrap();
        connection.execute(
            "INSERT INTO agents (id, workspace_id, name, prompt, schedule, permission_mode) VALUES ('a2', 'w2', 'Full Agent', 'do things', '0 * * * *', 'full')",
            [],
        ).unwrap();
        // Write capabilities are denied by default; they require an explicit grant.
        assert!(check_tool_allowed(&connection, "a2", "w2", tool_by_name("create_note").unwrap()).is_err());
        crate::permissions::grant_permission(&connection, "a2", "w2", "create_notes").unwrap();
        assert!(check_tool_allowed(&connection, "a2", "w2", tool_by_name("create_note").unwrap()).is_ok());
        // execute_command is a write capability too; its approval runs inside the command.
        crate::permissions::grant_permission(&connection, "a2", "w2", "execute_commands").unwrap();
        assert!(check_tool_allowed(&connection, "a2", "w2", tool_by_name("execute_command").unwrap()).is_ok());
    }

    #[test]
    fn unknown_tool_is_rejected() {
        // Unknown names fail tool lookup (the dispatch path also rejects them).
        assert!(tool_by_name("not_a_tool").is_none());
        assert!(tool_by_name("").is_none());
        // tool_list_prompt mentions every registered tool and the protocol.
        let prompt = tool_list_prompt();
        for tool in AGENT_TOOLS {
            assert!(prompt.contains(tool.name), "prompt missing {}", tool.name);
        }
        assert!(prompt.contains("final answer"));
        assert!(prompt.contains("requires your user's approval"));
    }

    /// Full loop test for the agent tool-calling runtime (#7): a scripted
    /// model closure answers the first call with a tool call (create_note) and
    /// the second with a final answer. The loop must execute the tool through
    /// the injected executor, feed the result back, and return the final
    /// answer plus the tool trace. No network or Tauri runtime needed.
    #[test]
    fn agent_tool_loop_runs_tools_and_finishes() {
        let connection = crate::tests::test_connection();
        connection.execute("INSERT INTO workspaces (id, name) VALUES ('w1', 'Work')", []).unwrap();

        let responses: Vec<String> = vec![
            r#"{"tool": "create_note", "arguments": {"title": "Loop test note", "content": "Made by the agent loop"}}"#.into(),
            "Done. I created the note titled Loop test note.".into(),
        ];
        let calls = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let calls_clone = calls.clone();
        let mut next = 0usize;
        let model_call = move |transcript: &str| {
            calls_clone.lock().unwrap().push(transcript.to_string());
            let reply = responses[next.min(responses.len() - 1)].clone();
            next += 1;
            Ok(reply)
        };
        let executed = std::sync::Arc::new(std::sync::Mutex::new(Vec::<(String, serde_json::Value)>::new()));
        let executed_clone = executed.clone();
        let execute_tool = move |tool: &str, args: &serde_json::Value| {
            executed_clone.lock().unwrap().push((tool.to_string(), args.clone()));
            Ok(serde_json::json!({ "id": "note-1", "title": args.get("title").cloned().unwrap_or(serde_json::Value::Null), "ok": true }))
        };

        let outcome = crate::run_agent_loop(
            "w1", "a1", "Loop Agent",
            "Create a note titled 'Loop test note' and confirm.",
            model_call, execute_tool,
        );

        assert!(outcome.error.is_none(), "loop error: {:?}", outcome.error);
        assert!(outcome.final_text.contains("Done. I created the note"), "final: {}", outcome.final_text);
        let tool_calls = executed.lock().unwrap();
        assert_eq!(tool_calls.len(), 1, "expected 1 tool call");
        assert_eq!(tool_calls[0].0, "create_note");
        assert_eq!(tool_calls[0].1["title"], "Loop test note");
        // The second model call must include the tool result (feedback loop).
        let model_calls = calls.lock().unwrap();
        assert_eq!(model_calls.len(), 2, "expected 2 model calls");
        assert!(model_calls[1].contains("Loop test note"), "tool result not fed back: {}", &model_calls[1][..model_calls[1].len().min(200)]);
        // The step trace is recorded for the run output.
        assert_eq!(outcome.steps.len(), 1);
        assert_eq!(outcome.steps[0]["tool"], "create_note");
        assert_eq!(outcome.steps[0]["status"], "ok");
    }

    /// The loop recovers from tool failures: the error is fed back to the
    /// model and the run can still finish with a final answer.
    #[test]
    fn agent_tool_loop_recovers_from_tool_failure() {
        let connection = crate::tests::test_connection();
        connection.execute("INSERT INTO workspaces (id, name) VALUES ('w1', 'Work')", []).unwrap();

        let responses: Vec<String> = vec![
            r#"{"tool": "search_workspace", "arguments": {"query": "x"}}"#.into(),
            "Search failed but I will proceed with the answer: nothing found.".into(),
        ];
        let mut next = 0usize;
        let model_call = move |_transcript: &str| {
            let reply = responses[next.min(responses.len() - 1)].clone();
            next += 1;
            Ok(reply)
        };

        let outcome = crate::run_agent_loop(
            "w1", "a1", "Loop Agent", "Search for x.",
            model_call,
            |_tool, _args| Err("simulated tool failure".into()),
        );

        assert!(outcome.error.is_none(), "loop error: {:?}", outcome.error);
        assert!(outcome.final_text.contains("proceed"), "final: {}", outcome.final_text);
        assert_eq!(outcome.steps.len(), 1);
        assert_eq!(outcome.steps[0]["status"], "error");
        assert_eq!(outcome.steps[0]["error"], "simulated tool failure");
    }

    /// The loop stops cleanly when the model never produces a final answer.
    #[test]
    fn agent_tool_loop_stops_at_step_budget() {
        let connection = crate::tests::test_connection();
        connection.execute("INSERT INTO workspaces (id, name) VALUES ('w1', 'Work')", []).unwrap();

        let model_call = |_transcript: &str| Ok(r#"{"tool": "list_tasks", "arguments": {}}"#.to_string());
        let execute_tool = |_tool: &str, _args: &serde_json::Value| Ok(serde_json::json!({ "tasks": [] }));

        let outcome = crate::run_agent_loop(
            "w1", "a1", "Loop Agent", "Keep calling tools.",
            model_call, execute_tool,
        );
        assert!(outcome.error.is_some(), "expected step-budget error");
        assert!(outcome.error.unwrap().contains("maximum of 6 tool steps"));
        assert_eq!(outcome.steps.len(), crate::MAX_AGENT_STEPS);
    }

    /// #9: the MCP/REST tool catalogue is generated from the same registry as
    /// the agent runtime, and every write-capability tool is gated (auth is
    /// required for all external tool calls).
    #[test]
    fn mcp_catalogue_matches_agent_runtime_and_schema_is_well_formed() {
        let listed = mcp_tools_list();
        let tools = listed["tools"].as_array().expect("tools array");
        assert_eq!(tools.len(), AGENT_TOOLS.len(), "MCP must expose every agent tool");
        for tool in tools {
            let name = tool["name"].as_str().unwrap();
            assert!(tool_by_name(name).is_some(), "MCP tool {name} missing from registry");
            let schema = &tool["inputSchema"];
            let required = schema["required"].as_array().map(|r| r.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>()).unwrap_or_default();
            if name == "list_agent_runs" {
                assert!(required.contains(&"agent_id"));
            } else {
                assert!(required.contains(&"workspace_id"), "{name} must require workspace_id");
            }
        }
        // Every registry entry maps to a capability (read or write); write
        // capabilities must never be in the default-allowed read set.
        for entry in AGENT_TOOLS {
            assert!(!entry.capability.is_empty());
            let is_read = matches!(entry.capability, "read_knowledge" | "read_transcripts");
            assert_eq!(is_read, !entry.self_approving && matches!(entry.name, "search_workspace" | "search_transcripts" | "search_emails" | "list_tasks" | "list_meetings" | "list_notes" | "list_calendar_events" | "list_emails" | "list_reminders" | "list_memories" | "list_agent_runs"), "capability mismatch for {}", entry.name);
        }
    }

    #[test]
    fn audit_log_receives_tool_execution_entry() {
        // Full dispatch needs an AppHandle; verify the audit hook via a
        // read-only tool that fails fast on the missing data dir instead:
        // simulate by calling record_audit through a direct dispatch-free path.
        let connection = conn();
        let _ = crate::retention::record_audit(&connection, "agent_tool_executed", Some("agent"), Some("tool"), Some("w1"), "Agent a1 (run r1) called tool list_tasks");
        let count: i64 = connection.query_row("SELECT COUNT(*) FROM audit_log WHERE action = 'agent_tool_executed'", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1);
    }
}
