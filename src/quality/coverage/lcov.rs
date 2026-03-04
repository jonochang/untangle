use crate::errors::{Result, UntangleError};
use crate::quality::coverage::CoverageMap;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Parse an LCOV file into a coverage map.
pub fn parse_lcov(path: &Path, project_root: &Path) -> Result<CoverageMap> {
    let content = std::fs::read_to_string(path)?;
    parse_lcov_str(&content, project_root)
}

fn normalize_paths(sf: &str, project_root: &Path) -> Vec<PathBuf> {
    let raw = PathBuf::from(sf);
    let abs = if raw.is_absolute() {
        raw.canonicalize().ok().or(Some(raw))
    } else {
        let joined = project_root.join(raw);
        joined.canonicalize().ok().or(Some(joined))
    };

    let mut paths = Vec::new();
    if let Some(abs_path) = abs {
        paths.push(abs_path.clone());
        if let Ok(rel) = abs_path.strip_prefix(project_root) {
            paths.push(rel.to_path_buf());
        }
    }
    paths
}

pub fn parse_lcov_str(content: &str, project_root: &Path) -> Result<CoverageMap> {
    let mut result: CoverageMap = HashMap::new();
    let mut current: Vec<PathBuf> = Vec::new();

    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("SF:") {
            current = normalize_paths(rest.trim(), project_root);
            continue;
        }

        if let Some(rest) = line.strip_prefix("DA:") {
            if current.is_empty() {
                continue;
            }
            let mut parts = rest.split(',');
            let line_no = parts.next().and_then(|p| p.trim().parse::<usize>().ok());
            let hits = parts.next().and_then(|p| p.trim().parse::<u64>().ok());
            if let (Some(ln), Some(h)) = (line_no, hits) {
                for file in &current {
                    result.entry(file.clone()).or_default().insert(ln, h);
                }
            }
            continue;
        }

        if line == "end_of_record" {
            current.clear();
        }
    }

    if result.is_empty() {
        return Err(UntangleError::Config(
            "No coverage data found in LCOV file".to_string(),
        ));
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn parse_lcov_basic() {
        let lcov = "SF:src/lib.rs\nDA:1,1\nDA:2,0\nend_of_record\n";
        let root = PathBuf::from("/tmp");
        let map = parse_lcov_str(lcov, &root).unwrap();
        assert_eq!(map.len(), 2);
        let lines = map.get(&PathBuf::from("/tmp/src/lib.rs")).unwrap();
        assert_eq!(lines.get(&1), Some(&1));
        assert_eq!(lines.get(&2), Some(&0));
    }
}
