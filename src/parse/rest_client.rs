use crate::parse::common::SourceLocation;
use std::path::Path;

/// A detected REST/HTTP client usage in source code.
#[derive(Debug, Clone)]
pub struct RestClientUsage {
    /// HTTP method (GET, POST, PUT, DELETE, etc.)
    pub method: Option<String>,
    /// URL or URL pattern found in the source
    pub url: String,
    /// Where in the source code this usage was found
    pub location: SourceLocation,
}

/// Detect REST/HTTP client usage patterns in source code.
///
/// Scans for common HTTP client library patterns across languages:
/// - Python: requests.get(), httpx.post(), urllib.request.urlopen()
/// - Go: http.Get(), http.Post(), http.NewRequest()
/// - Ruby: Net::HTTP.get(), HTTParty.get(), Faraday.get()
/// - Rust: reqwest::get(), Client::get()
pub fn detect_rest_usage(source: &[u8], file_path: &Path) -> Vec<RestClientUsage> {
    let source_str = match std::str::from_utf8(source) {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    let mut usages = Vec::new();

    for (line_idx, line) in source_str.lines().enumerate() {
        let line_num = line_idx + 1;
        let trimmed = line.trim();

        usages.extend(detect_http_calls_in_line(trimmed, file_path, line_num));
    }

    usages
}

fn detect_http_calls_in_line(
    line: &str,
    file_path: &Path,
    line_num: usize,
) -> Vec<RestClientUsage> {
    let mut results = Vec::new();

    // Common HTTP client patterns: library.method("url")
    let patterns: &[(&str, &str)] = &[
        // Python
        ("requests.get(", "GET"),
        ("requests.post(", "POST"),
        ("requests.put(", "PUT"),
        ("requests.delete(", "DELETE"),
        ("requests.patch(", "PATCH"),
        ("httpx.get(", "GET"),
        ("httpx.post(", "POST"),
        ("httpx.put(", "PUT"),
        ("httpx.delete(", "DELETE"),
        ("httpx.patch(", "PATCH"),
        // Go
        ("http.Get(", "GET"),
        ("http.Post(", "POST"),
        ("http.NewRequest(", ""),
        // Ruby
        ("HTTParty.get(", "GET"),
        ("HTTParty.post(", "POST"),
        ("HTTParty.put(", "PUT"),
        ("HTTParty.delete(", "DELETE"),
        ("Net::HTTP.get(", "GET"),
        ("Net::HTTP.post(", "POST"),
        ("Faraday.get(", "GET"),
        ("Faraday.post(", "POST"),
        // Rust
        ("reqwest::get(", "GET"),
        (".get(", "GET"),
        (".post(", "POST"),
        (".put(", "PUT"),
        (".delete(", "DELETE"),
        (".patch(", "PATCH"),
    ];

    for (pattern, method) in patterns {
        if let Some(pos) = line.find(pattern) {
            let after = &line[pos + pattern.len()..];
            if let Some(url) = extract_url_from_args(after) {
                // For http.NewRequest, extract method from first arg
                let actual_method = if pattern.contains("NewRequest") {
                    extract_http_method_from_args(after)
                } else if method.is_empty() {
                    None
                } else {
                    Some(method.to_string())
                };

                results.push(RestClientUsage {
                    method: actual_method,
                    url,
                    location: SourceLocation {
                        file: file_path.to_path_buf(),
                        line: line_num,
                        column: None,
                    },
                });
                break; // Only report one match per line
            }
        }
    }

    results
}

/// Extract a URL string from function arguments.
/// Looks for any quoted string argument that looks like a URL.
fn extract_url_from_args(args: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let mut search_from = 0;
        while search_from < args.len() {
            let remaining = &args[search_from..];
            if let Some(start) = remaining.find(quote) {
                let after_quote = &remaining[start + 1..];
                if let Some(end) = after_quote.find(quote) {
                    let candidate = &after_quote[..end];
                    // Check if it looks like a URL path or full URL
                    if candidate.starts_with("http://")
                        || candidate.starts_with("https://")
                        || candidate.starts_with('/')
                    {
                        return Some(candidate.to_string());
                    }
                    // Move past this quoted string and continue searching
                    search_from += start + 1 + end + 1;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }
    None
}

/// Extract the HTTP method from http.NewRequest("METHOD", "url", ...) style calls.
fn extract_http_method_from_args(args: &str) -> Option<String> {
    for quote in ['"', '\''] {
        if let Some(start) = args.find(quote) {
            let rest = &args[start + 1..];
            if let Some(end) = rest.find(quote) {
                let candidate = &rest[..end];
                let upper = candidate.to_uppercase();
                if ["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"]
                    .contains(&upper.as_str())
                {
                    return Some(upper);
                }
            }
        }
    }
    None
}

/// Match detected REST usages against known service base URLs and OpenAPI endpoints.
/// Returns tuples of (usage, matched_service_name, matched_endpoint_path).
pub fn match_usages_to_services(
    usages: &[RestClientUsage],
    services: &[(String, Vec<String>, Vec<super::openapi::OpenApiEndpoint>)],
) -> Vec<(RestClientUsage, String, Option<String>)> {
    let mut matches = Vec::new();

    for usage in usages {
        for (service_name, base_urls, endpoints) in services {
            // Check if URL matches any base_url prefix
            let base_match = base_urls.iter().any(|base| usage.url.starts_with(base));

            // Check if URL matches any endpoint path pattern
            let endpoint_match = endpoints.iter().find(|ep| {
                url_matches_pattern(&usage.url, &ep.path)
                    && (usage.method.is_none()
                        || usage
                            .method
                            .as_ref()
                            .is_some_and(|m| m.eq_ignore_ascii_case(&ep.method)))
            });

            if base_match || endpoint_match.is_some() {
                matches.push((
                    usage.clone(),
                    service_name.clone(),
                    endpoint_match.map(|ep| ep.path.clone()),
                ));
            }
        }
    }

    matches
}

/// Check if a URL matches an OpenAPI path pattern.
/// E.g., "/users/123" matches "/users/{id}"
fn url_matches_pattern(url: &str, pattern: &str) -> bool {
    let url_parts: Vec<&str> = url.split('/').collect();
    let pattern_parts: Vec<&str> = pattern.split('/').collect();

    if url_parts.len() != pattern_parts.len() {
        return false;
    }

    url_parts
        .iter()
        .zip(pattern_parts.iter())
        .all(|(u, p)| p.starts_with('{') && p.ends_with('}') || u == p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_python_requests() {
        let source = br#"
import requests

response = requests.get("https://api.example.com/users")
data = requests.post("https://api.example.com/users", json=payload)
"#;
        let usages = detect_rest_usage(source, Path::new("client.py"));
        assert_eq!(usages.len(), 2);
        assert_eq!(usages[0].method.as_deref(), Some("GET"));
        assert_eq!(usages[0].url, "https://api.example.com/users");
        assert_eq!(usages[1].method.as_deref(), Some("POST"));
    }

    #[test]
    fn detect_go_http_calls() {
        let source = br#"
resp, err := http.Get("https://api.example.com/items")
req, err := http.NewRequest("POST", "https://api.example.com/items", body)
"#;
        let usages = detect_rest_usage(source, Path::new("main.go"));
        assert_eq!(usages.len(), 2);
        assert_eq!(usages[0].method.as_deref(), Some("GET"));
        assert_eq!(usages[0].url, "https://api.example.com/items");
        assert_eq!(usages[1].method.as_deref(), Some("POST"));
    }

    #[test]
    fn detect_ruby_httparty() {
        let source = br#"
response = HTTParty.get("https://api.example.com/posts")
"#;
        let usages = detect_rest_usage(source, Path::new("client.rb"));
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].method.as_deref(), Some("GET"));
        assert_eq!(usages[0].url, "https://api.example.com/posts");
    }

    #[test]
    fn no_false_positives_on_dict_get() {
        // dict.get("key") should not match since "key" isn't a URL
        let source = br#"value = config.get("database_url")"#;
        let usages = detect_rest_usage(source, Path::new("config.py"));
        assert!(usages.is_empty());
    }

    #[test]
    fn url_pattern_matching() {
        assert!(url_matches_pattern("/users/123", "/users/{id}"));
        assert!(url_matches_pattern("/users", "/users"));
        assert!(!url_matches_pattern("/users/123/posts", "/users/{id}"));
        assert!(!url_matches_pattern("/items", "/users"));
    }

    #[test]
    fn match_usages_against_services() {
        use super::super::openapi::OpenApiEndpoint;

        let usages = vec![
            RestClientUsage {
                method: Some("GET".to_string()),
                url: "/api/v1/posts".to_string(),
                location: SourceLocation {
                    file: std::path::PathBuf::from("app.py"),
                    line: 10,
                    column: None,
                },
            },
            RestClientUsage {
                method: Some("GET".to_string()),
                url: "/api/v1/unknown".to_string(),
                location: SourceLocation {
                    file: std::path::PathBuf::from("app.py"),
                    line: 20,
                    column: None,
                },
            },
        ];

        let services = vec![(
            "post-api".to_string(),
            vec!["/api/v1/posts".to_string()],
            vec![OpenApiEndpoint {
                method: "GET".to_string(),
                path: "/api/v1/posts".to_string(),
                operation_id: Some("listPosts".to_string()),
            }],
        )];

        let matches = match_usages_to_services(&usages, &services);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].1, "post-api");
    }
}
