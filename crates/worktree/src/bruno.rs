//! Bruno collection import contract (BRUNO-01).
//!
//! This module owns the stable diagnostic codes shared by every Bruno importer
//! and the synthetic fixture corpus that pins the mapping contract. The tests
//! in this module assert that the corpus under `tests/fixtures/bruno`
//! classifies into exactly the three contract states (`importable`,
//! `importable_with_warnings`, `skipped`) and that credential values never
//! leak real secrets.

pub mod diagnostics {
    //! Stable diagnostic codes emitted by Bruno importers.
    //!
    //! Codes are contract, not implementation detail: changing one requires
    //! updating the fixture manifest tests in this module.

    // Discovery errors.
    pub const MIXED_FORMATS: &str = "BRUNO_MIXED_FORMATS";
    pub const NO_MANIFEST: &str = "BRUNO_NO_MANIFEST";

    // Request-level skip reasons.
    pub const UNRECOGNIZED_TYPE: &str = "BRUNO_UNRECOGNIZED_TYPE";
    pub const UNSUPPORTED_PROTOCOL: &str = "BRUNO_UNSUPPORTED_PROTOCOL";
    pub const UNSUPPORTED_AUTH: &str = "BRUNO_UNSUPPORTED_AUTH";
    pub const UNSUPPORTED_BODY: &str = "BRUNO_UNSUPPORTED_BODY";
    pub const UNSUPPORTED_PATH_PARAMS: &str = "BRUNO_UNSUPPORTED_PATH_PARAMS";
    pub const UNSUPPORTED_RUNTIME: &str = "BRUNO_UNSUPPORTED_RUNTIME";
    pub const UNSUPPORTED_ASSERTIONS: &str = "BRUNO_UNSUPPORTED_ASSERTIONS";
    pub const DYNAMIC_VALUE: &str = "BRUNO_DYNAMIC_VALUE";
    pub const MALFORMED_SYNTAX: &str = "BRUNO_MALFORMED_SYNTAX";

    // GraphQL-specific skip reasons.
    pub const GQL_METHOD_UNSUPPORTED: &str = "BRUNO_GQL_METHOD_UNSUPPORTED";
    pub const GQL_QUERY_EMPTY: &str = "BRUNO_GQL_QUERY_EMPTY";
    pub const GQL_VARIABLES_INVALID: &str = "BRUNO_GQL_VARIABLES_INVALID";
    pub const GQL_DYNAMIC_VALUE: &str = "BRUNO_GQL_DYNAMIC_VALUE";

    // Importable-with-warning reasons.
    pub const WARNING_CONTENT_TYPE_ADDED: &str = "BRUNO_WARNING_CONTENT_TYPE_ADDED";
    pub const WARNING_METADATA_DROPPED: &str = "BRUNO_WARNING_METADATA_DROPPED";
    pub const WARNING_SETTINGS_DROPPED: &str = "BRUNO_WARNING_SETTINGS_DROPPED";
}

/// Placeholder that every credential value in the fixture corpus must use.
pub const CREDENTIAL_PLACEHOLDER: &str = "__REDACTED_TEST_SECRET__";

#[cfg(test)]
mod tests {
    use super::diagnostics;
    use super::CREDENTIAL_PLACEHOLDER;
    use std::path::{Path, PathBuf};

    const FIXTURE_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/bruno");

    #[derive(Debug, PartialEq, Eq)]
    enum Status {
        Importable,
        ImportableWithWarnings(&'static [&'static str]),
        Skipped(&'static [&'static str]),
    }

    impl Status {
        fn codes(&self) -> &'static [&'static str] {
            match self {
                Status::Importable => &[],
                Status::ImportableWithWarnings(codes) => codes,
                Status::Skipped(codes) => codes,
            }
        }
    }

    /// Relative fixture path -> expected classification. Every file under the
    /// fixture root must appear here exactly once.
    const MANIFEST: &[(&str, Status)] = &[
        // importable
        ("importable/http-get.bru", Status::Importable),
        ("importable/http-get.yml", Status::Importable),
        ("importable/http-post-json.bru", Status::Importable),
        ("importable/http-post-json.yml", Status::Importable),
        ("importable/http-body-text.bru", Status::Importable),
        ("importable/http-body-xml.bru", Status::Importable),
        ("importable/http-disabled-items.bru", Status::Importable),
        ("importable/http-disabled-items.yml", Status::Importable),
        ("importable/graphql-query.bru", Status::Importable),
        ("importable/graphql-query.yml", Status::Importable),
        ("importable/graphql-mutation.bru", Status::Importable),
        ("importable/graphql-mutation.yml", Status::Importable),
        ("importable/graphql-variables.bru", Status::Importable),
        ("importable/graphql-variables.yml", Status::Importable),
        // importable_with_warnings
        (
            "importable_with_warnings/graphql-no-content-type.bru",
            Status::ImportableWithWarnings(&[diagnostics::WARNING_CONTENT_TYPE_ADDED]),
        ),
        (
            "importable_with_warnings/graphql-no-content-type.yml",
            Status::ImportableWithWarnings(&[diagnostics::WARNING_CONTENT_TYPE_ADDED]),
        ),
        (
            "importable_with_warnings/http-metadata-dropped.bru",
            Status::ImportableWithWarnings(&[diagnostics::WARNING_METADATA_DROPPED]),
        ),
        (
            "importable_with_warnings/http-metadata-dropped.yml",
            Status::ImportableWithWarnings(&[diagnostics::WARNING_METADATA_DROPPED]),
        ),
        (
            "importable_with_warnings/http-settings-dropped.yml",
            Status::ImportableWithWarnings(&[diagnostics::WARNING_SETTINGS_DROPPED]),
        ),
        // skipped
        (
            "skipped/http-dynamic-url.bru",
            Status::Skipped(&[diagnostics::DYNAMIC_VALUE]),
        ),
        (
            "skipped/http-dynamic-url.yml",
            Status::Skipped(&[diagnostics::DYNAMIC_VALUE]),
        ),
        (
            "skipped/http-basic-auth.bru",
            Status::Skipped(&[diagnostics::UNSUPPORTED_AUTH]),
        ),
        (
            "skipped/http-basic-auth.yml",
            Status::Skipped(&[diagnostics::UNSUPPORTED_AUTH]),
        ),
        (
            "skipped/http-bearer-auth.bru",
            Status::Skipped(&[diagnostics::UNSUPPORTED_AUTH]),
        ),
        (
            "skipped/http-bearer-auth.yml",
            Status::Skipped(&[diagnostics::UNSUPPORTED_AUTH]),
        ),
        (
            "skipped/http-oauth2-auth.bru",
            Status::Skipped(&[diagnostics::UNSUPPORTED_AUTH]),
        ),
        (
            "skipped/http-path-params.bru",
            Status::Skipped(&[diagnostics::UNSUPPORTED_PATH_PARAMS]),
        ),
        (
            "skipped/http-path-params.yml",
            Status::Skipped(&[diagnostics::UNSUPPORTED_PATH_PARAMS]),
        ),
        (
            "skipped/http-form-urlencoded.bru",
            Status::Skipped(&[diagnostics::UNSUPPORTED_BODY]),
        ),
        (
            "skipped/http-form-urlencoded.yml",
            Status::Skipped(&[diagnostics::UNSUPPORTED_BODY]),
        ),
        (
            "skipped/http-multipart-form.bru",
            Status::Skipped(&[diagnostics::UNSUPPORTED_BODY]),
        ),
        (
            "skipped/http-file-body.bru",
            Status::Skipped(&[diagnostics::UNSUPPORTED_BODY]),
        ),
        (
            "skipped/http-sparql-body.bru",
            Status::Skipped(&[diagnostics::UNSUPPORTED_BODY]),
        ),
        (
            "skipped/http-sparql-body.yml",
            Status::Skipped(&[diagnostics::UNSUPPORTED_BODY]),
        ),
        (
            "skipped/http-pre-request-script.bru",
            Status::Skipped(&[diagnostics::UNSUPPORTED_RUNTIME]),
        ),
        (
            "skipped/http-pre-request-script.yml",
            Status::Skipped(&[diagnostics::UNSUPPORTED_RUNTIME]),
        ),
        (
            "skipped/http-assertions.bru",
            Status::Skipped(&[diagnostics::UNSUPPORTED_ASSERTIONS]),
        ),
        (
            "skipped/http-assertions.yml",
            Status::Skipped(&[diagnostics::UNSUPPORTED_ASSERTIONS]),
        ),
        (
            "skipped/graphql-get.bru",
            Status::Skipped(&[diagnostics::GQL_METHOD_UNSUPPORTED]),
        ),
        (
            "skipped/graphql-get.yml",
            Status::Skipped(&[diagnostics::GQL_METHOD_UNSUPPORTED]),
        ),
        (
            "skipped/graphql-empty-query.bru",
            Status::Skipped(&[diagnostics::GQL_QUERY_EMPTY]),
        ),
        (
            "skipped/graphql-empty-query.yml",
            Status::Skipped(&[diagnostics::GQL_QUERY_EMPTY]),
        ),
        (
            "skipped/graphql-invalid-variables.bru",
            Status::Skipped(&[diagnostics::GQL_VARIABLES_INVALID]),
        ),
        (
            "skipped/graphql-invalid-variables.yml",
            Status::Skipped(&[diagnostics::GQL_VARIABLES_INVALID]),
        ),
        (
            "skipped/graphql-dynamic-variables.bru",
            Status::Skipped(&[diagnostics::GQL_DYNAMIC_VALUE]),
        ),
        (
            "skipped/graphql-dynamic-variables.yml",
            Status::Skipped(&[diagnostics::GQL_DYNAMIC_VALUE]),
        ),
        (
            "skipped/websocket-request.yml",
            Status::Skipped(&[diagnostics::UNSUPPORTED_PROTOCOL]),
        ),
        (
            "skipped/grpc-request.yml",
            Status::Skipped(&[diagnostics::UNSUPPORTED_PROTOCOL]),
        ),
    ];

    /// Key names that carry credentials. Normalized by lowercasing and
    /// dropping non-alphanumeric characters, so `secretAccessKey`,
    /// `client_secret`, and `apiKey` all match while config keys such as
    /// `token_source` or `access_token_url` do not.
    const CREDENTIAL_KEY_NAMES: &[&str] = &[
        "password",
        "token",
        "secret",
        "apikey",
        "authorization",
        "sessiontoken",
        "clientsecret",
        "secretaccesskey",
        "accesskeyid",
        "accesstoken",
        "refreshtoken",
        "idtoken",
    ];

    fn is_credential_key(key: &str) -> bool {
        let normalized: String = key
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .map(|c| c.to_ascii_lowercase())
            .collect();
        CREDENTIAL_KEY_NAMES.contains(&normalized.as_str())
    }

    fn yaml_scalar<'a>(line: &'a str, key: &str) -> Option<&'a str> {
        let value = line.strip_prefix(key)?.strip_prefix(':')?.trim();
        (!value.is_empty()).then_some(value)
    }

    /// Returns human-readable violations for every credential value that does
    /// not use the placeholder. Handles `.bru` `key: value` lines and YAML
    /// `- name: X` / `value: Y` list items.
    fn credential_violations(contents: &str) -> Vec<String> {
        let mut violations = Vec::new();
        let mut pending_name: Option<&str> = None;
        for (index, raw) in contents.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(rest) = line.strip_prefix('-').map(str::trim) {
                if let Some(name) = yaml_scalar(rest, "name") {
                    pending_name = Some(name);
                    continue;
                }
            }
            if let Some(value) = yaml_scalar(line, "value") {
                if let Some(name) = pending_name {
                    if is_credential_key(name) && !value.contains(CREDENTIAL_PLACEHOLDER) {
                        violations.push(format!(
                            "line {}: credential key '{name}' without placeholder",
                            index + 1
                        ));
                    }
                }
                continue;
            }
            if let Some((key, value)) = line.split_once(':') {
                let key = key
                    .trim()
                    .trim_start_matches('~')
                    .trim_matches('"')
                    .trim_matches('\'');
                if is_credential_key(key) && !value.trim().contains(CREDENTIAL_PLACEHOLDER) {
                    violations.push(format!(
                        "line {}: credential key '{key}' without placeholder",
                        index + 1
                    ));
                }
            }
        }
        violations
    }

    fn fixture_paths(root: &Path) -> Vec<String> {
        let mut paths = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).expect("fixture dir should be readable") {
                let entry = entry.expect("fixture entry should be readable");
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path
                    .extension()
                    .is_some_and(|ext| ext == "bru" || ext == "yml")
                {
                    paths.push(
                        path.strip_prefix(root)
                            .expect("fixture path should be under root")
                            .to_string_lossy()
                            .replace('\\', "/"),
                    );
                }
            }
        }
        paths.sort();
        paths
    }

    fn all_diagnostic_codes() -> Vec<&'static str> {
        use diagnostics::*;
        vec![
            MIXED_FORMATS,
            NO_MANIFEST,
            UNRECOGNIZED_TYPE,
            UNSUPPORTED_PROTOCOL,
            UNSUPPORTED_AUTH,
            UNSUPPORTED_BODY,
            UNSUPPORTED_PATH_PARAMS,
            UNSUPPORTED_RUNTIME,
            UNSUPPORTED_ASSERTIONS,
            DYNAMIC_VALUE,
            MALFORMED_SYNTAX,
            GQL_METHOD_UNSUPPORTED,
            GQL_QUERY_EMPTY,
            GQL_VARIABLES_INVALID,
            GQL_DYNAMIC_VALUE,
            WARNING_CONTENT_TYPE_ADDED,
            WARNING_METADATA_DROPPED,
            WARNING_SETTINGS_DROPPED,
        ]
    }

    #[test]
    fn fixture_manifest_matches_corpus() {
        let root = Path::new(FIXTURE_ROOT);
        let on_disk = fixture_paths(root);
        let mut in_manifest: Vec<&str> = MANIFEST.iter().map(|(path, _)| *path).collect();
        in_manifest.sort_unstable();

        assert_eq!(
            on_disk, in_manifest,
            "fixture corpus and manifest must stay in sync; every fixture needs a classification"
        );
    }

    #[test]
    fn fixture_corpus_covers_every_status_and_format() {
        let has_importable = MANIFEST.iter().any(|(_, s)| *s == Status::Importable);
        let has_warnings = MANIFEST
            .iter()
            .any(|(_, s)| matches!(s, Status::ImportableWithWarnings(_)));
        let has_skipped = MANIFEST
            .iter()
            .any(|(_, s)| matches!(s, Status::Skipped(_)));
        assert!(has_importable && has_warnings && has_skipped);

        let has_bru = MANIFEST.iter().any(|(path, _)| path.ends_with(".bru"));
        let has_yml = MANIFEST.iter().any(|(path, _)| path.ends_with(".yml"));
        assert!(has_bru && has_yml);
    }

    #[test]
    fn manifest_codes_are_known_diagnostics() {
        let known = all_diagnostic_codes();
        for (path, status) in MANIFEST {
            for code in status.codes() {
                assert!(
                    known.contains(code),
                    "fixture '{path}' references unknown diagnostic code '{code}'"
                );
            }
        }
    }

    #[test]
    fn diagnostic_codes_are_unique_and_prefixed() {
        let codes = all_diagnostic_codes();
        let mut sorted = codes.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(codes.len(), sorted.len(), "diagnostic codes must be unique");
        for code in codes {
            assert!(
                code.starts_with("BRUNO_"),
                "diagnostic code '{code}' must use the BRUNO_ prefix"
            );
        }
    }

    #[test]
    fn credential_values_use_placeholder() {
        let root = PathBuf::from(FIXTURE_ROOT);
        let mut failures = Vec::new();
        for path in fixture_paths(&root) {
            let contents = std::fs::read_to_string(root.join(&path))
                .expect("fixture should be readable UTF-8");
            for violation in credential_violations(&contents) {
                failures.push(format!("{path}: {violation}"));
            }
        }
        assert!(
            failures.is_empty(),
            "credential values must use {CREDENTIAL_PLACEHOLDER}:\n{}",
            failures.join("\n")
        );
    }

    #[test]
    fn fixtures_are_nonempty_utf8() {
        let root = PathBuf::from(FIXTURE_ROOT);
        for path in fixture_paths(&root) {
            let contents =
                std::fs::read_to_string(root.join(&path)).expect("fixture should be valid UTF-8");
            assert!(
                !contents.trim().is_empty(),
                "fixture '{path}' must not be empty"
            );
        }
    }
}
