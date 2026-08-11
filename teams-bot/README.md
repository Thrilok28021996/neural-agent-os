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
| `NEURAL_LOCAL_URL` | Neural Agent OS API URL | `http://host.docker.internal:8787` |
| `TEAMS_BOT_TOKEN` | Microsoft bot auth token | (required) |

## Deployment risks

Per the implementation plan, Teams bot APIs, permissions, and tenant policies
may limit automatic joining or recording. The bot is designed to fail gracefully
and report status. Test in a development tenant before production use.
