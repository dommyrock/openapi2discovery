# Google Discovery Service & Runtime CLI Generation

How the Google Workspace CLI (`gws`) uses Google's Discovery Service to dynamically generate its entire command surface at runtime — with zero hardcoded commands.

---

## What is the Google Discovery Service?

Think of it as a **live, machine-readable API catalog**. Every Google API (Drive, Gmail, Calendar, Sheets, etc.) publishes a **Discovery Document** — a big JSON file that completely describes the API:

- Every resource (files, messages, events...)
- Every method (list, get, create, delete...)
- Every parameter (name, type, required/optional, default values...)
- Every schema (what a "File" object looks like, what a "Message" contains...)
- Auth scopes, pagination, URL paths — everything

You can see a real one right now:

```
https://www.googleapis.com/discovery/v1/apis/drive/v3/rest
```

That URL returns a ~300KB JSON document that **is** the Drive API specification. It's not documentation *about* the API — it's the machine-readable **contract** of what the API accepts and returns, published by Google and updated whenever the API changes.

## How `$ref` Resolution Works

The Discovery Document uses JSON Schema's `$ref` (reference) mechanism to avoid repeating itself. For example, in the Drive API:

```json
"lastModifyingUser": {
  "$ref": "User"        // not defined inline, points to the "User" schema
},
"owners": {
  "type": "array",
  "items": { "$ref": "User" }   // same "User" schema, reused
}
```

And elsewhere in the same document, `User` is fully defined:

```json
"User": {
  "type": "object",
  "properties": {
    "displayName": { "type": "string" },
    "emailAddress": { "type": "string" },
    "photoLink": { "type": "string" }
  }
}
```

**Dynamic `$ref` resolution** means `gws` follows these references at runtime to build the complete picture — "a File has owners, owners are Users, a User has displayName/emailAddress/photoLink." It doesn't need this hardcoded; it walks the JSON graph.

## How `gws` Uses This — Step by Step

Here's what happens when you run `gws drive files list`:

```
┌─────────────────────────────────────────────────────────┐
│  1. User types:  gws drive files list                   │
│                       ─────                             │
│                       "drive" ← identifies the service  │
└──────────────────────────┬──────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────┐
│  2. gws fetches (and caches for 24h):                   │
│     googleapis.com/discovery/v1/apis/drive/v3/rest      │
│                                                         │
│     Gets back the full Drive API spec as JSON           │
└──────────────────────────┬──────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────┐
│  3. gws parses the Discovery Document:                  │
│                                                         │
│     resources.files.methods.list → {                    │
│       path: "drive/v3/files",                           │
│       httpMethod: "GET",                                │
│       parameters: { pageSize, pageToken, q, fields },   │
│       response: { $ref: "FileList" }                    │
│     }                                                   │
│                                                         │
│     Resolves $ref: "FileList" → schema with             │
│       files: [{ $ref: "File" }] → full File schema      │
└──────────────────────────┬──────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────┐
│  4. gws dynamically builds the CLI command:             │
│     - Knows what params are valid (pageSize, q, etc.)   │
│     - Knows their types (integer, string, etc.)         │
│     - Knows the HTTP method and URL path                │
│     - Can validate your --params JSON against the spec  │
└──────────────────────────┬──────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────┐
│  5. Makes the actual API call and returns structured    │
│     JSON response                                       │
└─────────────────────────────────────────────────────────┘
```

## Why This is Powerful

**Traditional CLI approach** (e.g. a hypothetical `gdrive` tool):

- Developer manually writes a `files list` command with hardcoded flags
- Google adds a new parameter `includeLabels` to the API
- CLI is now outdated — needs a new release to support it
- Docs drift from reality

**`gws` approach:**

- Zero hardcoded commands — the Discovery Document *is* the command tree
- Google adds `includeLabels` → it appears in the Discovery Document → `gws` supports it immediately
- `gws schema drive.files.list` always returns the **current** API contract, not stale docs

This is what the author means by:

> "The CLI becomes the canonical source of truth for what the API accepts **right now**, not what the docs said six months ago."

The Discovery Document is a **live spec** that Google keeps in sync with their actual API servers. By building the CLI entirely from that spec at runtime, `gws` can never go stale.

## Schema Introspection for Agents

An AI agent can run:

```bash
gws schema drive.files.list
```

And get back the full method signature as JSON — parameters, types, request body shape, response shape — all with `$ref`s resolved. The agent doesn't need to read documentation or guess at flags. It gets the machine-readable truth of what the API accepts, and can generate a valid request directly from it:

```bash
gws drive files list --params '{"pageSize": 10, "q": "mimeType=\"application/pdf\""}'
```

---

## Discovery Document is NOT OpenAPI

Google's Discovery format **predates** OpenAPI — it's a custom Google-specific format that has existed since ~2010, before the OpenAPI spec even existed (OpenAPI 2.0/Swagger was formalized in 2014).

### Discovery Document vs OpenAPI

| | Google Discovery | OpenAPI (Swagger) |
|---|---|---|
| **Origin** | Google, ~2010 | SmartBear/Linux Foundation, 2014+ |
| **Purpose** | Describe Google APIs for auto-generating client libraries | Industry standard for describing any REST API |
| **Schema format** | JSON Schema-like (custom subset) | JSON Schema (strict) |
| **Scope** | Only Google APIs | Any API |
| **URL pattern** | `$discovery/rest` endpoint per service | Typically a static `openapi.json` file |

They look superficially similar (both are JSON, both describe endpoints/params/schemas), but the structure is different. For example, Discovery uses `resources` nested hierarchically:

```json
{
  "resources": {
    "files": {
      "methods": {
        "list": { "httpMethod": "GET", "path": "files" },
        "get":  { "httpMethod": "GET", "path": "files/{fileId}" }
      }
    }
  }
}
```

Whereas OpenAPI uses flat path keys:

```json
{
  "paths": {
    "/files": { "get": { ... } },
    "/files/{fileId}": { "get": { ... } }
  }
}
```

### How Google Generates These

Google doesn't publicly document the exact generation pipeline, but what's known:

1. **It's generated from Google's internal API infrastructure** — Google uses an internal API framework (originally called "protorpc", now part of their infrastructure built on Protocol Buffers + gRPC). Their APIs are defined internally using `.proto` files and internal annotations.

2. **The Discovery Document is an auto-generated projection** of that internal definition into a REST-oriented JSON format. Google doesn't hand-write these — they're produced by internal tooling that reads the API's protobuf service definitions and outputs the Discovery JSON.

3. **It's served as a live endpoint**, not a static file. When you hit `https://www.googleapis.com/discovery/v1/apis/drive/v3/rest`, you're hitting an actual service that returns the current spec. When Google deploys a change to the Drive API (adds a parameter, adds a method), the Discovery Document updates automatically because it's derived from the same source of truth.

### The Generation Chain

```
Internal .proto definitions (source of truth)
    │
    ▼
Google's API infrastructure compiles these into:
    ├── gRPC server implementation
    ├── REST gateway (the actual API you call)
    └── Discovery Document (the metadata endpoint)
        │
        ▼
    googleapis.com/discovery/v1/apis/{service}/{version}/rest
```

### What Google Publicly Uses It For

Google originally built Discovery Documents to **auto-generate their client libraries**. The official Google API client libraries for Python, Java, Node, Go, Ruby, .NET, etc. are all generated from these documents. That's why they exist — they're the input format for Google's client library generators.

`gws` is essentially doing the same thing, but **at runtime instead of build time** — it reads the Discovery Document on the fly and builds a CLI from it, rather than generating static code.

### Converting to OpenAPI

It's possible, and people have done it. Community tools like [google-discovery-to-openapi](https://github.com/stackql/google-discovery-to-openapi) convert Discovery Documents to OpenAPI 3.x. But `gws` consumes the native Discovery format directly — no conversion needed.

---

## References

- Google Discovery API reference: https://developers.google.com/discovery/v1/reference
- Google Workspace CLI repo: https://github.com/googleworkspace/cli
- Author writeup: https://justin.poehnelt.com/posts/rewrite-your-cli-for-ai-agents/
