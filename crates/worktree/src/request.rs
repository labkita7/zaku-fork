use anyhow::{Context, anyhow};
use path::RelPath;
use serde::{Deserialize, Serialize};
use std::mem;
use toml_edit::{Item, Table};

use crate::bru::{BruFileData, ParsedBruRequest};

pub const REQUEST_FILE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestFileState {
    Parsed(RequestFile),
    Invalid(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestFile {
    pub meta: RequestFileMeta,
    pub http: RequestFileHttp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestFileMeta {
    pub version: u32,
}

impl Default for RequestFileMeta {
    fn default() -> Self {
        Self {
            version: REQUEST_FILE_VERSION,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestFileHttp {
    pub method: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub params: Vec<RequestFileParam>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<RequestFileHeader>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<RequestFileBody>,
}

impl Default for RequestFileHttp {
    fn default() -> Self {
        Self {
            method: "GET".to_string(),
            url: String::new(),
            params: Vec::new(),
            headers: Vec::new(),
            body: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestFileParam {
    pub name: String,
    pub value: String,
    #[serde(default, skip_serializing_if = "util::serde::is_false")]
    pub disabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestFileHeader {
    pub name: String,
    pub value: String,
    #[serde(default, skip_serializing_if = "util::serde::is_false")]
    pub disabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestFileBody {
    pub r#type: RequestFileBodyType,
    #[serde(default)]
    pub data: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RequestFileBodyType {
    Text,
    Json,
    Html,
    Xml,
}

impl RequestFileBodyType {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Text => "Text",
            Self::Json => "JSON",
            Self::Html => "HTML",
            Self::Xml => "XML",
        }
    }
}

pub fn serialize_request_file(request_file: &RequestFile) -> anyhow::Result<String> {
    let mut document = toml_edit::ser::to_document(request_file)?;
    promote_to_table(document.as_table_mut(), "meta")
        .context("Failed to serialize request meta")?;
    promote_to_table(document.as_table_mut(), "http")
        .context("Failed to serialize request http")?;
    Ok(document.to_string())
}

fn promote_to_table(parent: &mut Table, key: &str) -> anyhow::Result<()> {
    let Some(item) = parent.get_mut(key) else {
        return Ok(());
    };
    if item.is_table() {
        return Ok(());
    }

    let original_item = mem::take(item);
    let table = match original_item.into_table() {
        Ok(table) => table,
        Err(original_item) => {
            let item_type = original_item.type_name();
            *item = original_item;
            return Err(anyhow!("expected {key} to be table, got {item_type}"));
        }
    };
    *item = Item::Table(table);
    Ok(())
}

pub fn parse_request_file(contents: &str, path: &RelPath) -> RequestFileState {
    parse_request_file_with_bru(contents, path).0
}

pub fn parse_request_file_with_bru(
    contents: &str,
    path: &RelPath,
) -> (RequestFileState, Option<BruFileData>) {
    if is_bru_request_path(path) {
        match crate::bru::parse_bru_request(contents) {
            Ok(ParsedBruRequest { request, data }) => {
                (RequestFileState::Parsed(request), Some(data))
            }
            Err(error) => (RequestFileState::Invalid(error), None),
        }
    } else {
        (parse_toml_request_file(contents), None)
    }
}

fn parse_toml_request_file(contents: &str) -> RequestFileState {
    match toml::from_str::<RequestFile>(contents) {
        Ok(request_file) => RequestFileState::Parsed(request_file),
        Err(error) => RequestFileState::Invalid(error.to_string()),
    }
}

/// Files the request panel treats as requests: Zaku's own `.toml` format plus
/// Bruno request files, excluding Bruno collection metadata files.
pub fn is_request_file_path(path: &RelPath) -> bool {
    let Some(extension) = path.extension() else {
        return false;
    };
    if extension.eq_ignore_ascii_case("toml") {
        return true;
    }
    if !extension.eq_ignore_ascii_case("bru") {
        return false;
    }

    let Some(file_name) = path.file_name() else {
        return false;
    };
    if file_name.eq_ignore_ascii_case("collection.bru")
        || file_name.eq_ignore_ascii_case("folder.bru")
    {
        return false;
    }
    !path
        .components()
        .any(|component| component.eq_ignore_ascii_case("environments"))
}

pub fn is_bru_request_path(path: &RelPath) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("bru"))
}

pub fn serialize_request_file_at(
    request_file: &RequestFile,
    path: &RelPath,
    bru: Option<&BruFileData>,
) -> anyhow::Result<String> {
    if is_bru_request_path(path) {
        Ok(crate::bru::serialize_bru_request(
            request_file,
            bru.unwrap_or(&BruFileData::default()),
        ))
    } else {
        serialize_request_file(request_file)
    }
}

pub fn request_method_short_name(method: &str) -> String {
    let method = method.trim().to_ascii_uppercase();
    match method.as_str() {
        "GET" => "GET".to_string(),
        "POST" => "POST".to_string(),
        "PUT" => "PUT".to_string(),
        "PATCH" => "PTCH".to_string(),
        "DELETE" => "DEL".to_string(),
        "HEAD" => "HEAD".to_string(),
        "OPTIONS" => "OPT".to_string(),
        "QUERY" => "QURY".to_string(),
        _ => method.chars().take(4).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use indoc::indoc;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_parse_request_file() {
        let request_file = parse_request_file(
            indoc! {r#"
            [meta]
            version = 1

            [http]
            method = "POST"
            url = "https://api.zaku.dev/search"
            params = [
                { name = "query", value = "zaku" },
                { name = "debug", value = "1", disabled = true },
                { name = "test", value = "1", disabled = false },
            ]
            headers = [
                { name = "Content-Type", value = "application/json" },
                { name = "X-Debug", value = "1", disabled = true },
            ]
            body = { type = "json", data = '''
            {
              "hello": "world"
            }''' }
        "#},
            path::rel_path("request.toml"),
        );

        assert_eq!(
            request_file,
            RequestFileState::Parsed(RequestFile {
                meta: RequestFileMeta {
                    version: REQUEST_FILE_VERSION,
                },
                http: RequestFileHttp {
                    method: "POST".to_string(),
                    url: "https://api.zaku.dev/search".to_string(),
                    params: vec![
                        RequestFileParam {
                            name: "query".to_string(),
                            value: "zaku".to_string(),
                            disabled: false,
                        },
                        RequestFileParam {
                            name: "debug".to_string(),
                            value: "1".to_string(),
                            disabled: true,
                        },
                        RequestFileParam {
                            name: "test".to_string(),
                            value: "1".to_string(),
                            disabled: false,
                        },
                    ],
                    headers: vec![
                        RequestFileHeader {
                            name: "Content-Type".to_string(),
                            value: "application/json".to_string(),
                            disabled: false,
                        },
                        RequestFileHeader {
                            name: "X-Debug".to_string(),
                            value: "1".to_string(),
                            disabled: true,
                        },
                    ],
                    body: Some(RequestFileBody {
                        r#type: RequestFileBodyType::Json,
                        data: indoc! {r#"
                            {
                              "hello": "world"
                            }"#}
                        .to_string(),
                    }),
                },
            })
        );
    }

    #[test]
    fn test_serialize_request_file() {
        let request_file = RequestFile {
            meta: RequestFileMeta {
                version: REQUEST_FILE_VERSION,
            },
            http: RequestFileHttp {
                method: "POST".to_string(),
                url: "https://api.zaku.dev/search".to_string(),
                params: vec![
                    RequestFileParam {
                        name: "query".to_string(),
                        value: "zaku".to_string(),
                        disabled: false,
                    },
                    RequestFileParam {
                        name: "debug".to_string(),
                        value: "1".to_string(),
                        disabled: true,
                    },
                    RequestFileParam {
                        name: "test".to_string(),
                        value: "1".to_string(),
                        disabled: false,
                    },
                ],
                headers: vec![
                    RequestFileHeader {
                        name: "Content-Type".to_string(),
                        value: "application/json".to_string(),
                        disabled: false,
                    },
                    RequestFileHeader {
                        name: "X-Debug".to_string(),
                        value: "1".to_string(),
                        disabled: true,
                    },
                ],
                body: Some(RequestFileBody {
                    r#type: RequestFileBodyType::Json,
                    data: indoc! {r#"
                        {
                          "hello": "world"
                        }"#}
                    .to_string(),
                }),
            },
        };

        let serialized = serialize_request_file(&request_file).unwrap();
        let expected = indoc! {r#"
            [meta]
            version = 1

            [http]
            method = "POST"
            url = "https://api.zaku.dev/search"
            params = [{ name = "query", value = "zaku" }, { name = "debug", value = "1", disabled = true }, { name = "test", value = "1" }]
            headers = [{ name = "Content-Type", value = "application/json" }, { name = "X-Debug", value = "1", disabled = true }]
            body = { type = "json", data = """
            {
              "hello": "world"
            }""" }
        "#};

        assert_eq!(serialized, expected);
        assert_eq!(
            parse_request_file(&serialized, path::rel_path("request.toml")),
            RequestFileState::Parsed(request_file)
        );
    }

    #[test]
    fn test_parse_bru_request_file() {
        let (request_file, bru) = parse_request_file_with_bru(
            indoc! {"
            meta {
              name: countries
              type: graphql
              seq: 1
            }

            post {
              url: https://countries.trevorblades.com/graphql
            }

            body:graphql {
              query {
                country
              }
            }
        "},
            path::rel_path("collection/countries.bru"),
        );

        let RequestFileState::Parsed(request_file) = request_file else {
            panic!("Expected the bruno request to parse");
        };
        assert_eq!(request_file.http.method, "POST");
        assert_eq!(
            request_file.http.url,
            "https://countries.trevorblades.com/graphql"
        );
        assert!(bru.is_some());
    }
}
