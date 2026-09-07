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
    pub const IO_ERROR: &str = "BRUNO_IO_ERROR";

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

use crate::request::RequestFile;

/// Source format of a Bruno request file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrunoFormat {
    /// Legacy `.bru` markup.
    Bru,
    /// OpenCollection YAML (`.yml`).
    Yml,
}

impl BrunoFormat {
    pub fn extension(self) -> &'static str {
        match self {
            BrunoFormat::Bru => "bru",
            BrunoFormat::Yml => "yml",
        }
    }
}

/// Whether the source request is plain HTTP or a GraphQL request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrunoRequestKind {
    Http,
    GraphQl,
}

/// One successfully mapped request, ready to be written as native TOML.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedRequest {
    /// Path of the source file relative to the collection root.
    pub source_path: String,
    pub format: BrunoFormat,
    pub kind: BrunoRequestKind,
    /// Native request produced by the mapping; never mutated by the report.
    pub request: RequestFile,
    pub warnings: Vec<BrunoImportWarning>,
}

impl ImportedRequest {
    pub fn new(
        source_path: impl Into<String>,
        format: BrunoFormat,
        kind: BrunoRequestKind,
        request: RequestFile,
        warnings: Vec<BrunoImportWarning>,
    ) -> Self {
        Self {
            source_path: source_path.into(),
            format,
            kind,
            request,
            warnings,
        }
    }
}

/// Structured warning attached to an imported request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrunoImportWarning {
    pub code: &'static str,
    pub message: String,
}

impl BrunoImportWarning {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// A request that was not imported and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrunoSkippedRequest {
    pub source_path: String,
    pub format: BrunoFormat,
    pub code: &'static str,
    pub detail: String,
}

impl BrunoSkippedRequest {
    pub fn new(
        source_path: impl Into<String>,
        format: BrunoFormat,
        code: &'static str,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            source_path: source_path.into(),
            format,
            code,
            detail: detail.into(),
        }
    }
}

/// A collection-level or parse-level error with a clear source path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrunoImportError {
    pub source_path: String,
    pub code: &'static str,
    pub message: String,
}

impl BrunoImportError {
    pub fn new(
        source_path: impl Into<String>,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            source_path: source_path.into(),
            code,
            message: message.into(),
        }
    }
}

/// Result of importing one Bruno collection.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BrunoImportReport {
    pub imported: Vec<ImportedRequest>,
    pub skipped: Vec<BrunoSkippedRequest>,
    pub errors: Vec<BrunoImportError>,
}

impl BrunoImportReport {
    pub fn new(
        imported: Vec<ImportedRequest>,
        skipped: Vec<BrunoSkippedRequest>,
        errors: Vec<BrunoImportError>,
    ) -> Self {
        Self {
            imported,
            skipped,
            errors,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        diagnostics, BrunoFormat, BrunoImportError, BrunoImportReport, BrunoImportWarning,
        BrunoRequestKind, BrunoSkippedRequest, ImportedRequest, RequestFile,
        CREDENTIAL_PLACEHOLDER,
    };
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
            IO_ERROR,
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

    fn sample_request() -> RequestFile {
        use crate::request::{
            RequestFileBody, RequestFileBodyType, RequestFileHeader, RequestFileHttp,
        };

        RequestFile {
            meta: Default::default(),
            http: RequestFileHttp {
                method: "POST".to_string(),
                url: "https://api.example.com/graphql".to_string(),
                params: Vec::new(),
                headers: vec![RequestFileHeader {
                    name: "Content-Type".to_string(),
                    value: "application/json".to_string(),
                    disabled: false,
                }],
                body: Some(RequestFileBody {
                    r#type: RequestFileBodyType::Json,
                    data: r#"{"query":"query { __typename }"}"#.to_string(),
                }),
            },
        }
    }

    #[test]
    fn imported_request_holds_native_request_and_path() {
        let request = sample_request();
        let imported = ImportedRequest::new(
            "importable/graphql-query.bru",
            BrunoFormat::Bru,
            BrunoRequestKind::GraphQl,
            request.clone(),
            vec![BrunoImportWarning::new(
                diagnostics::WARNING_CONTENT_TYPE_ADDED,
                "added Content-Type: application/json",
            )],
        );

        assert_eq!(imported.source_path, "importable/graphql-query.bru");
        assert_eq!(imported.format, BrunoFormat::Bru);
        assert_eq!(imported.kind, BrunoRequestKind::GraphQl);
        assert_eq!(imported.request, request);
        assert_eq!(imported.warnings.len(), 1);
        assert_eq!(
            imported.warnings[0].code,
            diagnostics::WARNING_CONTENT_TYPE_ADDED
        );
    }

    #[test]
    fn report_aggregates_imported_skipped_and_errors() {
        let imported = ImportedRequest::new(
            "importable/http-get.yml",
            BrunoFormat::Yml,
            BrunoRequestKind::Http,
            sample_request(),
            Vec::new(),
        );
        let skipped = BrunoSkippedRequest::new(
            "skipped/http-basic-auth.bru",
            BrunoFormat::Bru,
            diagnostics::UNSUPPORTED_AUTH,
            "basic auth is not supported on MVP",
        );
        let error = BrunoImportError::new(
            "corrupt.bru",
            diagnostics::MALFORMED_SYNTAX,
            "unbalanced braces",
        );
        let report = BrunoImportReport::new(vec![imported], vec![skipped], vec![error]);

        assert_eq!(report.imported.len(), 1);
        assert_eq!(report.imported[0].source_path, "importable/http-get.yml");
        assert_eq!(report.skipped.len(), 1);
        assert_eq!(report.skipped[0].source_path, "skipped/http-basic-auth.bru");
        assert_eq!(report.skipped[0].code, diagnostics::UNSUPPORTED_AUTH);
        assert_eq!(report.errors.len(), 1);
        assert_eq!(report.errors[0].source_path, "corrupt.bru");
        assert_eq!(report.errors[0].code, diagnostics::MALFORMED_SYNTAX);
    }

    #[test]
    fn model_codes_are_known_diagnostics() {
        let known = all_diagnostic_codes();
        let warning = BrunoImportWarning::new(diagnostics::WARNING_SETTINGS_DROPPED, "settings");
        let skipped = BrunoSkippedRequest::new(
            "a.bru",
            BrunoFormat::Bru,
            diagnostics::DYNAMIC_VALUE,
            "dynamic",
        );
        let error = BrunoImportError::new("a.yml", diagnostics::MIXED_FORMATS, "mixed");
        let report = BrunoImportReport::new(vec![], vec![], vec![]);
        assert!(
            report.imported.is_empty() && report.skipped.is_empty() && report.errors.is_empty()
        );

        for (label, code, message) in [
            ("warning", warning.code, warning.message),
            ("skipped", skipped.code, skipped.detail),
            ("error", error.code, error.message),
        ] {
            assert!(
                known.contains(&code),
                "{label} references unknown diagnostic code '{code}'"
            );
            assert!(!message.is_empty(), "{label} must carry a message");
        }
    }

    #[test]
    fn format_extension_mapping() {
        assert_eq!(BrunoFormat::Bru.extension(), "bru");
        assert_eq!(BrunoFormat::Yml.extension(), "yml");
    }

    #[test]
    fn request_kind_distinguishes_http_and_graphql() {
        assert_ne!(BrunoRequestKind::Http, BrunoRequestKind::GraphQl);
    }
}
