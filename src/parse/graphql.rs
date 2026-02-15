use std::collections::HashMap;
use std::path::Path;

/// Parsed GraphQL schema: maps type names to their field names.
#[derive(Debug, Clone, Default)]
pub struct GraphqlSchema {
    /// Query type fields (operation names)
    pub queries: Vec<String>,
    /// Mutation type fields (operation names)
    pub mutations: Vec<String>,
    /// Subscription type fields (operation names)
    pub subscriptions: Vec<String>,
    /// All type names defined in the schema
    pub type_names: Vec<String>,
}

/// Parse a GraphQL schema file and extract type/operation information.
pub fn parse_graphql_schema(path: &Path) -> Result<GraphqlSchema, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
    parse_graphql_schema_str(&content)
}

/// Parse a GraphQL schema from a string.
pub fn parse_graphql_schema_str(content: &str) -> Result<GraphqlSchema, String> {
    if content.trim().is_empty() {
        return Ok(GraphqlSchema::default());
    }

    let doc = graphql_parser::parse_schema::<&str>(content)
        .map_err(|e| format!("GraphQL parse error: {e}"))?;

    let mut schema = GraphqlSchema::default();

    // Collect schema-level operation type names (defaults: Query, Mutation, Subscription)
    let mut query_type = "Query".to_string();
    let mut mutation_type = "Mutation".to_string();
    let mut subscription_type = "Subscription".to_string();

    // Check for explicit schema definition
    for def in &doc.definitions {
        if let graphql_parser::schema::Definition::SchemaDefinition(sd) = def {
            if let Some(q) = &sd.query {
                query_type = q.to_string();
            }
            if let Some(m) = &sd.mutation {
                mutation_type = m.to_string();
            }
            if let Some(s) = &sd.subscription {
                subscription_type = s.to_string();
            }
        }
    }

    // Collect all type definitions and their fields
    let mut type_fields: HashMap<String, Vec<String>> = HashMap::new();

    for def in &doc.definitions {
        if let graphql_parser::schema::Definition::TypeDefinition(td) = def {
            match td {
                graphql_parser::schema::TypeDefinition::Object(obj) => {
                    let name = obj.name.to_string();
                    let fields: Vec<String> =
                        obj.fields.iter().map(|f| f.name.to_string()).collect();
                    schema.type_names.push(name.clone());
                    type_fields.insert(name, fields);
                }
                graphql_parser::schema::TypeDefinition::Interface(iface) => {
                    schema.type_names.push(iface.name.to_string());
                }
                graphql_parser::schema::TypeDefinition::Enum(en) => {
                    schema.type_names.push(en.name.to_string());
                }
                graphql_parser::schema::TypeDefinition::InputObject(input) => {
                    schema.type_names.push(input.name.to_string());
                }
                graphql_parser::schema::TypeDefinition::Union(u) => {
                    schema.type_names.push(u.name.to_string());
                }
                graphql_parser::schema::TypeDefinition::Scalar(s) => {
                    schema.type_names.push(s.name.to_string());
                }
            }
        }
    }

    // Extract operation fields from well-known root types
    if let Some(fields) = type_fields.get(&query_type) {
        schema.queries = fields.clone();
    }
    if let Some(fields) = type_fields.get(&mutation_type) {
        schema.mutations = fields.clone();
    }
    if let Some(fields) = type_fields.get(&subscription_type) {
        schema.subscriptions = fields.clone();
    }

    Ok(schema)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_schema() {
        let schema_str = r#"
            type Query {
                getUser(id: ID!): User
                listUsers: [User!]!
            }

            type Mutation {
                createUser(name: String!): User
            }

            type User {
                id: ID!
                name: String!
                email: String
            }
        "#;

        let schema = parse_graphql_schema_str(schema_str).unwrap();
        assert_eq!(schema.queries, vec!["getUser", "listUsers"]);
        assert_eq!(schema.mutations, vec!["createUser"]);
        assert!(schema.subscriptions.is_empty());
        assert!(schema.type_names.contains(&"User".to_string()));
        assert!(schema.type_names.contains(&"Query".to_string()));
        assert!(schema.type_names.contains(&"Mutation".to_string()));
    }

    #[test]
    fn parse_schema_with_custom_root_types() {
        let schema_str = r#"
            schema {
                query: RootQuery
                mutation: RootMutation
            }

            type RootQuery {
                hello: String
            }

            type RootMutation {
                setGreeting(msg: String!): String
            }
        "#;

        let schema = parse_graphql_schema_str(schema_str).unwrap();
        assert_eq!(schema.queries, vec!["hello"]);
        assert_eq!(schema.mutations, vec!["setGreeting"]);
    }

    #[test]
    fn parse_empty_schema() {
        let schema = parse_graphql_schema_str("").unwrap();
        assert!(schema.queries.is_empty());
        assert!(schema.mutations.is_empty());
        assert!(schema.type_names.is_empty());
    }

    #[test]
    fn parse_schema_with_enums_and_interfaces() {
        let schema_str = r#"
            enum Status {
                ACTIVE
                INACTIVE
            }

            interface Node {
                id: ID!
            }

            type Query {
                node(id: ID!): Node
            }
        "#;

        let schema = parse_graphql_schema_str(schema_str).unwrap();
        assert!(schema.type_names.contains(&"Status".to_string()));
        assert!(schema.type_names.contains(&"Node".to_string()));
        assert_eq!(schema.queries, vec!["node"]);
    }
}
