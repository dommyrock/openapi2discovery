# OpenAPI to Discovery-Style Nested Schema

Convert flat OpenAPI 3.x specs into Google Discovery-style nested JSON schemas suitable for runtime CLI generation and AI agent consumption.

## Why?

Inspired by [Rewrite Your CLI for AI Agents](https://justin.poehnelt.com/posts/rewrite-your-cli-for-ai-agents/) — Google's Workspace CLI (`gws`) generates its entire command tree at runtime from a nested Discovery Document format. This nested structure is better for AI agents because:

- **Resources map to subcommands** — `drive files list` instead of flat paths like `/drive/v3/files`
- **1:1 JSON payloads** — agents generate request bodies that match the API schema directly
- **Schema introspection** — agents can query method signatures at runtime to understand parameters
- **Fewer tokens** — hierarchical nesting is more compact than enumerating all flat REST paths

The problem: Google's Discovery format is proprietary and only covers Google APIs. If you have your own API described by OpenAPI 3.x, there's no tool to produce an equivalent nested schema — until now.

## Tools

### Rust CLI (`openapi2discovery/`)

```bash
cd openapi2discovery && cargo build --release

# Basic usage
openapi2discovery input.json -o discovery.json

# Pretty print
openapi2discovery input.json --pretty

# From stdin
cat openapi.json | openapi2discovery - -o discovery.json

# Override service name/version
openapi2discovery input.json --name my-api --version v2
```

### Python script (quick prototyping)

```bash
python openapi_to_discovery.py openapi.json -o discovery.json
# or pipe
cat openapi.json | python openapi_to_discovery.py - | jq .
```

## Core transformation

1. Parse OpenAPI 3.x JSON spec
2. Group flat paths (`/users/{id}/posts/{postId}`) into a nested resource tree (`resources.users.resources.posts`)
3. Map HTTP verbs to semantic method names (`GET` collection → `list`, `GET` item → `get`, `POST` → `create`, etc.)
4. Lift `components.schemas` to top-level and simplify `$ref` paths
5. Output a Discovery-style JSON document with enough info to reconstruct valid HTTP requests
