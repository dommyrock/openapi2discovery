# CLI usage patterns from agents (extracted from gws)

How the `gws` CLI is actually invoked by AI agents and in multi-step workflows.
All examples below are sourced from `CONTEXT.md`, `skills/`, and `registry/recipes.yaml`
in [googleworkspace/cli](https://github.com/googleworkspace/cli).

## Agent rules of engagement (from CONTEXT.md)

Three mandatory rules given to every agent before it touches the CLI:

1. **Schema first** — *"If you don't know the exact JSON payload structure, run `gws schema <resource>.<method>` first to inspect the schema before executing."*
2. **Field masks always** — *"ALWAYS use field masks when listing or getting resources by appending `--params '{"fields": "id,name"}'` to avoid overwhelming your context window."*
3. **Dry-run before writes** — *"Always use the `--dry-run` flag for mutating operations (create, update, delete) to validate your JSON payload before actual execution."*

## Core syntax

```bash
gws <service> <resource> [sub-resource] <method> [flags]
```

## Navigation / help

```bash
gws --help
gws <service> --help
gws <service> <resource> --help
gws <service> <resource> <method> --help
```

## Schema introspection

```bash
gws schema drive.files.list
gws schema sheets.spreadsheets.create
```

## Reading data (GET / LIST)

```bash
# List with field masks to protect context window
gws drive files list --params '{"q": "name contains \"Report\"", "pageSize": 10}' \
  --fields "files(id,name,mimeType)"

# Get a single resource
gws gmail users messages get --params '{"userId": "me", "id": "MSG_123"}'

# Paginate with NDJSON
gws admin users list --params '{"domain": "example.com"}' --page-all
```

## Writing data (POST / PUT / PATCH)

```bash
# Create with --json (1:1 API payload)
gws sheets spreadsheets create --json '{"properties": {"title": "Q4 Budget"}}'

# Send with --json
gws gmail users messages send --params '{"userId": "me"}' --json '{"raw": "BASE64..."}'

# Dry-run a mutating call first
gws chat spaces messages create \
  --params '{"parent": "spaces/xyz"}' \
  --json '{"text": "Deploy complete."}' \
  --dry-run
```

## Helper commands (`+` prefixed, service-specific shortcuts)

```bash
gws gmail +send --to alice@example.com --subject "Hello" --body "Hi there"
gws gmail +reply --message-id MESSAGE_ID --body "Thanks!"
gws gmail +triage --max 5 --query 'from:boss'
gws drive +upload ./report.pdf --name "Q1 Report"
gws sheets +append --spreadsheet SPREADSHEET_ID --values "Alice,95"
gws sheets +read --spreadsheet ID --range "Sheet1!A1:D10"
gws calendar +agenda --today --timezone America/New_York
gws calendar +insert --summary 'Standup' --start '2026-06-17T09:00:00-07:00' \
  --end '2026-06-17T09:30:00-07:00'
gws docs +write --document DOC_ID --text 'Hello, world!'
gws chat +send --space spaces/AAAAxxxx --text 'Hello team!'
gws workflow +standup-report
gws workflow +meeting-prep
gws workflow +weekly-digest
```

## Multi-step agent workflows (from recipes.yaml)

**Create a doc, share it, notify by email:**
```bash
gws drive files copy --params '{"fileId": "TEMPLATE_DOC_ID"}' \
  --json '{"name": "Project Brief - Q2 Launch"}'
gws docs +write --document-id NEW_DOC_ID \
  --text '## Project: Q2 Launch\n\n### Objective\nLaunch the new feature by end of Q2.'
gws drive permissions create --params '{"fileId": "NEW_DOC_ID"}' \
  --json '{"role": "writer", "type": "user", "emailAddress": "team@company.com"}'
gws gmail +send --to reviewer@company.com --subject 'Please review: Project Brief' \
  --body 'https://docs.google.com/document/d/DOC_ID'
```

**Triage email → save attachments to Drive:**
```bash
gws gmail users messages list \
  --params '{"userId": "me", "q": "has:attachment from:client@example.com"}' --format table
gws gmail users messages get --params '{"userId": "me", "id": "MESSAGE_ID"}'
gws gmail users messages attachments get \
  --params '{"userId": "me", "messageId": "MESSAGE_ID", "id": "ATTACHMENT_ID"}'
gws drive +upload --file ./attachment.pdf --parent FOLDER_ID
```

**Read sheet data → generate doc report → share:**
```bash
gws sheets +read --spreadsheet SHEET_ID --range "Sales!A1:D"
gws docs documents create --json '{"title": "Sales Report - January 2025"}'
gws docs +write --document-id DOC_ID \
  --text '## Sales Report\n\nTotal deals: 45\nRevenue: $125,000'
gws drive permissions create --params '{"fileId": "DOC_ID"}' \
  --json '{"role": "reader", "type": "user", "emailAddress": "cfo@company.com"}'
```

**Create sheet events → schedule calendar:**
```bash
gws sheets +read --spreadsheet SHEET_ID --range "Events!A2:D"
gws calendar +insert --summary 'Team Standup' \
  --start '2026-01-20T09:00:00' --end '2026-01-20T09:30:00' \
  --attendee alice@company.com --attendee bob@company.com
```

**Post-mortem setup (cross-service):**
```bash
gws docs +write --title 'Post-Mortem: [Incident]' \
  --body '## Summary\n\n## Timeline\n\n## Root Cause\n\n## Action Items'
gws calendar +insert --summary 'Post-Mortem Review' \
  --attendee team@company.com \
  --start '2026-03-16T14:00:00' --end '2026-03-16T15:00:00'
gws chat +send --space spaces/ENG_SPACE --text 'Post-mortem scheduled.'
```

## Response sanitization

```bash
gws gmail users messages get --params '...' \
  --sanitize "projects/P/locations/L/templates/T"
```

## Agent surface registration

```bash
# Gemini extension
gemini extensions install https://github.com/googleworkspace/cli

# OpenClaw skills
npx skills add https://github.com/googleworkspace/cli
npx skills add https://github.com/googleworkspace/cli/tree/main/skills/gws-drive
```

## Key takeaways for our CLI

1. **Schema-first workflow** — agents always call `schema` before constructing payloads
2. **`--json` is the primary input** — no flag explosion, the agent sends the API payload directly
3. **`--dry-run` before every write** — agents validate before mutating
4. **Field masks on every read** — agents never fetch full payloads
5. **Helper commands (`+`)** — ergonomic shortcuts for common multi-field operations
6. **Multi-step recipes** — agents chain 3-6 CLI calls across services in a single workflow
7. **NDJSON for pagination** — pipe to `jq` for stream processing
8. **Consistent output** — all commands write JSON to stdout, errors to stderr
