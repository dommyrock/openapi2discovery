use openapi2discovery::{parse_openapi, transform, DiscoveryDocument};

fn load(fixture: &str) -> DiscoveryDocument {
    let spec = parse_openapi(fixture).expect("failed to parse fixture");
    transform(&spec, None, None)
}

mod petstore_mock_json {
    use super::*;

    #[test]
    fn top_level_fields() {
        let doc = load("tests/fixtures/petstore.json");
        assert_eq!(doc.kind, "discovery#restDescription");
        assert_eq!(doc.discovery_version, "v1");
        assert_eq!(doc.name, "petstore");
        assert_eq!(doc.version, "1.0.0");
        assert_eq!(doc.title, "Petstore");
        assert_eq!(doc.protocol, "rest");
        assert_eq!(doc.root_url, "https://petstore.example.com/v1");
    }

    #[test]
    fn resource_methods() {
        let doc = load("tests/fixtures/petstore.json");
        let pets = doc.resources.get("pets").expect("missing pets resource");

        // Collection methods
        let list = pets.methods.get("list").expect("missing list");
        assert_eq!(list.http_method, "GET");
        assert_eq!(list.path, "pets");
        assert!(list.parameters.contains_key("limit"));
        assert_eq!(list.parameters["limit"].location, "query");
        assert!(!list.parameters["limit"].required);

        let create = pets.methods.get("create").expect("missing create");
        assert_eq!(create.http_method, "POST");
        assert!(create.request.is_some());
        assert_eq!(create.request.as_ref().unwrap().ref_name, "Pet");

        // Item methods
        let get = pets.methods.get("get").expect("missing get");
        assert_eq!(get.http_method, "GET");
        assert_eq!(get.path, "pets/{petId}");
        assert_eq!(get.parameter_order, vec!["petId"]);
        assert!(get.response.is_some());
        assert_eq!(get.response.as_ref().unwrap().ref_name, "Pet");

        let delete = pets.methods.get("delete").expect("missing delete");
        assert_eq!(delete.http_method, "DELETE");
    }

    #[test]
    fn schemas_lifted() {
        let doc = load("tests/fixtures/petstore.json");
        assert!(doc.schemas.contains_key("Pet"));
        assert!(doc.schemas.contains_key("Pets"));

        // $ref in Pets.items should be simplified
        let pets_schema = &doc.schemas["Pets"];
        assert_eq!(pets_schema["items"]["$ref"], "Pet");
    }

    #[test]
    fn multiple_verbs_same_path() {
        let doc = load("tests/fixtures/petstore.json");
        let pets = &doc.resources["pets"];
        // /pets has GET (list) and POST (create) — both should exist
        assert!(pets.methods.contains_key("list"));
        assert!(pets.methods.contains_key("create"));
        // /pets/{petId} has GET (get) and DELETE (delete) — both should exist
        assert!(pets.methods.contains_key("get"));
        assert!(pets.methods.contains_key("delete"));
    }

    #[test]
    fn round_trip_has_enough_info_for_http_request() {
        let doc = load("tests/fixtures/petstore.json");
        let get = &doc.resources["pets"].methods["get"];

        // Verify we have everything needed to construct an HTTP request
        assert!(!get.http_method.is_empty());
        assert!(!get.path.is_empty());
        assert!(!get.parameter_order.is_empty());
        for param_name in &get.parameter_order {
            assert!(get.parameters.contains_key(param_name));
            assert_eq!(get.parameters[param_name].location, "path");
        }
    }
}

mod nested_resources {
    use super::*;

    #[test]
    fn three_levels() {
        let doc = load("tests/fixtures/nested.json");

        let users = doc.resources.get("users").expect("missing users");
        assert!(users.methods.contains_key("list"));
        assert!(users.methods.contains_key("get"));

        let posts = users
            .resources
            .get("posts")
            .expect("missing posts under users");
        assert!(posts.methods.contains_key("list"));
        assert!(posts.methods.contains_key("create"));
        assert!(posts.methods.contains_key("get"));
        assert!(posts.methods.contains_key("update"));

        let comments = posts
            .resources
            .get("comments")
            .expect("missing comments under posts");
        assert!(comments.methods.contains_key("list"));
        assert!(comments.methods.contains_key("get"));
        assert!(comments.methods.contains_key("delete"));
    }

    #[test]
    fn method_ids_are_dotted() {
        let doc = load("tests/fixtures/nested.json");
        let get_comment =
            &doc.resources["users"].resources["posts"].resources["comments"].methods["get"];
        assert_eq!(get_comment.id, "blog-api.users.posts.comments.get");
    }

    #[test]
    fn parameter_order_follows_url() {
        let doc = load("tests/fixtures/nested.json");
        let delete_comment =
            &doc.resources["users"].resources["posts"].resources["comments"].methods["delete"];
        assert_eq!(
            delete_comment.parameter_order,
            vec!["userId", "postId", "commentId"]
        );
    }

    #[test]
    fn schema_refs_simplified() {
        let doc = load("tests/fixtures/nested.json");
        let post = &doc.schemas["Post"];
        // author.$ref should be "User" not "#/components/schemas/User"
        assert_eq!(post["properties"]["author"]["$ref"], "User");
    }
}

mod edge_cases {
    use super::*;

    #[test]
    fn empty_spec_no_panic() {
        let json = r#"{
            "openapi": "3.0.3",
            "info": { "title": "Empty", "version": "0" },
            "paths": {}
        }"#;
        let spec: openapiv3::OpenAPI = serde_json::from_str(json).unwrap();
        let doc = transform(&spec, None, None);
        assert!(doc.resources.is_empty());
        assert!(doc.schemas.is_empty());
    }

    #[test]
    fn name_and_version_overrides() {
        let json = r#"{
            "openapi": "3.0.3",
            "info": { "title": "Original", "version": "1.0" },
            "paths": {}
        }"#;
        let spec: openapiv3::OpenAPI = serde_json::from_str(json).unwrap();
        let doc = transform(&spec, Some("custom-name"), Some("v42"));
        assert_eq!(doc.name, "custom-name");
        assert_eq!(doc.version, "v42");
        assert_eq!(doc.title, "Original"); // title is NOT overridden
    }
}
