use crate::errors::{Result, UntangleError};
use crate::quality::complexity::go::GoComplexity;
use crate::quality::complexity::python::PythonComplexity;
use crate::quality::complexity::ruby::RubyComplexity;
use crate::quality::complexity::rust::RustComplexity;
use crate::quality::complexity::ComplexityFrontend;
use crate::quality::FunctionInfo;
use crate::walk::{self, Language};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn frontend_for(lang: Language) -> Box<dyn ComplexityFrontend> {
    match lang {
        Language::Go => Box::new(GoComplexity),
        Language::Python => Box::new(PythonComplexity),
        Language::Ruby => Box::new(RubyComplexity),
        Language::Rust => Box::new(RustComplexity),
    }
}

pub fn discover_files_by_lang(
    root: &Path,
    lang: Option<Language>,
    include: &[String],
    exclude: &[String],
    include_tests: bool,
) -> Result<(HashMap<Language, Vec<PathBuf>>, Vec<Language>)> {
    let files_by_lang: HashMap<Language, Vec<PathBuf>> = match lang {
        Some(lang) => {
            let files = walk::discover_files(root, lang, include, exclude, include_tests)?;
            let mut map = HashMap::new();
            if !files.is_empty() {
                map.insert(lang, files);
            }
            map
        }
        None => walk::discover_files_multi(root, include, exclude, include_tests)?,
    };

    if files_by_lang.is_empty() {
        return Err(UntangleError::NoFiles {
            path: root.to_path_buf(),
        });
    }

    let mut langs: Vec<Language> = files_by_lang.keys().copied().collect();
    langs.sort_by(|a, b| a.to_string().cmp(&b.to_string()));

    Ok((files_by_lang, langs))
}

pub fn collect_functions(
    root: &Path,
    files_by_lang: &HashMap<Language, Vec<PathBuf>>,
) -> (Vec<FunctionInfo>, usize, Vec<String>) {
    let mut all_functions: Vec<FunctionInfo> = Vec::new();
    let mut files_parsed = 0usize;

    for (lang, files) in files_by_lang {
        let frontend = frontend_for(*lang);
        for file in files {
            let Ok(source) = std::fs::read(file) else {
                continue;
            };
            let relative = file.strip_prefix(root).unwrap_or(file).to_path_buf();
            let functions = frontend.extract_functions(&source, &relative);
            if !functions.is_empty() {
                all_functions.extend(functions);
            }
            files_parsed += 1;
        }
    }

    let languages: Vec<String> = files_by_lang.keys().map(|lang| lang.to_string()).collect();

    (all_functions, files_parsed, languages)
}
