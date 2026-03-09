#!/usr/bin/env python3
"""
openapi_to_discovery.py
Convert an OpenAPI 3.x JSON spec to a Google Discovery Document (nested format).

Usage:
    python openapi_to_discovery.py openapi.json > discovery.json
    python openapi_to_discovery.py openapi.json -o discovery.json
"""

import json
import sys
import re
import argparse
from typing import Any


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def slugify(text: str) -> str:
    """Turn a path segment like {fileId} → fileId, /v1/files → files."""
    return re.sub(r"[^a-zA-Z0-9_]", "", text)


def path_to_resource_chain(path: str) -> list[str]:
    """
    Break an OpenAPI path into a chain of resource names for nesting.

    /v1/files             → ["files"]
    /v1/files/{fileId}    → ["files"]          (param-only segment trimmed)
    /v1/drives/{driveId}/files → ["drives", "files"]
    """
    parts = [p for p in path.split("/") if p]
    chain: list[str] = []
    for part in parts:
        if part.startswith("{"):          # path param — skip as resource name
            continue
        if re.match(r"^v\d", part):       # version segment — skip
            continue
        chain.append(part)
    return chain or ["root"]


def openapi_param_to_discovery(param: dict) -> tuple[str, dict]:
    """Convert a single OpenAPI parameter object to a Discovery parameter entry."""
    name = param.get("name", "unknown")
    schema = param.get("schema", {})
    disc: dict[str, Any] = {
        "location": param.get("in", "query"),   # query | path | header
        "description": param.get("description", ""),
        "type": schema.get("type", "string"),
        "required": param.get("required", False),
    }
    if "enum" in schema:
        disc["enum"] = schema["enum"]
        disc["enumDescriptions"] = schema.get("x-enum-descriptions", [""] * len(schema["enum"]))
    if "default" in schema:
        disc["default"] = str(schema["default"])
    if schema.get("type") == "array":
        disc["repeated"] = True
        items = schema.get("items", {})
        disc["type"] = items.get("type", "string")
    return name, disc


def resolve_ref(ref: str, openapi: dict) -> dict:
    """Resolve a $ref like #/components/schemas/Foo into the actual schema dict."""
    if not ref.startswith("#/"):
        return {}
    parts = ref.lstrip("#/").split("/")
    node = openapi
    for p in parts:
        node = node.get(p, {})
    return node


def openapi_schema_to_discovery(schema: dict, openapi: dict, _depth=0) -> dict:
    """Recursively convert an OpenAPI schema to a Discovery schema object."""
    if "$ref" in schema:
        ref_name = schema["$ref"].split("/")[-1]
        return {"$ref": ref_name}

    stype = schema.get("type", "object")
    disc: dict[str, Any] = {"type": stype}

    if "description" in schema:
        disc["description"] = schema["description"]

    if stype == "object" or "properties" in schema:
        disc["type"] = "object"
        props = schema.get("properties", {})
        if props:
            disc["properties"] = {
                k: openapi_schema_to_discovery(v, openapi, _depth + 1)
                for k, v in props.items()
            }

    elif stype == "array":
        items = schema.get("items", {})
        disc["items"] = openapi_schema_to_discovery(items, openapi, _depth + 1)

    if "enum" in schema:
        disc["enum"] = schema["enum"]

    if "format" in schema:
        disc["format"] = schema["format"]

    return disc


def build_method(path: str, http_verb: str, op: dict, openapi: dict) -> dict:
    """Convert one OpenAPI operation into a Discovery method object."""
    op_id = op.get("operationId", f"{http_verb}_{slugify(path)}")
    method: dict[str, Any] = {
        "id": op_id,
        "path": path.lstrip("/"),
        "httpMethod": http_verb.upper(),
        "description": op.get("summary", op.get("description", "")),
        "parameters": {},
        "parameterOrder": [],
        "scopes": [],
    }

    # --- parameters ---
    for param in op.get("parameters", []):
        # resolve $ref params
        if "$ref" in param:
            param = resolve_ref(param["$ref"], openapi)
        name, disc_param = openapi_param_to_discovery(param)
        method["parameters"][name] = disc_param
        if param.get("required") and param.get("in") == "path":
            method["parameterOrder"].append(name)

    if not method["parameterOrder"]:
        del method["parameterOrder"]

    # --- request body → request $ref ---
    body = op.get("requestBody", {})
    if body:
        content = body.get("content", {})
        json_content = content.get("application/json", {})
        schema = json_content.get("schema", {})
        if "$ref" in schema:
            method["request"] = {"$ref": schema["$ref"].split("/")[-1]}
        elif schema:
            method["request"] = {"$ref": op_id + "Request"}

    # --- responses → response $ref ---
    responses = op.get("responses", {})
    for code in ("200", "201", "default"):
        resp = responses.get(code, {})
        if resp:
            content = resp.get("content", {})
            json_content = content.get("application/json", {})
            schema = json_content.get("schema", {})
            if "$ref" in schema:
                method["response"] = {"$ref": schema["$ref"].split("/")[-1]}
            elif schema:
                method["response"] = {"$ref": op_id + "Response"}
            break

    # --- security scopes ---
    for sec in op.get("security", []):
        for scope_list in sec.values():
            method["scopes"].extend(scope_list)
    if not method["scopes"]:
        del method["scopes"]

    return op_id.split(".")[-1] if "." in op_id else op_id.replace("-", "_"), method


def nest_into_resources(resources: dict, chain: list[str], method_name: str, method: dict):
    """Recursively nest a method into the resource tree at the given chain depth."""
    if not chain:
        resources.setdefault("methods", {})[method_name] = method
        return
    head, *tail = chain
    resources.setdefault("resources", {}).setdefault(head, {})
    nest_into_resources(resources["resources"][head], tail, method_name, method)


# ---------------------------------------------------------------------------
# Main conversion
# ---------------------------------------------------------------------------

def convert(openapi: dict) -> dict:
    info = openapi.get("info", {})
    servers = openapi.get("servers", [{}])
    base_url = servers[0].get("url", "https://example.com") if servers else "https://example.com"

    # Strip trailing slash & derive base path
    base_url = base_url.rstrip("/")
    version = info.get("version", "v1")

    discovery: dict[str, Any] = {
        "kind": "discovery#restDescription",
        "discoveryVersion": "v1",
        "id": f"{slugify(info.get('title', 'api'))}:{version}",
        "name": slugify(info.get("title", "api")).lower(),
        "version": version,
        "title": info.get("title", ""),
        "description": info.get("description", ""),
        "rootUrl": base_url + "/",
        "baseUrl": base_url + "/",
        "basePath": "/",
        "documentationLink": info.get("termsOfService", ""),
        "protocol": "rest",
        "resources": {},
        "schemas": {},
        "parameters": {},
    }

    # --- global parameters (e.g. api key, pretty-print, etc.) ---
    # OpenAPI doesn't have a clean equivalent, but components/parameters can serve
    for name, param in openapi.get("components", {}).get("parameters", {}).items():
        pname, disc_param = openapi_param_to_discovery(param)
        discovery["parameters"][pname] = disc_param

    # --- schemas ---
    for schema_name, schema in openapi.get("components", {}).get("schemas", {}).items():
        disc_schema = openapi_schema_to_discovery(schema, openapi)
        disc_schema["id"] = schema_name
        discovery["schemas"][schema_name] = disc_schema

    # --- paths → nested resources ---
    for path, path_item in openapi.get("paths", {}).items():
        chain = path_to_resource_chain(path)
        for verb in ("get", "post", "put", "patch", "delete", "options", "head"):
            op = path_item.get(verb)
            if not op:
                continue
            # Merge path-level parameters into operation
            op.setdefault("parameters", [])
            for p in path_item.get("parameters", []):
                if p not in op["parameters"]:
                    op["parameters"].append(p)

            method_key, method_obj = build_method(path, verb, op, openapi)
            nest_into_resources({"resources": discovery["resources"]}, chain, method_key, method_obj)

    # Clean up empty top-level keys
    for key in ("parameters", "schemas"):
        if not discovery[key]:
            del discovery[key]

    return discovery


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(description="Convert OpenAPI 3.x JSON → Google Discovery Document")
    parser.add_argument("input", help="Path to openapi.json (use - for stdin)")
    parser.add_argument("-o", "--output", help="Output path (default: stdout)")
    parser.add_argument("--indent", type=int, default=2, help="JSON indent level (default: 2)")
    args = parser.parse_args()

    if args.input == "-":
        openapi = json.load(sys.stdin)
    else:
        with open(args.input) as f:
            openapi = json.load(f)

    discovery = convert(openapi)
    output_str = json.dumps(discovery, indent=args.indent)

    if args.output:
        with open(args.output, "w") as f:
            f.write(output_str)
        print(f"✓ Written to {args.output}", file=sys.stderr)
    else:
        print(output_str)


if __name__ == "__main__":
    main()