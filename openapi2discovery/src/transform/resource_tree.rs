use crate::discovery::*;
use crate::resolver::{ref_name, RefResolver};
use crate::tree::{ends_with_param, parse_segments, resource_chain, Segment};
use openapiv3::*;
use std::collections::BTreeMap;

pub(crate) fn build_resources(
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

#[cfg(test)]
mod tests {
    use super::*;

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
