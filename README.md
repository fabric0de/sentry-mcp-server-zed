# Sentry MCP Server for Zed

Zed extension that runs `@sentry/mcp-server` in stdio mode.

## Configure

To use Sentry MCP, go to your [Sentry account API auth token settings](https://sentry.io/settings/account/api/auth-tokens/) and create a User Auth Token.

Recommended scopes:

- `org:read`
- `project:read`
- `project:write`
- `team:read`
- `team:write`
- `event:write`

Zed settings:

```json
{
  "context_servers": {
    "sentry-mcp": {
      "enabled": true,
      "settings": {
        "sentry_access_token": "sntryu_..."
      }
    }
  }
}
```

## Use In Agent Panel

- Open Zed Agent Panel and make sure `sentry-mcp` is enabled.
- Ask with explicit server intent at first:
  - `Use sentry-mcp to fetch 10 unresolved issues.`
  - `Use sentry-mcp to list my organizations.`

If the assistant replies with plain text instructions instead of running tools, check:

- Agent profile/tool permissions allow MCP tools.
- `sentry_access_token` exists in the active workspace settings (workspace settings can override global settings).

## Notes

- This extension follows the latest `@sentry/mcp-server` version.
