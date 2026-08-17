# Teams Meeting Bot

Optional Docker-deployed service for joining Microsoft Teams meetings.

## Quick start

```bash
cd teams-bot
docker build -t neural-teams-bot .
docker run -d --name neural-teams \
  -e TEAMS_BOT_TOKEN=your_token \
  -e NEURAL_LOCAL_URL=http://host.docker.internal:8787 \
  -p 9000:9000 \
  neural-teams-bot
```

## How it works

1. The desktop application detects Teams meeting URLs in calendar events.
2. It sends join instructions to the bot via the local REST API.
3. The bot reports its status (joining, joined, left) back to the application.
4. For full audio capture, additional Microsoft Graph API permissions are required.

## Configuration

| Variable | Description | Default |
|----------|-------------|---------|
| `NAO_TEAMS_BOT_SECRET` | Shared secret with the local app — used for both API auth (`X-Teams-Bot-Token`) and AES-256-GCM recording encryption. **Must match the app's `NAO_TEAMS_BOT_SECRET`.** | (required) |
| `NEURAL_API_URL` | Neural Agent OS API URL | `http://host.docker.internal:8787` |
| `NEURAL_TEAMS_CLIENT_ID` / `NEURAL_TEAMS_CLIENT_SECRET` / `NEURAL_TEAMS_TENANT_ID` | Microsoft Entra app credentials for Graph client-credentials auth | `common` tenant |
| `NEURAL_TEAMS_DATA_DIR` | Recording/metadata directory inside the container | `/data` |

## Secure transfer contract

The bot authenticates every request to the local app with an
`X-Teams-Bot-Token: <NAO_TEAMS_BOT_SECRET>` header; without a valid token the
app rejects the request (HTTP 401). Recordings are encrypted with AES-256-GCM
(key = SHA-256 of the shared secret) and uploaded as a raw binary body to
`POST /teams-bot/upload?meeting_id=...&title=...` — the app decrypts and
validates them locally before storing. Uploads retry on failure and meeting
state is preserved until the transfer completes.

## Deployment risks

Per the implementation plan, Teams bot APIs, permissions, and tenant policies
may limit automatic joining or recording. The bot is designed to fail gracefully
and report status. Test in a development tenant before production use.
