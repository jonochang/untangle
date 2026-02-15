use std::path::Path;

/// A parsed OpenAPI endpoint.
#[derive(Debug, Clone)]
pub struct OpenApiEndpoint {
    /// HTTP method (GET, POST, PUT, DELETE, PATCH)
    pub method: String,
    /// URL path (e.g., "/api/v1/users/{id}")
    pub path: String,
    /// Operation ID if specified
    pub operation_id: Option<String>,
}

/// Parsed OpenAPI specification.
#[derive(Debug, Clone, Default)]
pub struct OpenApiSpec {
    /// All endpoints defined in the spec
    pub endpoints: Vec<OpenApiEndpoint>,
    /// Base URL / server URL if specified
    pub servers: Vec<String>,
}

/// Parse an OpenAPI specification from a YAML or JSON file.
pub fn parse_openapi_spec(path: &Path) -> Result<OpenApiSpec, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;

    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match ext {
        "yaml" | "yml" => parse_openapi_yaml(&content),
        "json" => parse_openapi_json(&content),
        _ => {
            // Try YAML first, fall back to JSON
            parse_openapi_yaml(&content).or_else(|_| parse_openapi_json(&content))
        }
    }
}

/// Parse OpenAPI from a YAML string.
pub fn parse_openapi_yaml(content: &str) -> Result<OpenApiSpec, String> {
    let value: serde_yaml::Value =
        serde_yaml::from_str(content).map_err(|e| format!("YAML parse error: {e}"))?;
    parse_openapi_value(&value)
}

/// Parse OpenAPI from a JSON string.
pub fn parse_openapi_json(content: &str) -> Result<OpenApiSpec, String> {
    let json_value: serde_json::Value =
        serde_json::from_str(content).map_err(|e| format!("JSON parse error: {e}"))?;
    // Convert JSON value to YAML value for unified processing
    let yaml_str =
        serde_yaml::to_string(&json_value).map_err(|e| format!("Conversion error: {e}"))?;
    let value: serde_yaml::Value =
        serde_yaml::from_str(&yaml_str).map_err(|e| format!("YAML parse error: {e}"))?;
    parse_openapi_value(&value)
}

fn parse_openapi_value(value: &serde_yaml::Value) -> Result<OpenApiSpec, String> {
    let mut spec = OpenApiSpec::default();

    // Extract servers
    if let Some(servers) = value.get("servers") {
        if let Some(servers_seq) = servers.as_sequence() {
            for server in servers_seq {
                if let Some(url) = server.get("url").and_then(|u| u.as_str()) {
                    spec.servers.push(url.to_string());
                }
            }
        }
    }

    // Extract paths
    if let Some(paths) = value.get("paths") {
        if let Some(paths_map) = paths.as_mapping() {
            for (path_key, path_value) in paths_map {
                let path_str = match path_key.as_str() {
                    Some(s) => s.to_string(),
                    None => continue,
                };

                let methods = ["get", "post", "put", "delete", "patch", "options", "head"];

                if let Some(path_map) = path_value.as_mapping() {
                    for (method_key, method_value) in path_map {
                        if let Some(method) = method_key.as_str() {
                            if methods.contains(&method) {
                                let operation_id = method_value
                                    .get("operationId")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string());

                                spec.endpoints.push(OpenApiEndpoint {
                                    method: method.to_uppercase(),
                                    path: path_str.clone(),
                                    operation_id,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(spec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_openapi_yaml() {
        let yaml = r#"
openapi: "3.0.0"
info:
  title: User API
  version: "1.0"
servers:
  - url: https://api.example.com/v1
paths:
  /users:
    get:
      operationId: listUsers
      summary: List all users
    post:
      operationId: createUser
      summary: Create a user
  /users/{id}:
    get:
      operationId: getUser
      summary: Get a user
    delete:
      operationId: deleteUser
      summary: Delete a user
"#;
        let spec = parse_openapi_yaml(yaml).unwrap();
        assert_eq!(spec.servers, vec!["https://api.example.com/v1"]);
        assert_eq!(spec.endpoints.len(), 4);

        let get_users = spec
            .endpoints
            .iter()
            .find(|e| e.path == "/users" && e.method == "GET")
            .unwrap();
        assert_eq!(get_users.operation_id.as_deref(), Some("listUsers"));

        let delete_user = spec
            .endpoints
            .iter()
            .find(|e| e.method == "DELETE")
            .unwrap();
        assert_eq!(delete_user.path, "/users/{id}");
        assert_eq!(delete_user.operation_id.as_deref(), Some("deleteUser"));
    }

    #[test]
    fn parse_openapi_json_format() {
        let json = r#"{
            "openapi": "3.0.0",
            "info": {"title": "Test API", "version": "1.0"},
            "paths": {
                "/items": {
                    "get": {
                        "operationId": "listItems"
                    }
                }
            }
        }"#;
        let spec = parse_openapi_json(json).unwrap();
        assert_eq!(spec.endpoints.len(), 1);
        assert_eq!(spec.endpoints[0].method, "GET");
        assert_eq!(spec.endpoints[0].path, "/items");
        assert_eq!(spec.endpoints[0].operation_id.as_deref(), Some("listItems"));
    }

    #[test]
    fn parse_openapi_without_operation_ids() {
        let yaml = r#"
openapi: "3.0.0"
info:
  title: Minimal API
  version: "1.0"
paths:
  /health:
    get:
      summary: Health check
"#;
        let spec = parse_openapi_yaml(yaml).unwrap();
        assert_eq!(spec.endpoints.len(), 1);
        assert_eq!(spec.endpoints[0].path, "/health");
        assert!(spec.endpoints[0].operation_id.is_none());
    }

    #[test]
    fn parse_empty_openapi() {
        let yaml = r#"
openapi: "3.0.0"
info:
  title: Empty API
  version: "1.0"
"#;
        let spec = parse_openapi_yaml(yaml).unwrap();
        assert!(spec.endpoints.is_empty());
        assert!(spec.servers.is_empty());
    }
}
