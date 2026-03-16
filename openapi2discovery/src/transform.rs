use crate::discovery::*;
use crate::resolver::{ref_name, RefResolver};
use crate::tree::{ends_with_param, parse_segments, resource_chain, Segment};
use openapiv3::*;
use serde_json::Value;
use std::collections::BTreeMap;

/// Convert an OpenAPI spec into a Discovery Document.
pub fn transform(
    spec: &OpenAPI,
    name_override: Option<&str>,
    version_override: Option<&str>,
) -> DiscoveryDocument {
    let resolver = RefResolver::new(spec);

    let name = name_override
        .map(str::to_owned)
        .unwrap_or_else(|| slugify(&spec.info.title));
    let version = version_override
        .map(str::to_owned)
        .unwrap_or_else(|| spec.info.version.clone());
    let root_url = spec
        .servers
        .first()
        .map(|s| s.url.clone())
        .unwrap_or_default();

    let id = format!("{name}:{version}");

    DiscoveryDocument {
        kind: "discovery#restDescription".into(),
        discovery_version: "v1".into(),
        id,
        title: spec.info.title.clone(),
        description: spec.info.description.clone(),
        protocol: "rest".into(),
        root_url,
        service_path: String::new(),
        schemas: lift_schemas(spec),
        resources: build_resources(spec, &resolver, &name),
        name,
        version,
    }
}

fn slugify(s: &str) -> String {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

// ---------------------------------------------------------------------------
// Schemas
// ---------------------------------------------------------------------------

fn lift_schemas(spec: &OpenAPI) -> BTreeMap<String, Value> {
    let Some(components) = &spec.components else {
        return BTreeMap::new();
    };

    components
        .schemas
        .iter()
        .map(|(name, schema)| {
            let val = serde_json::to_value(schema).unwrap_or_default();
            let mut cleaned = to_discovery_schema(val);
            if let Value::Object(ref mut m) = cleaned {
                m.insert("id".into(), Value::String(name.clone()));
            }
            (name.clone(), cleaned)
        })
        .collect()
}

/// Keys that exist in OpenAPI schemas but not in Google Discovery schemas.
///
/// Note: `oneOf`, `anyOf`, and `allOf` are also handled above for the
/// single-variant/nullable case — this list acts as the fallthrough that
/// strips them when they could not be flattened (e.g. multi-variant unions).
const OPENAPI_ONLY_KEYS: &[&str] = &[
    "required",
    "nullable",
    "oneOf",
    "anyOf",
    "allOf",
    "additionalProperties",
    "discriminator",
    "readOnly",
    "writeOnly",
    "xml",
    "externalDocs",
    "example",
    "deprecated",
    "minLength",
    "maxLength",
    "minimum",
    "maximum",
    "exclusiveMinimum",
    "exclusiveMaximum",
    "pattern",
    "minItems",
    "maxItems",
    "uniqueItems",
    "minProperties",
    "maxProperties",
];

/// Recursively convert an OpenAPI-style schema JSON value into Discovery format:
///  - Simplify `$ref` paths
///  - Strip OpenAPI-only keys
///  - Flatten `oneOf`/`anyOf`/`allOf` nullable patterns to a plain `$ref`
fn to_discovery_schema(value: Value) -> Value {
    match value {
        Value::Object(mut map) => {
            // --- flatten oneOf/anyOf nullable → $ref ---
            for key in &["oneOf", "anyOf"] {
                if let Some(arr) = map.get(*key).and_then(|v| v.as_array()) {
                    let refs: Vec<&Value> = arr
                        .iter()
                        .filter(|v| v.get("type").and_then(|t| t.as_str()) != Some("null"))
                        .collect();
                    if refs.len() == 1 {
                        let replacement = refs[0].clone();
                        map.remove(*key);
                        if let Value::Object(inner) = replacement {
                            for (k, v) in inner {
                                map.insert(k, v);
                            }
                        }
                    }
                }
            }

            // --- flatten allOf with single entry ---
            if let Some(arr) = map.get("allOf").and_then(|v| v.as_array()) {
                if arr.len() == 1 {
                    let replacement = arr[0].clone();
                    map.remove("allOf");
                    if let Value::Object(inner) = replacement {
                        for (k, v) in inner {
                            map.entry(k).or_insert(v);
                        }
                    }
                }
            }

            // --- simplify $ref paths ---
            if let Some(Value::String(r)) = map.get("$ref") {
                if let Some(name) = r.strip_prefix("#/components/schemas/") {
                    map.insert("$ref".into(), Value::String(name.into()));
                }
            }

            // --- strip OpenAPI-only keys ---
            for key in OPENAPI_ONLY_KEYS {
                map.remove(*key);
            }

            // --- recurse into remaining values ---
            Value::Object(
                map.into_iter()
                    .map(|(k, v)| (k, to_discovery_schema(v)))
                    .collect(),
            )
        }
        Value::Array(arr) => Value::Array(arr.into_iter().map(to_discovery_schema).collect()),
        other => other,
    }
}

// ---------------------------------------------------------------------------
// Resource tree
// ---------------------------------------------------------------------------

fn build_resources(
    spec: &OpenAPI,
    resolver: &RefResolver,
    service_name: &str,
) -> BTreeMap<String, DiscoveryResource> {
    let mut root: BTreeMap<String, DiscoveryResource> = BTreeMap::new();

    for (path_str, path_item_ref) in &spec.paths.paths {
        let ReferenceOr::Item(path_item) = path_item_ref else {
            continue;
        };

        let segments = parse_segments(path_str);
        let chain = resource_chain(&segments);
        if chain.is_empty() {
            continue;
        }

        let is_item = ends_with_param(&segments);

        let ops: [(&str, &Option<Operation>); 5] = [
            ("GET", &path_item.get),
            ("POST", &path_item.post),
            ("PUT", &path_item.put),
            ("PATCH", &path_item.patch),
            ("DELETE", &path_item.delete),
        ];

        for (verb, op) in ops {
            let Some(operation) = op else { continue };
            let method_name = method_name_for(verb, is_item, operation);
            let id = format!("{service_name}.{}.{method_name}", chain.join("."));
            let method = build_method(&MethodContext {
                id: &id,
                verb,
                path: path_str,
                segments: &segments,
                operation,
                path_item,
                resolver,
                spec,
            });
            insert_method(&mut root, &chain, &method_name, method);
        }
    }

    root
}

fn method_name_for(verb: &str, is_item: bool, op: &Operation) -> String {
    match (verb, is_item) {
        ("GET", false) => return "list".into(),
        ("GET", true) => return "get".into(),
        ("POST", false) => return "create".into(),
        ("PUT", true) => return "update".into(),
        ("PATCH", true) => return "patch".into(),
        ("DELETE", true) => return "delete".into(),
        _ => {}
    }
    op.operation_id
        .clone()
        .unwrap_or_else(|| verb.to_lowercase())
}

fn insert_method(
    resources: &mut BTreeMap<String, DiscoveryResource>,
    chain: &[String],
    method_name: &str,
    method: DiscoveryMethod,
) {
    let Some((head, rest)) = chain.split_first() else {
        return;
    };
    let resource = resources.entry(head.clone()).or_default();
    if rest.is_empty() {
        resource.methods.insert(method_name.into(), method);
    } else {
        insert_method(&mut resource.resources, rest, method_name, method);
    }
}

// ---------------------------------------------------------------------------
// Method construction
// ---------------------------------------------------------------------------

struct MethodContext<'a> {
    id: &'a str,
    verb: &'a str,
    path: &'a str,
    segments: &'a [Segment],
    operation: &'a Operation,
    path_item: &'a PathItem,
    resolver: &'a RefResolver<'a>,
    spec: &'a OpenAPI,
}

fn build_method(ctx: &MethodContext) -> DiscoveryMethod {
    let mut parameters = BTreeMap::new();

    for p in &ctx.path_item.parameters {
        if let Some(param) = ctx.resolver.resolve_parameter(p) {
            add_parameter(param, &mut parameters);
        }
    }
    for p in &ctx.operation.parameters {
        if let Some(param) = ctx.resolver.resolve_parameter(p) {
            add_parameter(param, &mut parameters);
        }
    }

    let mut parameter_order = Vec::new();
    for seg in ctx.segments {
        if let Segment::Param(name) = seg {
            parameter_order.push(name.clone());
            parameters
                .entry(name.clone())
                .or_insert_with(|| DiscoveryParameter {
                    param_type: "string".into(),
                    required: true,
                    location: "path".into(),
                    description: None,
                    format: None,
                    enum_values: None,
                    default: None,
                });
        }
    }

    let request = ctx
        .operation
        .request_body
        .as_ref()
        .and_then(|rb| ctx.resolver.resolve_request_body(rb))
        .and_then(|rb| rb.content.get("application/json"))
        .and_then(|media| media.schema.as_ref())
        .and_then(extract_schema_ref);

    let response = [200, 201]
        .iter()
        .find_map(|&code| {
            ctx.operation
                .responses
                .responses
                .get(&StatusCode::Code(code))
        })
        .and_then(|r| ctx.resolver.resolve_response(r))
        .and_then(|r| r.content.get("application/json"))
        .and_then(|media| media.schema.as_ref())
        .and_then(extract_schema_ref);

    DiscoveryMethod {
        id: ctx.id.into(),
        http_method: ctx.verb.into(),
        path: ctx.path.strip_prefix('/').unwrap_or(ctx.path).into(),
        description: ctx
            .operation
            .description
            .clone()
            .or_else(|| ctx.operation.summary.clone()),
        parameters,
        parameter_order,
        request,
        response,
        scopes: extract_scopes(ctx.operation, ctx.spec),
    }
}

fn add_parameter(param: &Parameter, out: &mut BTreeMap<String, DiscoveryParameter>) {
    let (data, location) = match param {
        Parameter::Query { parameter_data, .. } => (parameter_data, "query"),
        Parameter::Path { parameter_data, .. } => (parameter_data, "path"),
        Parameter::Header { .. } | Parameter::Cookie { .. } => return,
    };

    let (param_type, format, enum_values) = extract_type_info(&data.format);

    out.insert(
        data.name.clone(),
        DiscoveryParameter {
            param_type,
            required: data.required,
            location: location.into(),
            description: data.description.clone(),
            format,
            enum_values,
            default: None,
        },
    );
}

// ---------------------------------------------------------------------------
// Type extraction helpers
// ---------------------------------------------------------------------------

fn extract_type_info(
    pf: &ParameterSchemaOrContent,
) -> (String, Option<String>, Option<Vec<String>>) {
    match pf {
        ParameterSchemaOrContent::Schema(ReferenceOr::Item(schema)) => {
            type_from_schema_kind(&schema.schema_kind)
        }
        _ => ("string".into(), None, None),
    }
}

fn type_from_schema_kind(kind: &SchemaKind) -> (String, Option<String>, Option<Vec<String>>) {
    let SchemaKind::Type(t) = kind else {
        return ("string".into(), None, None);
    };

    match t {
        Type::String(s) => {
            let fmt = format_string(&s.format);
            let enums = if s.enumeration.is_empty() {
                None
            } else {
                Some(s.enumeration.iter().filter_map(|e| e.clone()).collect())
            };
            ("string".into(), fmt, enums)
        }
        Type::Integer(i) => ("integer".into(), format_string(&i.format), None),
        Type::Number(n) => ("number".into(), format_string(&n.format), None),
        Type::Boolean(_) => ("boolean".into(), None, None),
        Type::Array(_) => ("array".into(), None, None),
        Type::Object(_) => ("object".into(), None, None),
    }
}

fn format_string<T: std::fmt::Debug>(v: &VariantOrUnknownOrEmpty<T>) -> Option<String> {
    match v {
        VariantOrUnknownOrEmpty::Item(f) => Some(format!("{f:?}").to_lowercase()),
        VariantOrUnknownOrEmpty::Unknown(u) => Some(u.clone()),
        VariantOrUnknownOrEmpty::Empty => None,
    }
}

fn extract_schema_ref(schema: &ReferenceOr<Schema>) -> Option<SchemaRef> {
    match schema {
        ReferenceOr::Reference { reference } => Some(SchemaRef {
            ref_name: ref_name(reference).into(),
        }),
        ReferenceOr::Item(s) => match &s.schema_kind {
            SchemaKind::AllOf { all_of } if all_of.len() == 1 => extract_schema_ref(&all_of[0]),
            _ => None,
        },
    }
}

fn extract_scopes(operation: &Operation, spec: &OpenAPI) -> Vec<String> {
    let security = operation.security.as_ref().or(spec.security.as_ref());
    security
        .into_iter()
        .flatten()
        .flat_map(|req| req.values())
        .flatten()
        .cloned()
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_works() {
        assert_eq!(slugify("My Cool API"), "my-cool-api");
        assert_eq!(slugify("Petstore"), "petstore");
        assert_eq!(slugify("  Spaces  Everywhere  "), "spaces-everywhere");
    }

    #[test]
    fn simplify_refs_rewrites_component_refs() {
        let input = serde_json::json!({
            "properties": { "owner": { "$ref": "#/components/schemas/User" } }
        });
        let output = to_discovery_schema(input);
        assert_eq!(output["properties"]["owner"]["$ref"], "User");
    }

    #[test]
    fn strips_openapi_only_keys() {
        let input = serde_json::json!({
            "type": "object",
            "required": ["name"],
            "nullable": true,
            "additionalProperties": { "type": "string" },
            "properties": { "name": { "type": "string" } }
        });
        let output = to_discovery_schema(input);
        assert!(output.get("required").is_none());
        assert!(output.get("nullable").is_none());
        assert!(output.get("additionalProperties").is_none());
        assert_eq!(output["properties"]["name"]["type"], "string");
    }

    #[test]
    fn flattens_oneof_nullable() {
        let input = serde_json::json!({
            "oneOf": [
                { "type": "null" },
                { "$ref": "#/components/schemas/Foo" }
            ]
        });
        let output = to_discovery_schema(input);
        assert!(output.get("oneOf").is_none());
        assert_eq!(output["$ref"], "Foo");
    }

    #[test]
    fn method_name_mapping() {
        let op = Operation::default();
        assert_eq!(method_name_for("GET", false, &op), "list");
        assert_eq!(method_name_for("GET", true, &op), "get");
        assert_eq!(method_name_for("POST", false, &op), "create");
        assert_eq!(method_name_for("PUT", true, &op), "update");
        assert_eq!(method_name_for("PATCH", true, &op), "patch");
        assert_eq!(method_name_for("DELETE", true, &op), "delete");
    }
}
