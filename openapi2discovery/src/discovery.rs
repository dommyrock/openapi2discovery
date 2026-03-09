use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryDocument {
    pub kind: String,
    pub discovery_version: String,
    pub name: String,
    pub version: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub protocol: String,
    pub root_url: String,
    pub service_path: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub schemas: BTreeMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub resources: BTreeMap<String, DiscoveryResource>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct DiscoveryResource {
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub methods: BTreeMap<String, DiscoveryMethod>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub resources: BTreeMap<String, DiscoveryResource>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryMethod {
    pub id: String,
    pub http_method: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub parameters: BTreeMap<String, DiscoveryParameter>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub parameter_order: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request: Option<SchemaRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<SchemaRef>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiscoveryParameter {
    #[serde(rename = "type")]
    pub param_type: String,
    pub required: bool,
    pub location: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(rename = "enum", skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SchemaRef {
    #[serde(rename = "$ref")]
    pub ref_name: String,
}
