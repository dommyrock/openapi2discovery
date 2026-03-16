mod resource_tree;
mod schemas;

use crate::discovery::DiscoveryDocument;
use crate::resolver::RefResolver;
use openapiv3::OpenAPI;

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
        schemas: schemas::lift_schemas(spec),
        resources: resource_tree::build_resources(spec, &resolver, &name),
        name,
        version,
    }
}

/// Lowercase a title and join words with hyphens: `"My Cool API"` → `"my-cool-api"`.
fn slugify(s: &str) -> String {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_works() {
        assert_eq!(slugify("My Cool API"), "my-cool-api");
        assert_eq!(slugify("Petstore"), "petstore");
        assert_eq!(slugify("  Spaces  Everywhere  "), "spaces-everywhere");
    }
}
