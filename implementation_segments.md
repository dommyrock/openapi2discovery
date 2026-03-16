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
