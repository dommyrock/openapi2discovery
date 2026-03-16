use openapiv3::OpenAPI;
use serde_json::Value;
use std::collections::BTreeMap;

pub(crate) fn lift_schemas(spec: &OpenAPI) -> BTreeMap<String, Value> {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
