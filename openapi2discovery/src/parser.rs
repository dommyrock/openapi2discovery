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
fn downgrade_oas31_types(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if let Some(type_val) = map.get("type") {
                if let Some(arr) = type_val.as_array() {
                    let types: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).collect();
                    let non_null: Vec<&&str> = types.iter().filter(|t| **t != "null").collect();
                    let has_null = types.contains(&"null");

                    if non_null.len() == 1 {
                        map.insert("type".to_string(), Value::String(non_null[0].to_string()));
                        if has_null {
                            map.insert("nullable".to_string(), Value::Bool(true));
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
