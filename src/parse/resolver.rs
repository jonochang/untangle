use std::path::Path;

/// Check if a Go import path is a standard library package.
/// Stdlib packages have no dots in their import path.
pub fn is_go_stdlib(import_path: &str) -> bool {
    !import_path.contains('.')
}

/// Convert a CamelCase constant to snake_case (for Ruby Zeitwerk convention).
pub fn camel_to_snake(name: &str) -> String {
    let mut result = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                // Check if previous char is lowercase, or next char is lowercase (e.g., HTMLParser -> html_parser)
                let prev_lower = name.chars().nth(i - 1).is_some_and(|p| p.is_lowercase());
                let next_lower = name.chars().nth(i + 1).is_some_and(|n| n.is_lowercase());
                if prev_lower || (i > 1 && next_lower) {
                    result.push('_');
                }
            }
            result.push(c.to_lowercase().next().unwrap());
        } else {
            result.push(c);
        }
    }
    result
}

/// Check if a file is a Python package (directory with __init__.py).
pub fn is_python_package(dir: &Path) -> bool {
    dir.join("__init__.py").exists()
}

/// Check if a Go file is a test file.
pub fn is_go_test_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|name| name.ends_with("_test.go"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn go_stdlib_detection() {
        assert!(is_go_stdlib("fmt"));
        assert!(is_go_stdlib("net/http"));
        assert!(!is_go_stdlib("github.com/user/repo"));
    }

    #[test]
    fn camel_case_conversion() {
        assert_eq!(camel_to_snake("UserController"), "user_controller");
        assert_eq!(camel_to_snake("HTMLParser"), "html_parser");
        assert_eq!(camel_to_snake("Foo"), "foo");
        assert_eq!(camel_to_snake("FooBar"), "foo_bar");
    }

    #[test]
    fn go_test_file_detection() {
        assert!(is_go_test_file(Path::new("foo_test.go")));
        assert!(!is_go_test_file(Path::new("foo.go")));
        assert!(!is_go_test_file(Path::new("test.go")));
    }
}
