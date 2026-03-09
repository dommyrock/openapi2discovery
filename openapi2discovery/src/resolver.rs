use openapiv3::{OpenAPI, Parameter, ReferenceOr, RequestBody, Response, Schema};
use std::collections::BTreeMap;

/// Lookup tables for resolving `$ref` pointers within an OpenAPI spec.
pub struct RefResolver<'a> {
    schemas: BTreeMap<&'a str, &'a Schema>,
    parameters: BTreeMap<&'a str, &'a Parameter>,
    request_bodies: BTreeMap<&'a str, &'a RequestBody>,
    responses: BTreeMap<&'a str, &'a Response>,
}

/// Extract the terminal name from a `$ref` path: `#/components/schemas/User` → `User`.
pub fn ref_name(ref_path: &str) -> &str {
    ref_path.rsplit('/').next().unwrap_or(ref_path)
}

macro_rules! resolve_method {
    ($fn_name:ident, $field:ident, $T:ty) => {
        pub fn $fn_name<'b>(&'b self, r: &'b ReferenceOr<$T>) -> Option<&'b $T>
        where
            'a: 'b,
        {
            match r {
                ReferenceOr::Item(item) => Some(item),
                ReferenceOr::Reference { reference } => self.$field.get(ref_name(reference)).copied(),
            }
        }
    };
}

impl<'a> RefResolver<'a> {
    pub fn new(spec: &'a OpenAPI) -> Self {
        let mut resolver = Self {
            schemas: BTreeMap::new(),
            parameters: BTreeMap::new(),
            request_bodies: BTreeMap::new(),
            responses: BTreeMap::new(),
        };

        let Some(components) = &spec.components else {
            return resolver;
        };

        for (name, r) in &components.schemas {
            if let ReferenceOr::Item(v) = r {
                resolver.schemas.insert(name, v);
            }
        }
        for (name, r) in &components.parameters {
            if let ReferenceOr::Item(v) = r {
                resolver.parameters.insert(name, v);
            }
        }
        for (name, r) in &components.request_bodies {
            if let ReferenceOr::Item(v) = r {
                resolver.request_bodies.insert(name, v);
            }
        }
        for (name, r) in &components.responses {
            if let ReferenceOr::Item(v) = r {
                resolver.responses.insert(name, v);
            }
        }

        resolver
    }

    resolve_method!(resolve_schema, schemas, Schema);
    resolve_method!(resolve_parameter, parameters, Parameter);
    resolve_method!(resolve_request_body, request_bodies, RequestBody);
    resolve_method!(resolve_response, responses, Response);
}
