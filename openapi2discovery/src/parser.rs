use openapiv3::OpenAPI;
use serde_json::Value;
use std::io::Read;

/// Parse an OpenAPI 3.x JSON spec from a file path, or from stdin if `path` is `"-"`.
///
/// Automatically converts OAS 3.1 type-arrays (e.g. `"type": ["string", "null"]`)
/// to OAS 3.0 style (`"type": "string", "nullable": true`) so that the `openapiv3`
/// crate (which only supports 3.0) can parse the spec.
pub fn parse_openapi(path: &str) -> Result<OpenAPI, Box<dyn std::error::Error>> {
    let json = if path == "-" {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        buf
    } else {
        std::fs::read_to_string(path)?
    };

    let mut value: Value = serde_json::from_str(&json)?;
    downgrade_oas31_types(&mut value);
    Ok(serde_json::from_value(value)?)
}

/// Recursively walk the JSON tree and convert OAS 3.1 type-arrays to OAS 3.0.
///
/// `"type": ["string", "null"]`  →  `"type": "string", "nullable": true`
/// `"type": ["null", "integer"]` →  `"type": "integer", "nullable": true`
///
/// Multi-type unions without null (e.g. `["string", "integer"]`) are left
/// untouched — `openapiv3` will reject them, which is the desired behaviour
/// since OAS 3.0 has no union types.
fn downgrade_oas31_types(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if let Some(type_val) = map.get("type") {
                if let Some(arr) = type_val.as_array() {
                    let types: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).collect();
                    let non_null: Vec<&str> =
                        types.iter().copied().filter(|t| *t != "null").collect();
                    let has_null = types.contains(&"null");

                    if non_null.len() == 1 {
                        map.insert("type".into(), Value::String(non_null[0].into()));
                        if has_null {
                            map.insert("nullable".into(), Value::Bool(true));
                        }
                    }
                }
            }
            for val in map.values_mut() {
                downgrade_oas31_types(val);
            }
        }
        Value::Array(arr) => {
            for item in arr {
                downgrade_oas31_types(item);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn converts_nullable_string() {
        let mut v = json!({"type": ["string", "null"]});
        downgrade_oas31_types(&mut v);
        assert_eq!(v, json!({"type": "string", "nullable": true}));
    }

    #[test]
    fn converts_null_first_order() {
        let mut v = json!({"type": ["null", "integer"]});
        downgrade_oas31_types(&mut v);
        assert_eq!(v, json!({"type": "integer", "nullable": true}));
    }

    #[test]
    fn leaves_plain_type_string_unchanged() {
        let mut v = json!({"type": "string"});
        downgrade_oas31_types(&mut v);
        assert_eq!(v, json!({"type": "string"}));
    }

    #[test]
    fn leaves_multi_type_union_unchanged() {
        let mut v = json!({"type": ["string", "integer"]});
        let expected = v.clone();
        downgrade_oas31_types(&mut v);
        assert_eq!(v, expected);
    }

    #[test]
    fn recurses_into_nested_properties() {
        let mut v = json!({
            "properties": {
                "name": { "type": ["string", "null"] },
                "items": [{ "type": ["boolean", "null"] }]
            }
        });
        downgrade_oas31_types(&mut v);
        assert_eq!(
            v,
            json!({
                "properties": {
                    "name": { "type": "string", "nullable": true },
                    "items": [{ "type": "boolean", "nullable": true }]
                }
            })
        );
    }
}
