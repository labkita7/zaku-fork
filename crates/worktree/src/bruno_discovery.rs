//! Bruno collection root discovery (BRUNO-03).
//!
//! Pure filesystem logic: validates a collection root by its manifest, walks
//! it for request files, and rejects unsafe or ambiguous inputs. Request
//! content is never parsed here; each file is classified by the parsers
//! (BRUNO-04/05).

use crate::bruno::{diagnostics, BrunoFormat, BrunoImportError};
use std::path::{Path, PathBuf};

/// A request file found under a validated Bruno collection root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredRequest {
    /// Path of the source file relative to the collection root.
    pub source_path: String,
    pub format: BrunoFormat,
}

/// A validated Bruno collection root and its request files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrunoCollectionDiscovery {
    /// Absolute path of the directory holding the manifest.
    pub root: PathBuf,
    /// The manifest file that validated the root.
    pub manifest: PathBuf,
    pub requests: Vec<DiscoveredRequest>,
}

const OPENCOLLECTION_MANIFEST: &str = "opencollection.yml";
const LEGACY_MANIFEST: &str = "bruno.json";

/// Directories never treated as request input, at any nesting level.
const ALWAYS_IGNORED_DIRS: &[&str] = &[".git", "node_modules"];

/// File name prefixes never treated as request input.
const ALWAYS_IGNORED_FILE_PREFIXES: &[&str] = &[".env"];

pub fn discover_collection(root: &Path) -> Result<BrunoCollectionDiscovery, BrunoImportError> {
    let manifest = find_manifest(root)?;
    let ignore = manifest_ignore(&manifest);
    let mut requests = Vec::new();
    walk(root, root, &manifest, &ignore, &mut requests)?;
    requests.sort_by(|a, b| a.source_path.cmp(&b.source_path));

    let has_bru = requests
        .iter()
        .any(|request| request.format == BrunoFormat::Bru);
    let has_yml = requests
        .iter()
        .any(|request| request.format == BrunoFormat::Yml);
    if has_bru && has_yml {
        return Err(BrunoImportError::new(
            root.display().to_string(),
            diagnostics::MIXED_FORMATS,
            "collection mixes .bru and .yml request files",
        ));
    }

    Ok(BrunoCollectionDiscovery {
        root: root.to_path_buf(),
        manifest,
        requests,
    })
}

fn find_manifest(root: &Path) -> Result<PathBuf, BrunoImportError> {
    for name in [OPENCOLLECTION_MANIFEST, LEGACY_MANIFEST] {
        let candidate = root.join(name);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(BrunoImportError::new(
        root.display().to_string(),
        diagnostics::NO_MANIFEST,
        "no opencollection.yml or bruno.json found",
    ))
}

/// Reads the `ignore` list from a legacy `bruno.json`. OpenCollection v1.0.0
/// has no ignore field, so YAML collections contribute none. Best-effort:
/// a malformed manifest still validates the root, it just yields no ignores.
fn manifest_ignore(manifest: &Path) -> Vec<String> {
    if manifest
        .file_name()
        .is_some_and(|name| name == LEGACY_MANIFEST)
    {
        let Ok(contents) = std::fs::read_to_string(manifest) else {
            return Vec::new();
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&contents) else {
            return Vec::new();
        };
        if let Some(ignore) = value.get("ignore").and_then(|value| value.as_array()) {
            return ignore
                .iter()
                .filter_map(|value| value.as_str())
                .map(str::to_string)
                .collect();
        }
    }
    Vec::new()
}

fn walk(
    collection_root: &Path,
    dir: &Path,
    manifest: &Path,
    ignore: &[String],
    requests: &mut Vec<DiscoveredRequest>,
) -> Result<(), BrunoImportError> {
    let entries = std::fs::read_dir(dir).map_err(|error| {
        BrunoImportError::new(
            dir.display().to_string(),
            diagnostics::IO_ERROR,
            error.to_string(),
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            BrunoImportError::new(
                dir.display().to_string(),
                diagnostics::IO_ERROR,
                error.to_string(),
            )
        })?;
        let path = entry.path();
        let relative = path
            .strip_prefix(collection_root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if path.is_dir() {
            if !is_ignored(&relative, ignore) {
                walk(collection_root, &path, manifest, ignore, requests)?;
            }
        } else if is_request_file(&path, manifest, &relative, ignore) {
            let format = if path.extension().is_some_and(|ext| ext == "bru") {
                BrunoFormat::Bru
            } else {
                BrunoFormat::Yml
            };
            requests.push(DiscoveredRequest {
                source_path: relative,
                format,
            });
        }
    }
    Ok(())
}

fn is_request_file(path: &Path, manifest: &Path, relative: &str, ignore: &[String]) -> bool {
    if path == manifest || is_ignored(relative, ignore) {
        return false;
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if ALWAYS_IGNORED_FILE_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix))
    {
        return false;
    }
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("bru") => true,
        Some("yml") => {
            let contents = std::fs::read_to_string(path).unwrap_or_default();
            looks_like_opencollection_request(&contents)
        }
        _ => false,
    }
}

/// A YAML file is a request candidate only if it opens with a top-level
/// `info:` block, the signature of an OpenCollection request or folder file.
/// Full validation happens in the YAML parser (BRUNO-04).
fn looks_like_opencollection_request(contents: &str) -> bool {
    contents
        .lines()
        .map(str::trim_start)
        .find(|line| !line.is_empty() && !line.starts_with('#') && *line != "---")
        .is_some_and(|line| line == "info:" || line.starts_with("info: "))
}

fn is_ignored(relative: &str, ignore: &[String]) -> bool {
    if relative
        .split('/')
        .any(|component| ALWAYS_IGNORED_DIRS.contains(&component))
    {
        return true;
    }
    ignore
        .iter()
        .any(|pattern| path_matches_ignore(relative, pattern))
}

/// Matches a manifest ignore entry against a relative path. Entries such as
/// `node_modules`, `private`, or `**/private/**` match the directory at any
/// nesting level; multi-component entries match as path prefixes.
fn path_matches_ignore(relative: &str, pattern: &str) -> bool {
    let pattern = pattern
        .trim()
        .trim_start_matches("./")
        .trim_start_matches("**/")
        .trim_end_matches("/**");
    if pattern.is_empty() {
        return false;
    }
    let prefix = format!("{pattern}/");
    relative == pattern
        || relative.starts_with(&prefix)
        || relative.split('/').any(|component| component == pattern)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bruno::diagnostics;
    use std::path::PathBuf;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "zaku-bruno-discovery-{label}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("temp dir should be creatable");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }

        fn write(&self, relative: &str, contents: &str) {
            let path = self.0.join(relative);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("temp parent should be creatable");
            }
            std::fs::write(path, contents).expect("temp file should be writable");
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    const OPENCOLLECTION_MANIFEST_CONTENTS: &str =
        "opencollection: \"1.0.0\"\ninfo:\n  name: Test\n";

    #[test]
    fn discovers_nested_bru_requests() {
        let dir = TempDir::new("nested-bru");
        dir.write(
            "bruno.json",
            "{\n  \"version\": \"1\",\n  \"name\": \"Nested\",\n  \"type\": \"collection\"\n}\n",
        );
        dir.write("users/get.bru", "meta {\n  name: Get\n  type: http\n}\n\nget {\n  url: https://api.example.com/users\n}\n");
        dir.write("users/list.bru", "meta {\n  name: List\n  type: http\n}\n\nget {\n  url: https://api.example.com/users\n}\n");
        dir.write("admin/settings.bru", "meta {\n  name: Settings\n  type: http\n}\n\nget {\n  url: https://api.example.com/settings\n}\n");

        let discovery = discover_collection(dir.path()).expect("discovery should succeed");
        let paths: Vec<&str> = discovery
            .requests
            .iter()
            .map(|request| request.source_path.as_str())
            .collect();
        assert_eq!(
            paths,
            vec!["admin/settings.bru", "users/get.bru", "users/list.bru"]
        );
        assert!(discovery
            .requests
            .iter()
            .all(|request| request.format == BrunoFormat::Bru));
        assert_eq!(
            discovery
                .manifest
                .file_name()
                .and_then(|name| name.to_str()),
            Some("bruno.json")
        );
    }

    #[test]
    fn discovers_yaml_requests_with_opencollection_manifest() {
        let dir = TempDir::new("nested-yml");
        dir.write("opencollection.yml", OPENCOLLECTION_MANIFEST_CONTENTS);
        dir.write("admin/settings.yml", "info:\n  name: Settings\n  type: http\n\nhttp:\n  method: GET\n  url: https://api.example.com/settings\n");
        dir.write("admin/folder.yml", "info:\n  name: Admin\n  type: folder\n");

        let discovery = discover_collection(dir.path()).expect("discovery should succeed");
        let paths: Vec<&str> = discovery
            .requests
            .iter()
            .map(|request| request.source_path.as_str())
            .collect();
        assert_eq!(paths, vec!["admin/folder.yml", "admin/settings.yml"]);
        assert!(discovery
            .requests
            .iter()
            .all(|request| request.format == BrunoFormat::Yml));
        assert_eq!(
            discovery
                .manifest
                .file_name()
                .and_then(|name| name.to_str()),
            Some("opencollection.yml")
        );
    }

    #[test]
    fn missing_manifest_is_rejected() {
        let dir = TempDir::new("no-manifest");
        dir.write("a.bru", "meta {\n  name: A\n  type: http\n}\n");

        let error = discover_collection(dir.path()).expect_err("missing manifest must fail");
        assert_eq!(error.code, diagnostics::NO_MANIFEST);
        assert_eq!(error.source_path, dir.path().display().to_string());
    }

    #[test]
    fn mixed_formats_are_rejected() {
        let dir = TempDir::new("mixed");
        dir.write("opencollection.yml", OPENCOLLECTION_MANIFEST_CONTENTS);
        dir.write("a.bru", "meta {\n  name: A\n  type: http\n}\n");
        dir.write("b.yml", "info:\n  name: B\n  type: http\n\nhttp:\n  method: GET\n  url: https://api.example.com/b\n");

        let error = discover_collection(dir.path()).expect_err("mixed formats must fail");
        assert_eq!(error.code, diagnostics::MIXED_FORMATS);
    }

    #[test]
    fn bruno_json_manifest_is_recognized() {
        let dir = TempDir::new("legacy");
        dir.write(
            "bruno.json",
            "{\n  \"version\": \"1\",\n  \"name\": \"Legacy\",\n  \"type\": \"collection\"\n}\n",
        );
        dir.write("a.bru", "meta {\n  name: A\n  type: http\n}\n");

        let discovery = discover_collection(dir.path()).expect("legacy root should succeed");
        assert_eq!(
            discovery
                .manifest
                .file_name()
                .and_then(|name| name.to_str()),
            Some("bruno.json")
        );
        assert_eq!(discovery.requests.len(), 1);
        assert_eq!(discovery.requests[0].format, BrunoFormat::Bru);
    }

    #[test]
    fn opencollection_manifest_wins_over_bruno_json() {
        let dir = TempDir::new("both-manifests");
        dir.write("opencollection.yml", OPENCOLLECTION_MANIFEST_CONTENTS);
        dir.write(
            "bruno.json",
            "{\n  \"version\": \"1\",\n  \"name\": \"Legacy\",\n  \"type\": \"collection\"\n}\n",
        );
        dir.write("a.yml", "info:\n  name: A\n  type: http\n\nhttp:\n  method: GET\n  url: https://api.example.com/a\n");

        let discovery = discover_collection(dir.path()).expect("discovery should succeed");
        assert_eq!(
            discovery
                .manifest
                .file_name()
                .and_then(|name| name.to_str()),
            Some("opencollection.yml")
        );
    }

    #[test]
    fn ignores_manifest_ignore_list_and_sensitive_files() {
        let dir = TempDir::new("ignores");
        dir.write("bruno.json", "{\n  \"version\": \"1\",\n  \"name\": \"Ig\",\n  \"type\": \"collection\",\n  \"ignore\": [\"node_modules\", \"private\", \"**/scratch/**\"]\n}\n");
        dir.write(
            "node_modules/pkg/x.bru",
            "meta {\n  name: X\n  type: http\n}\n",
        );
        dir.write(
            "private/secret.bru",
            "meta {\n  name: Secret\n  type: http\n}\n",
        );
        dir.write(
            "a/scratch/tmp.bru",
            "meta {\n  name: Tmp\n  type: http\n}\n",
        );
        dir.write("users/get.bru", "meta {\n  name: Get\n  type: http\n}\n");
        dir.write(".env", "API_KEY=__REDACTED_TEST_SECRET__\n");
        dir.write(".git/config", "[core]\n");
        dir.write("scripts/util.js", "module.exports = {};\n");
        dir.write(
            "docker-compose.yml",
            "services:\n  api:\n    image: example\n",
        );

        let discovery = discover_collection(dir.path()).expect("discovery should succeed");
        let paths: Vec<&str> = discovery
            .requests
            .iter()
            .map(|request| request.source_path.as_str())
            .collect();
        assert_eq!(paths, vec!["users/get.bru"]);
    }

    #[test]
    fn yaml_without_info_block_is_not_a_request() {
        let dir = TempDir::new("yaml-config");
        dir.write("opencollection.yml", OPENCOLLECTION_MANIFEST_CONTENTS);
        dir.write(
            "docker-compose.yml",
            "services:\n  api:\n    image: example\n",
        );
        dir.write("real.yml", "info:\n  name: Real\n  type: http\n\nhttp:\n  method: GET\n  url: https://api.example.com/real\n");

        let discovery = discover_collection(dir.path()).expect("discovery should succeed");
        let paths: Vec<&str> = discovery
            .requests
            .iter()
            .map(|request| request.source_path.as_str())
            .collect();
        assert_eq!(paths, vec!["real.yml"]);
    }
}
