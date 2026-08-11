# External Coding-Agent Integration

Neural Agent OS exposes controlled integration surfaces for Claude Code, Codex, and OpenCode.

## Available interfaces

### 1. MCP Server (loopback only, port 8787)

Connect any MCP-compatible agent to `http://127.0.0.1:8787/mcp`.

**Available tools:**

| Tool | Description | Permissions |
|------|-------------|-------------|
| `search_workspace` | Search indexed knowledge | Read knowledge |
| `list_tasks` | List workspace tasks | Read tasks |

**Claude Code setup:**

```bash
# Add to Claude Code MCP config (~/.claude/mcp.json)
{
  "mcpServers": {
    "neural-agent-os": {
      "url": "http://127.0.0.1:8787/mcp"
    }
  }
}
```

**Codex setup:**

```bash
codex mcp add neural-agent-os http://127.0.0.1:8787/mcp
```

**OpenCode setup:**

```bash
opencode mcp connect neural-agent-os --url http://127.0.0.1:8787/mcp
```

### 2. REST API (loopback only)

| Endpoint | Description |
|----------|-------------|
| `GET /health` | Health check |
| `GET /v1/workspaces` | List workspaces |
| `GET /v1/search?workspace_id=<id>&q=<query>` | Search workspace |
| `GET /v1/tasks?workspace_id=<id>` | List tasks |
| `GET /v1/meetings?workspace_id=<id>` | List meetings |

### 3. CLI

```bash
# Build the CLI binary
cargo build --manifest-path src-tauri/Cargo.toml --bin neural-cli

# Usage
neural-cli health
neural-cli workspaces
neural-cli search personal "quarterly planning"
neural-cli tasks personal
neural-cli meetings personal
```

### 4. Context export

Export workspace data as JSON for offline agent consumption:

```bash
# From the desktop app: Data → Export workspace
# Or programmatically via the REST API
curl http://127.0.0.1:8787/v1/export?workspace_id=personal > workspace.json
```

## Permission model

External agents start with **read-only** access by default. The user must
explicitly grant additional permissions per workspace:

| Permission | Scope |
|------------|-------|
| Read knowledge | Search indexed sources |
| Read transcripts | Read meeting transcripts |
| Create notes | Add notes to workspace |
| Create reminders | Schedule reminders |
| Modify tasks | Create/update/delete tasks |
| Schedule events | Create/modify calendar events |
| Draft email | Generate email drafts |
| Send email | Send through configured accounts |
| Modify files | Edit workspace files |
| Execute commands | Run shell commands |

Configure permissions in the Agents view when creating an agent. External
agents connect through MCP are read-only until the user explicitly enables
write access per workspace.

## Security

- All interfaces are loopback-only (`127.0.0.1`)
- No external network access
- API keys stored in OS keychain
- MCP tools require explicit workspace_id
- Write operations require approval by default
