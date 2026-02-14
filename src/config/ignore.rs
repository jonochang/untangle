use std::path::Path;

/// Load `.untangleignore` by walking up from the given directory.
/// Parses gitignore-style patterns (skip blank lines and # comments).
pub fn load_untangleignore(start: &Path) -> Vec<String> {
    let mut dir = start.to_path_buf();
    loop {
        let ignore_path = dir.join(".untangleignore");
        if ignore_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&ignore_path) {
                return parse_ignore_patterns(&content);
            }
        }
        if !dir.pop() {
            break;
        }
    }
    Vec::new()
}

fn parse_ignore_patterns(content: &str) -> Vec<String> {
    content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_patterns() {
        let content = r#"
# This is a comment
vendor/**
node_modules/**

# Another comment
*.generated.go

build/
"#;
        let patterns = parse_ignore_patterns(content);
        assert_eq!(
            patterns,
            vec!["vendor/**", "node_modules/**", "*.generated.go", "build/"]
        );
    }

    #[test]
    fn empty_content() {
        let patterns = parse_ignore_patterns("");
        assert!(patterns.is_empty());
    }

    #[test]
    fn only_comments() {
        let patterns = parse_ignore_patterns("# comment\n# another");
        assert!(patterns.is_empty());
    }

    #[test]
    fn load_from_temp_dir() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(".untangleignore"), "vendor/**\nbuild/\n").unwrap();

        let patterns = load_untangleignore(tmp.path());
        assert_eq!(patterns, vec!["vendor/**", "build/"]);
    }

    #[test]
    fn no_ignore_file() {
        let tmp = tempfile::tempdir().unwrap();
        let patterns = load_untangleignore(tmp.path());
        assert!(patterns.is_empty());
    }
}
