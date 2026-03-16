# Implementation Segments

## References

- [You Need to Rewrite Your CLI for AI Agents](https://justin.poehnelt.com/posts/rewrite-your-cli-for-ai-agents/)
- [The MCP Abstraction Tax](https://justin.poehnelt.com/posts/mcp-abstraction-tax/)
- [Google Workspace CLI (OSS reference implementation)](https://github.com/googleworkspace/cli)

## What we have (done)

- **Discovery Document generator** — OpenAPI 3.x → nested Discovery JSON with resources, methods, schemas, parameters, `$ref`s, scopes

## What's missing to build the runtime CLI

| Priority | Feature | What it does |
|----------|---------|--------------|
| **P0** | **Runtime command tree** | Read the Discovery Document and dynamically generate subcommands: `mycli api objects list`, `mycli api objects get` — no hardcoded commands |
| **P0** | **`--json` flag** | Accept raw JSON payloads that map 1:1 to request schemas for POST/PUT/PATCH |
| **P0** | **Schema introspection** | `mycli schema api.objects.get` dumps the method signature (params, request body, response type, scopes) as machine-readable JSON |
| **P1** | **HTTP execution** | Actually make the API call using `httpMethod`, `path`, `rootUrl`, and resolved parameters |
| **P1** | **`--dry-run`** | Validate the request against the schema without hitting the API |
| **P1** | **Field masks** | `--params '{"fields": "..."}'` to limit response size for agents |
| **P1** | **Input hardening** | Reject path traversal, control chars, pre-encoded URLs in parameter values |
| **P2** | **NDJSON pagination** | `--page-all` streams one JSON object per page instead of buffering |
| **P2** | **Skill files** | Markdown + YAML frontmatter encoding agent-specific guidance (e.g. "always use `--dry-run` before writes") |
| **P2** | **Response sanitization** | Filter API responses against prompt injection in embedded data |
| **P2** | **MCP surface** | Expose same Discovery-driven methods as MCP tools over stdio |

### Extras

Items not in the core priority list above but needed for a production-quality agent-first CLI.

| Priority | Feature | What it does |
|----------|---------|--------------|
| **P1** | **Authentication** | HTTP execution requires auth. At minimum `--bearer-token` flag and env var (`CLI_TOKEN`); extensible to OAuth2 flows, API keys, or mTLS later |
| **P1** | **Output formatting (`--output json\|table\|csv`)** | Agents need `json` (default), humans need `table`. Distinct from `--json` which controls *input*. All output modes must write to stdout for piping |
| **P1** | **Retry with backoff** | Retry on HTTP 429/5xx with exponential backoff respecting `Retry-After` headers. Essential for rate-limited APIs |
| **P1** | **Structured error output** | Errors must be predictable JSON (`{"error": {"code": 404, "message": "..."}}`), not raw HTTP bodies. Agents need parseable errors to decide next steps |
| **P2** | **Agent context file (`CONTEXT.md`)** | A single provider-agnostic document describing CLI syntax, rules, key flags, and usage patterns. Loaded into the agent's prompt at session start. Must work across Claude, Gemini, OpenAI Codex, and any other model — no provider-specific instructions or assumptions. The file should describe *what the CLI does and how to call it*, not *how a specific model should reason* |

## Reference implementation mapping (googleworkspace/cli)

The [gws CLI](https://github.com/googleworkspace/cli) is a Rust/clap 4 binary that implements all of the above. Below is where each feature lives in that codebase.

### P0 — Runtime command tree

| File | Role |
|------|------|
| `src/commands.rs` | `build_cli(doc)` recursively walks `RestDescription.resources` to create nested `clap::Command` subcommands |
| `src/discovery.rs` | Serde models (`RestDescription`, `RestResource`, `RestMethod`, `JsonSchema`, etc.) + `fetch_discovery_document()` with 24h file cache |
| `src/main.rs` | Two-phase parse: peels off service name from argv, fetches Discovery, then re-parses against the generated command tree |
| `src/services.rs` | `SERVICES` constant maps aliases (`drive`, `gmail`) to API name + version |
| `src/helpers/mod.rs` | `get_helper(service)` injects extra `+`-prefixed helper subcommands into the dynamic tree |

### P0 — `--json` flag

| File | Role |
|------|------|
| `src/commands.rs` | Adds `--json` arg only when `method.request.is_some()` |
| `src/executor.rs` | `parse_and_validate_inputs()` reads the value; `validate_body_against_schema()` recursively checks types, required fields, enums against the Discovery schema |

### P0 — Schema introspection

| File | Role |
|------|------|
| `src/schema.rs` | `handle_schema_command(path, resolve_refs)` — parses dotted paths like `drive.files.list`; `find_method()` walks the resource tree; `build_schema_output()` assembles JSON with params, request body (inlined), response, scopes; `resolve_schema_refs()` inlines `$ref`s with cycle detection |
| `src/main.rs` | Routes the `"schema"` subcommand |

### P1 — HTTP execution

| File | Role |
|------|------|
| `src/executor.rs` | `build_url()` substitutes path params into Discovery URL templates; `build_http_request()` creates `reqwest` builders with auth + query params; `build_multipart_stream()` handles file uploads (64KB chunks); response handling for JSON vs binary |
| `src/client.rs` | `build_client()` configures `reqwest::Client`; `send_with_retry()` retries on 429 with exponential backoff respecting `Retry-After` |
| `src/auth.rs` | `get_token(scopes)` via env vars, encrypted credentials, or Application Default Credentials |

### P1 — `--dry-run`

| File | Role |
|------|------|
| `src/commands.rs` | Global `--dry-run` flag with `action(SetTrue)` |
| `src/executor.rs` | Runs full validation pipeline (URL building, param + body schema checks) then prints the constructed request to stdout **without** sending it |

### P1 — Field masks

| File | Role |
|------|------|
| `src/executor.rs` | `fields` key in `--params` JSON is extracted as a query parameter |
| `CONTEXT.md` | Documents the pattern and instructs agents to always use field masks |

### P1 — Input hardening

| File | Role |
|------|------|
| `src/validate.rs` | `validate_safe_output_dir()` — rejects `../`, symlinks, absolute paths; `validate_resource_name()` — rejects control chars, `?`, `#`, `%`; `validate_api_identifier()` — allowlist `[a-zA-Z0-9_.-]`; `encode_path_segment()` — percent-encodes URL path segments; `reject_control_chars()` — blocks null bytes and ASCII control chars (40+ test cases) |
| `src/discovery.rs` | `fetch_discovery_document()` validates service/version via `validate_api_identifier()` before cache paths or URLs |
| `AGENTS.md` | Security checklist for contributors |

### P2 — NDJSON pagination

| File | Role |
|------|------|
| `src/commands.rs` | Defines `--page-all` (bool), `--page-limit` (default 10), `--page-delay` (default 100ms) on every method |
| `src/executor.rs` | Pagination loop: extracts `nextPageToken` from response → injects as query param → outputs one compact JSON line per page → sleeps `page-delay` between fetches |
| `src/formatter.rs` | `format_value_paginated()` — JSON emits NDJSON, CSV/table omits headers on continuation pages, YAML uses `---` separators |

### P2 — Skill files

| File | Role |
|------|------|
| `src/generate_skills.rs` | `gws generate-skills` auto-generates `SKILL.md` files: `render_service_skill()` for per-service skills, `render_helper_skill()` for helpers, `generate_shared_skill()` for shared auth/flags/security rules |
| `registry/personas.yaml` | 11 persona definitions (exec-assistant, project-manager, etc.) |
| `registry/recipes.yaml` | 30+ multi-step recipe definitions |
| `skills/` | 70+ generated `SKILL.md` files organized by category |
| `.github/workflows/generate-skills.yml` | Hourly cron regenerates skills when Discovery API changes |
| `.github/workflows/publish-skills.yml` | Publishes skills to ClawHub (OpenClaw registry) |

### P2 — Response sanitization

| File | Role |
|------|------|
| `src/helpers/modelarmor.rs` | `sanitize_text(template, text)` sends responses to Google Cloud Model Armor API; `SanitizeConfig` with Warn/Block modes; injects `+sanitize-prompt`, `+sanitize-response`, `+create-template` helper commands |
| `src/commands.rs` | Global `--sanitize <TEMPLATE>` flag (also via `GWS_SANITIZE_TEMPLATE` env var) |
| `src/executor.rs` | After JSON response, if `--sanitize` is set: Warn mode annotates with `_sanitization` field; Block mode suppresses output and exits non-zero |
| `templates/modelarmor/jailbreak.json` | Embedded preset template for jailbreak/prompt-injection detection |

### P2 — MCP surface

| File | Role |
|------|------|
| `gemini-extension.json` | Registers `gws` as a Gemini CLI extension with `contextFileName: "CONTEXT.md"` |
| `CONTEXT.md` | Injected into agent prompts — rules, core syntax, key flags, usage patterns |
| `skills/` | OpenClaw-compatible `SKILL.md` files discoverable as agent tools via ClawHub |
| `CLAUDE.md` / `AGENTS.md` | Agent and contributor configuration |

Note: `gws` does **not** implement MCP as an in-process server. Instead it relies on convention-based integration — structured JSON output, consistent error format, `--dry-run`, and skill files make it a natural fit for agent tool-use without implementing the MCP protocol directly.

---

### Extras — Authentication

| File | Role |
|------|------|
| `src/auth.rs` | `get_token(scopes)` acquires OAuth2 tokens via env vars, encrypted credential files, or Application Default Credentials |
| `src/executor.rs` | Attaches bearer token to every outgoing request |

### Extras — Output formatting

| File | Role |
|------|------|
| `src/formatter.rs` | `format_value()` dispatches to JSON (compact), table, CSV, or YAML formatters based on `--output` flag; `format_value_paginated()` handles continuation pages per format |
| `src/commands.rs` | Defines `--output` flag with allowed values |

### Extras — Retry with backoff

| File | Role |
|------|------|
| `src/client.rs` | `send_with_retry()` retries on HTTP 429 with exponential backoff (1s, 2s, 4s), respecting `Retry-After` headers |

### Extras — Structured error output

| File | Role |
|------|------|
| `src/executor.rs` | `handle_error_response()` parses Google API error JSON into a consistent shape; non-JSON errors are wrapped in the same structure |

### Extras — Agent context file

| File | Role |
|------|------|
| `CONTEXT.md` | The agent context document — rules, core syntax, key flags, field mask guidance, pagination patterns. In `gws` this is Gemini-specific; **our version must be provider-agnostic** (no model-specific reasoning instructions, no provider-specific tool-use syntax) |
| `gemini-extension.json` | Registers the context file with Gemini CLI. We would need equivalent registration for Claude (`CLAUDE.md`), Codex (`.codex/`), and any other agent surface — or a single `CONTEXT.md` that all providers can consume |
| `AGENTS.md` | Contributor-facing architecture and security guide (separate from the agent-facing context file) |

See: [cli-usage.md](gws-cli-usage.md) & [our-cli-usage.md](our-cli-usage.md)(private) for extracted CLI usage patterns from agents and multi-step workflows.
