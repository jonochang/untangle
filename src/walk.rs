use crate::errors::Result;
use globset::{Glob, GlobSetBuilder};
use ignore::WalkBuilder;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Supported language for file discovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
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

/// Auto-detect all languages present in the directory, sorted by file count descending.
pub fn detect_languages(root: &Path) -> Vec<Language> {
    let walker = WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .build();

    let mut counts: HashMap<Language, usize> = HashMap::new();

    for entry in walker.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if let Some(lang) = language_for_file(path) {
            *counts.entry(lang).or_insert(0) += 1;
        }
    }

    let mut langs: Vec<(Language, usize)> = counts.into_iter().collect();
    langs.sort_by(|a, b| b.1.cmp(&a.1));
    langs.into_iter().map(|(lang, _)| lang).collect()
}

/// Determine the language for a file based on its extension.
pub fn language_for_file(path: &Path) -> Option<Language> {
    let ext = path.extension().and_then(|e| e.to_str())?;
    match ext {
        "py" => Some(Language::Python),
        "rb" => Some(Language::Ruby),
        "go" => Some(Language::Go),
        "rs" => Some(Language::Rust),
        _ => None,
    }
}

/// Discover source files under `root` for multiple languages.
///
/// Walks the filesystem once and partitions files by language.
/// Same filtering logic as `discover_files()` but for all detected languages.
pub fn discover_files_multi(
    root: &Path,
    include_patterns: &[String],
    exclude_patterns: &[String],
    include_tests: bool,
) -> Result<HashMap<Language, Vec<PathBuf>>> {
    // Build exclude globset
    let mut exclude_builder = GlobSetBuilder::new();
    for pattern in exclude_patterns {
        exclude_builder.add(Glob::new(pattern)?);
    }
    let exclude_set = exclude_builder.build()?;

    // Language-specific exclude sets (e.g., Go test files)
    let go_test_exclude = if !include_tests {
        let mut builder = GlobSetBuilder::new();
        for pattern in Language::Go.default_excludes() {
            builder.add(Glob::new(&pattern)?);
        }
        builder.build().ok()
    } else {
        None
    };

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

    let mut files_by_lang: HashMap<Language, Vec<PathBuf>> = HashMap::new();

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        // Determine language from extension
        let lang = match language_for_file(path) {
            Some(l) => l,
            None => continue,
        };

        // Get relative path for glob matching
        let relative = path.strip_prefix(root).unwrap_or(path);

        // Apply exclude patterns
        if exclude_set.is_match(relative) || exclude_set.is_match(path) {
            continue;
        }
        if let Some(fname) = path.file_name() {
            if exclude_set.is_match(Path::new(fname)) {
                continue;
            }
        }

        // Apply language-specific test excludes (e.g., Go *_test.go)
        if lang == Language::Go {
            if let Some(ref go_exclude) = go_test_exclude {
                if go_exclude.is_match(relative)
                    || go_exclude.is_match(path)
                    || path
                        .file_name()
                        .map(|f| go_exclude.is_match(Path::new(f)))
                        .unwrap_or(false)
                {
                    continue;
                }
            }
        }

        // Apply include patterns (if any)
        if let Some(ref include) = include_set {
            if !include.is_match(relative) && !include.is_match(path) {
                continue;
            }
        }

        files_by_lang
            .entry(lang)
            .or_default()
            .push(path.to_path_buf());
    }

    // Sort each language's files for deterministic output
    for files in files_by_lang.values_mut() {
        files.sort();
    }

    Ok(files_by_lang)
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

    #[test]
    fn language_for_file_extensions() {
        assert_eq!(
            language_for_file(Path::new("foo.py")),
            Some(Language::Python)
        );
        assert_eq!(language_for_file(Path::new("bar.rb")), Some(Language::Ruby));
        assert_eq!(language_for_file(Path::new("baz.go")), Some(Language::Go));
        assert_eq!(language_for_file(Path::new("qux.rs")), Some(Language::Rust));
        assert_eq!(language_for_file(Path::new("readme.md")), None);
        assert_eq!(language_for_file(Path::new("noext")), None);
    }

    #[test]
    fn detect_languages_mixed_fixture() {
        let langs = detect_languages(Path::new("tests/fixtures/polyglot"));
        assert!(langs.contains(&Language::Go), "Should detect Go files");
        assert!(
            langs.contains(&Language::Python),
            "Should detect Python files"
        );
        assert!(langs.contains(&Language::Ruby), "Should detect Ruby files");
    }

    #[test]
    fn discover_files_multi_partitions() {
        let result =
            discover_files_multi(Path::new("tests/fixtures/polyglot"), &[], &[], false).unwrap();

        assert!(result.contains_key(&Language::Go), "Should have Go files");
        assert!(
            result.contains_key(&Language::Python),
            "Should have Python files"
        );
        assert!(
            result.contains_key(&Language::Ruby),
            "Should have Ruby files"
        );

        // Go files should be .go
        for f in result.get(&Language::Go).unwrap() {
            assert!(f.to_string_lossy().ends_with(".go"));
        }
        // Python files should be .py
        for f in result.get(&Language::Python).unwrap() {
            assert!(f.to_string_lossy().ends_with(".py"));
        }
        // Ruby files should be .rb
        for f in result.get(&Language::Ruby).unwrap() {
            assert!(f.to_string_lossy().ends_with(".rb"));
        }
    }
}
