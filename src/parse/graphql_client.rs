use crate::parse::common::SourceLocation;
use std::path::Path;

/// A detected GraphQL client usage in source code.
#[derive(Debug, Clone)]
pub struct GraphqlClientUsage {
    /// The operation name extracted (e.g., "GetUser", "CreatePost")
    pub operation_name: Option<String>,
    /// The operation type if detectable (query, mutation, subscription)
    pub operation_type: Option<GraphqlOperationType>,
    /// The raw query string if extractable
    pub raw_query: Option<String>,
    /// Where in the source code this usage was found
    pub location: SourceLocation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphqlOperationType {
    Query,
    Mutation,
    Subscription,
}

/// Detect GraphQL client usage patterns in source code.
///
/// This uses regex-like string scanning rather than tree-sitter for simplicity,
/// since GraphQL queries are typically embedded as string literals.
pub fn detect_graphql_usage(source: &[u8], file_path: &Path) -> Vec<GraphqlClientUsage> {
    let source_str = match std::str::from_utf8(source) {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    let mut usages = Vec::new();

    for (line_idx, line) in source_str.lines().enumerate() {
        let line_num = line_idx + 1;
        let trimmed = line.trim();

        // Detect GraphQL operation keywords in string literals
        if let Some(usage) = detect_operation_in_line(trimmed, file_path, line_num) {
            usages.push(usage);
        }
    }

    usages
}

fn detect_operation_in_line(
    line: &str,
    file_path: &Path,
    line_num: usize,
) -> Option<GraphqlClientUsage> {
    // Look for GraphQL operation patterns: query/mutation/subscription followed by name
    // These appear in string literals, template strings, heredocs, or on their own line
    // inside multi-line strings.
    let patterns = [
        ("query ", GraphqlOperationType::Query),
        ("mutation ", GraphqlOperationType::Mutation),
        ("subscription ", GraphqlOperationType::Subscription),
    ];

    for (keyword, op_type) in &patterns {
        // Find the keyword in the line (case-sensitive since GraphQL is case-sensitive)
        if let Some(pos) = line.find(keyword) {
            let after = &line[pos + keyword.len()..];
            let name = extract_operation_name(after);

            // Accept if:
            // 1. The line looks like it's in a string context (quotes, backticks, gql())
            // 2. OR the line has a GraphQL operation with a name followed by { or (
            //    (this handles multi-line strings where the operation is on its own line)
            let has_graphql_syntax = after.trim_start().contains('{')
                || after.trim_start().contains('(')
                || after.trim_start().starts_with('{');

            if looks_like_graphql_string(line) || (name.is_some() && has_graphql_syntax) {
                return Some(GraphqlClientUsage {
                    operation_name: name,
                    operation_type: Some(*op_type),
                    raw_query: None,
                    location: SourceLocation {
                        file: file_path.to_path_buf(),
                        line: line_num,
                        column: None,
                    },
                });
            }
        }
    }

    None
}

/// Extract an operation name from text after the operation keyword.
/// E.g., from "GetUser($id: ID!) {" extracts "GetUser"
fn extract_operation_name(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.starts_with('{') {
        return None; // Anonymous operation
    }

    let name: String = trimmed
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();

    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// Check if a line looks like it contains a GraphQL string literal.
fn looks_like_graphql_string(line: &str) -> bool {
    // Common patterns:
    // - String quotes: ", ', `
    // - Python triple quotes: """, '''
    // - Ruby heredoc: <<~GRAPHQL, <<-GRAPHQL
    // - Go raw string: `query ...`
    // - gql() function call
    // - graphql tag: graphql`...`
    line.contains('"')
        || line.contains('\'')
        || line.contains('`')
        || line.contains("gql(")
        || line.contains("gql`")
        || line.contains("graphql(")
        || line.contains("graphql`")
        || line.contains("GRAPHQL")
        || line.contains("GraphQL")
}

/// Match detected client usages against known schema operations.
/// Returns tuples of (usage, matched_service_name).
pub fn match_usages_to_schemas(
    usages: &[GraphqlClientUsage],
    schemas: &[(String, super::graphql::GraphqlSchema)],
) -> Vec<(GraphqlClientUsage, String)> {
    let mut matches = Vec::new();

    for usage in usages {
        if let Some(ref op_name) = usage.operation_name {
            for (service_name, schema) in schemas {
                let found = schema.queries.iter().any(|q| q == op_name)
                    || schema.mutations.iter().any(|m| m == op_name)
                    || schema.subscriptions.iter().any(|s| s == op_name);

                if found {
                    matches.push((usage.clone(), service_name.clone()));
                }
            }
        }
    }

    matches
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn detect_python_gql_query() {
        let source = br#"
result = client.execute(gql("""
    query GetUser($id: ID!) {
        user(id: $id) {
            name
            email
        }
    }
"""))
"#;
        let usages = detect_graphql_usage(source, Path::new("app.py"));
        assert!(!usages.is_empty());
        let usage = &usages[0];
        assert_eq!(usage.operation_name.as_deref(), Some("GetUser"));
        assert_eq!(usage.operation_type, Some(GraphqlOperationType::Query));
    }

    #[test]
    fn detect_go_graphql_query() {
        let source = br#"
req := graphql.NewRequest(`
    mutation CreatePost($title: String!) {
        createPost(title: $title) {
            id
        }
    }
`)
"#;
        let usages = detect_graphql_usage(source, Path::new("main.go"));
        assert!(!usages.is_empty());
        let usage = &usages[0];
        assert_eq!(usage.operation_name.as_deref(), Some("CreatePost"));
        assert_eq!(usage.operation_type, Some(GraphqlOperationType::Mutation));
    }

    #[test]
    fn detect_ruby_graphql_heredoc() {
        let source = br#"
QUERY = <<~GRAPHQL
    query ListPosts {
        posts {
            title
        }
    }
GRAPHQL
"#;
        let usages = detect_graphql_usage(source, Path::new("client.rb"));
        assert!(!usages.is_empty());
        let usage = &usages[0];
        assert_eq!(usage.operation_name.as_deref(), Some("ListPosts"));
        assert_eq!(usage.operation_type, Some(GraphqlOperationType::Query));
    }

    #[test]
    fn no_false_positives_on_plain_code() {
        let source = b"fn query_database(sql: &str) -> Vec<Row> { todo!() }";
        let usages = detect_graphql_usage(source, Path::new("db.rs"));
        assert!(usages.is_empty());
    }

    #[test]
    fn extract_operation_name_with_params() {
        assert_eq!(
            extract_operation_name("GetUser($id: ID!) {"),
            Some("GetUser".to_string())
        );
    }

    #[test]
    fn extract_operation_name_anonymous() {
        assert_eq!(extract_operation_name("{ user { name } }"), None);
    }

    #[test]
    fn match_usages_to_known_schemas() {
        use super::super::graphql::GraphqlSchema;

        let schema = GraphqlSchema {
            queries: vec!["getUser".to_string(), "listUsers".to_string()],
            mutations: vec!["createUser".to_string()],
            subscriptions: vec![],
            type_names: vec![],
        };

        let usages = vec![
            GraphqlClientUsage {
                operation_name: Some("getUser".to_string()),
                operation_type: Some(GraphqlOperationType::Query),
                raw_query: None,
                location: SourceLocation {
                    file: PathBuf::from("app.py"),
                    line: 10,
                    column: None,
                },
            },
            GraphqlClientUsage {
                operation_name: Some("unknownOp".to_string()),
                operation_type: Some(GraphqlOperationType::Query),
                raw_query: None,
                location: SourceLocation {
                    file: PathBuf::from("app.py"),
                    line: 20,
                    column: None,
                },
            },
        ];

        let schemas = vec![("user-api".to_string(), schema)];
        let matches = match_usages_to_schemas(&usages, &schemas);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].1, "user-api");
        assert_eq!(matches[0].0.operation_name.as_deref(), Some("getUser"));
    }
}
