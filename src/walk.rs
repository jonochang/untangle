use crate::errors::Result;
use globset::{Glob, GlobSetBuilder};
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

/// Supported language for file discovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Python,
    Ruby,
    Go,
    Rust,
}

impl Language {
    /// File extensions for this language.
    pub fn extensions(&self) -> &[&str] {
        match self {
            Language::Python => &["py"],
            Language::Ruby => &["rb"],
            Language::Go => &["go"],
            Language::Rust => &["rs"],
        }
    }

    /// Default exclude patterns for this language.
    pub fn default_excludes(&self) -> Vec<String> {
        match self {
            Language::Go => vec!["*_test.go".to_string()],
            Language::Rust | Language::Python | Language::Ruby => vec![],
        }
    }
}

impl std::str::FromStr for Language {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "python" | "py" => Ok(Language::Python),
            "ruby" | "rb" => Ok(Language::Ruby),
            "go" => Ok(Language::Go),
            "rust" | "rs" => Ok(Language::Rust),
            _ => Err(format!("unsupported language: {s}")),
        }
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Language::Python => write!(f, "python"),
            Language::Ruby => write!(f, "ruby"),
            Language::Go => write!(f, "go"),
            Language::Rust => write!(f, "rust"),
        }
    }
}

/// Discover source files under `root` for the given language.
///
/// - Respects `.gitignore`
/// - Applies include/exclude glob patterns
/// - Excludes test files by default for Go
/// - Returns sorted paths for deterministic output
pub fn discover_files(
    root: &Path,
    lang: Language,
    include_patterns: &[String],
    exclude_patterns: &[String],
    include_tests: bool,
) -> Result<Vec<PathBuf>> {
    let extensions = lang.extensions();

    // Build exclude globset
    let mut exclude_builder = GlobSetBuilder::new();
    for pattern in exclude_patterns {
        exclude_builder.add(Glob::new(pattern)?);
    }
    if !include_tests {
        for pattern in lang.default_excludes() {
            exclude_builder.add(Glob::new(&pattern)?);
        }
    }
    let exclude_set = exclude_builder.build()?;

    // Build include globset (if any patterns specified)
    let include_set = if include_patterns.is_empty() {
        None
    } else {
        let mut builder = GlobSetBuilder::new();
        for pattern in include_patterns {
            builder.add(Glob::new(pattern)?);
        }
        Some(builder.build()?)
    };

    let walker = WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .build();

    let mut files = Vec::new();

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();

        // Only consider files
        if !path.is_file() {
            continue;
        }

        // Check extension
        let ext_match = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|ext| extensions.contains(&ext));

        if !ext_match {
            continue;
        }

        // Get relative path for glob matching
        let relative = path.strip_prefix(root).unwrap_or(path);

        // Apply exclude patterns
        if exclude_set.is_match(relative) || exclude_set.is_match(path) {
            continue;
        }
        // Also check just the filename for patterns like *_test.go
        if let Some(fname) = path.file_name() {
            if exclude_set.is_match(Path::new(fname)) {
                continue;
            }
        }

        // Apply include patterns (if any)
        if let Some(ref include) = include_set {
            if !include.is_match(relative) && !include.is_match(path) {
                continue;
            }
        }

        files.push(path.to_path_buf());
    }

    // Sort for deterministic output
    files.sort();

    Ok(files)
}

/// Auto-detect language by counting file extensions.
pub fn detect_language(root: &Path) -> Option<Language> {
    let walker = WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .build();

    let mut py_count = 0usize;
    let mut rb_count = 0usize;
    let mut go_count = 0usize;
    let mut rs_count = 0usize;

    for entry in walker.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        match path.extension().and_then(|e| e.to_str()) {
            Some("py") => py_count += 1,
            Some("rb") => rb_count += 1,
            Some("go") => go_count += 1,
            Some("rs") => rs_count += 1,
            _ => {}
        }
    }

    let max = py_count.max(rb_count).max(go_count).max(rs_count);
    if max == 0 {
        return None;
    }

    if max == py_count {
        Some(Language::Python)
    } else if max == go_count {
        Some(Language::Go)
    } else if max == rs_count {
        Some(Language::Rust)
    } else {
        Some(Language::Ruby)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_from_str() {
        assert_eq!("python".parse::<Language>().unwrap(), Language::Python);
        assert_eq!("py".parse::<Language>().unwrap(), Language::Python);
        assert_eq!("ruby".parse::<Language>().unwrap(), Language::Ruby);
        assert_eq!("go".parse::<Language>().unwrap(), Language::Go);
        assert_eq!("rust".parse::<Language>().unwrap(), Language::Rust);
        assert_eq!("rs".parse::<Language>().unwrap(), Language::Rust);
        assert!("java".parse::<Language>().is_err());
    }

    #[test]
    fn go_default_excludes_test_files() {
        let excludes = Language::Go.default_excludes();
        assert!(excludes.contains(&"*_test.go".to_string()));
    }
}
