use std::collections::{BTreeSet, HashMap};
use std::env;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
use unic_langid::LanguageIdentifier;

include!(concat!(env!("OUT_DIR"), "/i18n_bundle.rs"));

#[derive(Debug, Clone)]
pub struct I18n {
    supported: Vec<&'static str>,
    locales: HashMap<&'static str, HashMap<String, String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationIssue {
    pub locale: String,
    pub key: String,
    pub message: String,
}

impl fmt::Display for ValidationIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {}", self.locale, self.key, self.message)
    }
}

impl I18n {
    pub fn load() -> Result<Self, String> {
        let mut locales = HashMap::new();
        for (locale, raw) in EMBEDDED_LOCALES {
            let parsed: HashMap<String, String> = serde_json::from_str(raw)
                .map_err(|err| format!("failed to parse embedded locale {locale}: {err}"))?;
            locales.insert(*locale, parsed);
        }

        Ok(Self {
            supported: SUPPORTED_LOCALES.to_vec(),
            locales,
        })
    }

    pub fn supported(&self) -> &[&'static str] {
        &self.supported
    }

    pub fn select_locale(&self, cli_locale: Option<String>) -> String {
        select_locale(cli_locale, self.supported())
    }

    pub fn t(&self, locale: &str, key: &str) -> String {
        if let Some(value) = self.lookup(locale, key) {
            return value.to_string();
        }
        key.to_string()
    }

    pub fn tf(&self, locale: &str, key: &str, args: &[(&str, String)]) -> String {
        let mut rendered = self.t(locale, key);
        for (name, value) in args {
            let needle = format!("{{{name}}}");
            rendered = rendered.replace(&needle, value);
        }
        rendered
    }

    fn lookup<'a>(&'a self, locale: &str, key: &str) -> Option<&'a str> {
        let mut candidates = Vec::new();
        candidates.push(locale.to_string());
        if let Some(base) = base_language(locale)
            && base != locale
        {
            candidates.push(base);
        }
        candidates.push("en".to_string());

        for candidate in candidates {
            if let Some(map) = self.locales.get(candidate.as_str())
                && let Some(value) = map.get(key)
            {
                return Some(value.as_str());
            }
        }

        None
    }
}

pub fn detect_env_locale() -> Option<String> {
    for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(val) = env::var(key) {
            let trimmed = val.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

pub fn detect_system_locale() -> Option<String> {
    sys_locale::get_locale()
}

pub fn normalize_locale(raw: &str) -> Option<String> {
    let mut cleaned = raw.trim();
    if cleaned.is_empty() {
        return None;
    }
    if let Some((head, _)) = cleaned.split_once('.') {
        cleaned = head;
    }
    if let Some((head, _)) = cleaned.split_once('@') {
        cleaned = head;
    }
    let cleaned = cleaned.replace('_', "-");
    cleaned
        .parse::<LanguageIdentifier>()
        .ok()
        .map(|lid| lid.to_string())
}

pub fn base_language(tag: &str) -> Option<String> {
    tag.split('-').next().map(|s| s.to_ascii_lowercase())
}

pub fn select_locale(cli_locale: Option<String>, supported: &[&str]) -> String {
    fn resolve(candidate: &str, supported: &[&str]) -> Option<String> {
        let norm = normalize_locale(candidate)?;
        if supported.iter().any(|s| *s == norm) {
            return Some(norm);
        }
        let base = base_language(&norm)?;
        if supported.iter().any(|s| *s == base) {
            return Some(base);
        }
        None
    }

    if let Some(cli) = cli_locale.as_deref()
        && let Some(found) = resolve(cli, supported)
    {
        return found;
    }

    if let Some(env_loc) = detect_env_locale()
        && let Some(found) = resolve(&env_loc, supported)
    {
        return found;
    }

    if let Some(sys_loc) = detect_system_locale()
        && let Some(found) = resolve(&sys_loc, supported)
    {
        return found;
    }

    "en".to_string()
}

pub fn status_from_disk(root: impl AsRef<Path>) -> Result<StatusReport, String> {
    let root = root.as_ref();
    let locales = read_locale_list(root)?;
    let english = read_locale_map(root, "en")?;
    let mut missing_files = Vec::new();
    let mut extra_keys = Vec::new();
    let mut missing_keys = Vec::new();

    for locale in &locales {
        let path = root.join("i18n").join(format!("{locale}.json"));
        if !path.exists() {
            missing_files.push(locale.clone());
            continue;
        }
        let map = read_locale_map(root, locale)?;
        let english_keys: BTreeSet<_> = english.keys().cloned().collect();
        let locale_keys: BTreeSet<_> = map.keys().cloned().collect();

        for key in english_keys.difference(&locale_keys) {
            missing_keys.push((locale.clone(), key.clone()));
        }
        for key in locale_keys.difference(&english_keys) {
            extra_keys.push((locale.clone(), key.clone()));
        }
    }

    Ok(StatusReport {
        locale_count: locales.len(),
        missing_files,
        missing_keys,
        extra_keys,
    })
}

pub fn validate_from_disk(root: impl AsRef<Path>) -> Result<Vec<ValidationIssue>, String> {
    let root = root.as_ref();
    let locales = read_locale_list(root)?;
    let english = read_locale_map(root, "en")?;
    let mut issues = Vec::new();

    for locale in &locales {
        let path = root.join("i18n").join(format!("{locale}.json"));
        if !path.exists() {
            issues.push(ValidationIssue {
                locale: locale.clone(),
                key: "*".to_string(),
                message: "missing locale file".to_string(),
            });
            continue;
        }

        let map = read_locale_map(root, locale)?;
        for (key, en_value) in &english {
            match map.get(key) {
                Some(value) => {
                    if placeholder_tokens(value) != placeholder_tokens(en_value) {
                        issues.push(ValidationIssue {
                            locale: locale.clone(),
                            key: key.clone(),
                            message: "placeholder mismatch".to_string(),
                        });
                    }
                    if newline_count(value) != newline_count(en_value) {
                        issues.push(ValidationIssue {
                            locale: locale.clone(),
                            key: key.clone(),
                            message: "newline mismatch".to_string(),
                        });
                    }
                    if backtick_spans(value) != backtick_spans(en_value) {
                        issues.push(ValidationIssue {
                            locale: locale.clone(),
                            key: key.clone(),
                            message: "backtick span mismatch".to_string(),
                        });
                    }
                }
                None => issues.push(ValidationIssue {
                    locale: locale.clone(),
                    key: key.clone(),
                    message: "missing key".to_string(),
                }),
            }
        }
    }

    Ok(issues)
}

#[derive(Debug, Clone)]
pub struct StatusReport {
    pub locale_count: usize,
    pub missing_files: Vec<String>,
    pub missing_keys: Vec<(String, String)>,
    pub extra_keys: Vec<(String, String)>,
}

impl StatusReport {
    pub fn is_clean(&self) -> bool {
        self.missing_files.is_empty() && self.missing_keys.is_empty() && self.extra_keys.is_empty()
    }
}

fn read_locale_list(root: &Path) -> Result<Vec<String>, String> {
    let locales_path = root.join("i18n").join("locales.json");
    let raw = fs::read_to_string(&locales_path)
        .map_err(|err| format!("failed to read {}: {err}", locales_path.display()))?;
    serde_json::from_str(&raw)
        .map_err(|err| format!("failed to parse {}: {err}", locales_path.display()))
}

fn read_locale_map(root: &Path, locale: &str) -> Result<HashMap<String, String>, String> {
    let path = root.join("i18n").join(format!("{locale}.json"));
    let raw = fs::read_to_string(&path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    let value: Value = serde_json::from_str(&raw)
        .map_err(|err| format!("failed to parse {}: {err}", path.display()))?;
    match value {
        Value::Object(map) => map
            .into_iter()
            .map(|(key, value)| match value {
                Value::String(s) => Ok((key, s)),
                _ => Err(format!(
                    "{} contains non-string value for key {}",
                    path.display(),
                    key
                )),
            })
            .collect(),
        _ => Err(format!("{} must be a JSON object", path.display())),
    }
}

fn placeholder_tokens(value: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_placeholder = false;

    for ch in value.chars() {
        match (in_placeholder, ch) {
            (false, '{') => {
                in_placeholder = true;
                current.clear();
            }
            (true, '}') => {
                in_placeholder = false;
                tokens.push(current.clone());
                current.clear();
            }
            (true, ch) => current.push(ch),
            _ => {}
        }
    }

    tokens
}

fn newline_count(value: &str) -> usize {
    value.matches('\n').count()
}

fn backtick_spans(value: &str) -> Vec<String> {
    let parts: Vec<&str> = value.split('`').collect();
    if parts.len().is_multiple_of(2) {
        return vec!["<unmatched>".to_string()];
    }

    parts
        .iter()
        .enumerate()
        .filter(|(idx, _)| idx % 2 == 1)
        .map(|(_, part)| (*part).to_string())
        .collect()
}

pub fn repo_root() -> Result<PathBuf, String> {
    std::env::current_dir().map_err(|err| format!("failed to get current dir: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn normalizes_locale_variants() {
        assert_eq!(normalize_locale("en_US.UTF-8").as_deref(), Some("en-US"));
        assert_eq!(normalize_locale("de_DE@euro").as_deref(), Some("de-DE"));
        assert_eq!(normalize_locale("fr").as_deref(), Some("fr"));
    }

    #[test]
    fn selects_base_language_fallback() {
        let supported = ["en", "en-GB", "ja"];
        assert_eq!(
            select_locale(Some("en_US.UTF-8".to_string()), &supported),
            "en"
        );
        assert_eq!(
            select_locale(Some("en-GB".to_string()), &supported),
            "en-GB"
        );
        assert_eq!(select_locale(Some("ja-JP".to_string()), &supported), "ja");
    }

    #[test]
    fn preserves_placeholder_structure() {
        assert_eq!(
            placeholder_tokens("Hello {name} from {place}"),
            vec!["name", "place"]
        );
        assert_eq!(backtick_spans("Use `cargo test` now"), vec!["cargo test"]);
        assert_eq!(newline_count("a\nb\n"), 2);
    }
}
