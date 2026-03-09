use openapiv3::OpenAPI;
use std::io::Read;

/// Parse an OpenAPI 3.x JSON spec from a file path, or from stdin if `path` is `"-"`.
pub fn parse_openapi(path: &str) -> Result<OpenAPI, Box<dyn std::error::Error>> {
    let json = if path == "-" {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        buf
    } else {
        std::fs::read_to_string(path)?
    };

    Ok(serde_json::from_str(&json)?)
}
