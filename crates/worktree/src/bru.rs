use serde_json::Value;

use crate::request::{
    RequestFile, RequestFileBody, RequestFileBodyType, RequestFileHeader, RequestFileParam,
};

/// Bruno `.bru` request file: everything Zaku understands is lifted into
/// [`RequestFile`], while the original blocks are kept verbatim so a save
/// round-trips without losing Bruno-only sections (auth, vars, scripts, docs).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BruFileData {
    blocks: Vec<BruBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BruBlock {
    name: String,
    lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedBruRequest {
    pub(crate) request: RequestFile,
    pub(crate) data: BruFileData,
}

const METHOD_BLOCK_NAMES: [&str; 9] = [
    "get", "post", "put", "delete", "patch", "head", "options", "connect", "trace",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BodyKind {
    Plain(RequestFileBodyType),
    Graphql,
}

pub(crate) fn parse_bru_request(contents: &str) -> Result<ParsedBruRequest, String> {
    let blocks = parse_blocks(contents)?;
    let mut request = RequestFile::default();
    let mut meta_type = String::new();

    for block in &blocks {
        if block.name == "meta" {
            for (_, name, value) in parse_kv_pairs(&block.lines) {
                if name.eq_ignore_ascii_case("type") {
                    meta_type = value.to_ascii_lowercase();
                }
            }
        }
    }

    for block in &blocks {
        if is_method_block_name(&block.name) {
            request.http.method = block.name.to_ascii_uppercase();
            for (_, name, value) in parse_kv_pairs(&block.lines) {
                if name.eq_ignore_ascii_case("url") {
                    request.http.url = value;
                }
            }
            break;
        }
    }
    if request.http.method == "GET" && is_graphql_type(&meta_type) {
        request.http.method = "POST".to_string();
    }

    let mut params_loaded = false;
    for block in &blocks {
        match block.name.as_str() {
            "headers" => {
                request.http.headers = parse_kv_pairs(&block.lines)
                    .into_iter()
                    .map(|(disabled, name, value)| RequestFileHeader {
                        name,
                        value,
                        disabled,
                    })
                    .collect();
            }
            "params" | "params:query" if !params_loaded => {
                request.http.params = parse_kv_pairs(&block.lines)
                    .into_iter()
                    .map(|(disabled, name, value)| RequestFileParam {
                        name,
                        value,
                        disabled,
                    })
                    .collect();
                params_loaded = true;
            }
            _ => {}
        }
    }

    let mut body: Option<(BodyKind, String)> = None;
    let mut graphql_vars: Option<String> = None;
    for block in &blocks {
        if block.name == "body:graphql:vars" {
            if graphql_vars.is_none() {
                graphql_vars = Some(dedent_lines(&block.lines));
            }
            continue;
        }
        if body.is_none()
            && let Some(kind) = body_kind_for_block(&block.name)
        {
            body = Some((kind, dedent_lines(&block.lines)));
        }
    }
    if let Some((kind, data)) = body {
        request.http.body = Some(match kind {
            BodyKind::Plain(body_type) => RequestFileBody {
                r#type: body_type,
                data,
            },
            BodyKind::Graphql => RequestFileBody {
                r#type: RequestFileBodyType::Json,
                data: graphql_envelope(data, graphql_vars),
            },
        });
    }

    Ok(ParsedBruRequest {
        request,
        data: BruFileData { blocks },
    })
}

pub(crate) fn serialize_bru_request(request_file: &RequestFile, data: &BruFileData) -> String {
    let mut blocks = data.blocks.clone();
    if blocks.is_empty() {
        blocks.push(BruBlock {
            name: "meta".to_string(),
            lines: vec![
                "  name: request".to_string(),
                "  type: http".to_string(),
                "  seq: 1".to_string(),
            ],
        });
    }

    // Method block: replace the url entry, keep other entries (auth, body selector).
    let method_name = request_file.http.method.trim().to_ascii_lowercase();
    let url = request_file.http.url.trim();
    let url_line = (!url.is_empty()).then(|| format!("  url: {url}"));
    let mut method_present = false;
    for block in &mut blocks {
        if !is_method_block_name(&block.name) {
            continue;
        }
        method_present = true;
        block.name.clone_from(&method_name);
        block.lines.retain(
            |line| !matches!(kv_entry(line), Some((name, _)) if name.eq_ignore_ascii_case("url")),
        );
        if let Some(url_line) = url_line.clone() {
            block.lines.insert(0, url_line);
        }
        break;
    }
    if !method_present {
        let mut lines = Vec::new();
        if let Some(url_line) = url_line {
            lines.push(url_line);
        }
        blocks.push(BruBlock {
            name: method_name,
            lines,
        });
    }

    let headers = request_file
        .http
        .headers
        .iter()
        .map(|header| kv_line(header.disabled, &header.name, &header.value))
        .collect();
    set_block_lines(&mut blocks, "headers", headers);

    let params_block_name = if blocks.iter().any(|block| block.name == "params:query") {
        "params:query"
    } else if blocks.iter().any(|block| block.name == "params") {
        "params"
    } else {
        "params:query"
    };
    let params = request_file
        .http
        .params
        .iter()
        .map(|param| kv_line(param.disabled, &param.name, &param.value))
        .collect();
    set_block_lines(&mut blocks, params_block_name, params);

    let original_body_index = blocks
        .iter()
        .position(|block| block.name.starts_with("body:"))
        .unwrap_or(blocks.len());
    let original_body_block = blocks
        .iter()
        .find(|block| block.name != "body:graphql:vars" && block.name.starts_with("body:"))
        .map(|block| block.name.clone());
    blocks.retain(|block| !block.name.starts_with("body:"));
    let mut body_blocks = Vec::new();
    if let Some(body) = &request_file.http.body {
        if body.r#type == RequestFileBodyType::Json
            && original_body_block.as_deref() == Some("body:graphql")
        {
            let (query, variables) = unwrap_graphql_envelope(&body.data);
            body_blocks.push(BruBlock {
                name: "body:graphql".to_string(),
                lines: text_block_lines(&query),
            });
            if let Some(variables) = variables {
                body_blocks.push(BruBlock {
                    name: "body:graphql:vars".to_string(),
                    lines: text_block_lines(&variables),
                });
            }
        } else {
            body_blocks.push(BruBlock {
                name: body_block_name(body.r#type, original_body_block.as_deref()),
                lines: text_block_lines(&body.data),
            });
        }
    }
    let insert_at = original_body_index.min(blocks.len());
    blocks.splice(insert_at..insert_at, body_blocks);

    let mut output = String::new();
    for block in &blocks {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(&block.name);
        output.push_str(" {\n");
        for line in &block.lines {
            output.push_str(line);
            output.push('\n');
        }
        output.push_str("}\n");
    }
    output
}

fn parse_blocks(contents: &str) -> Result<Vec<BruBlock>, String> {
    let contents = contents.replace("\r\n", "\n");
    let lines: Vec<&str> = contents.split('\n').collect();
    let mut blocks = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let Some(name) = lines.get(index).and_then(|line| block_name(line)) else {
            index += 1;
            continue;
        };
        index += 1;
        let mut content = Vec::new();
        while let Some(line) = lines.get(index) {
            if line.starts_with('}') {
                break;
            }
            content.push((*line).to_string());
            index += 1;
        }
        if lines.get(index).is_none() {
            return Err(format!("unterminated block `{name}`"));
        }
        blocks.push(BruBlock {
            name,
            lines: content,
        });
        index += 1;
    }
    Ok(blocks)
}

fn block_name(line: &str) -> Option<String> {
    let line = line.trim();
    let name = line.strip_suffix('{')?.trim_end();
    if name.is_empty()
        || !name.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | ':' | '-' | '.' | '@')
        })
    {
        return None;
    }
    Some(name.to_owned())
}

fn is_method_block_name(name: &str) -> bool {
    METHOD_BLOCK_NAMES
        .iter()
        .any(|method| method.eq_ignore_ascii_case(name))
}

fn is_graphql_type(meta_type: &str) -> bool {
    matches!(meta_type, "graphql" | "graphql-request")
}

fn body_kind_for_block(name: &str) -> Option<BodyKind> {
    match name {
        "body:json" => Some(BodyKind::Plain(RequestFileBodyType::Json)),
        "body:text" => Some(BodyKind::Plain(RequestFileBodyType::Text)),
        "body:xml" => Some(BodyKind::Plain(RequestFileBodyType::Xml)),
        "body:html" => Some(BodyKind::Plain(RequestFileBodyType::Html)),
        "body:sparql" | "body:formUrlEncoded" | "body:multipartForm" | "body:ndjson" => {
            Some(BodyKind::Plain(RequestFileBodyType::Text))
        }
        "body:graphql" => Some(BodyKind::Graphql),
        _ => None,
    }
}

fn body_block_name(body_type: RequestFileBodyType, original: Option<&str>) -> String {
    if let Some(original) = original
        && body_kind_for_block(original) == Some(BodyKind::Plain(body_type))
    {
        return original.to_owned();
    }
    match body_type {
        RequestFileBodyType::Json => "body:json",
        RequestFileBodyType::Text => "body:text",
        RequestFileBodyType::Xml => "body:xml",
        RequestFileBodyType::Html => "body:html",
    }
    .to_owned()
}

fn graphql_envelope(query: String, vars: Option<String>) -> String {
    let mut envelope = serde_json::Map::new();
    envelope.insert("query".to_owned(), Value::String(query.clone()));
    if let Some(vars) = vars
        .filter(|vars| !vars.trim().is_empty())
        .and_then(|vars| serde_json::from_str::<Value>(&vars).ok())
    {
        envelope.insert("variables".to_owned(), vars);
    }
    serde_json::to_string_pretty(&Value::Object(envelope)).unwrap_or(query)
}

fn unwrap_graphql_envelope(data: &str) -> (String, Option<String>) {
    match serde_json::from_str::<Value>(data) {
        Ok(Value::String(query)) => (query, None),
        Ok(Value::Object(object)) => {
            let query = object
                .get("query")
                .and_then(Value::as_str)
                .unwrap_or(data)
                .to_owned();
            let variables = object.get("variables").map(|variables| {
                serde_json::to_string_pretty(variables).unwrap_or_else(|_| variables.to_string())
            });
            (query, variables)
        }
        _ => (data.to_owned(), None),
    }
}

fn parse_kv_pairs(lines: &[String]) -> Vec<(bool, String, String)> {
    let mut pairs = Vec::new();
    let mut index = 0;
    while let Some(line) = lines.get(index) {
        let line = line.trim();
        index += 1;
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        let (disabled, line) = match line.strip_prefix('~') {
            Some(rest) => (true, rest.trim_start()),
            None => (false, line),
        };
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        pairs.push((
            disabled,
            name.to_owned(),
            parse_kv_value(value.trim(), lines, &mut index),
        ));
    }
    pairs
}

fn parse_kv_value(value: &str, lines: &[String], index: &mut usize) -> String {
    if let Some(rest) = value.strip_prefix("'''") {
        if let Some(single_line) = rest.strip_suffix("'''") {
            return single_line.to_owned();
        }
        let mut parts = vec![rest.to_owned()];
        while let Some(line) = lines.get(*index) {
            *index += 1;
            match line.trim().strip_suffix("'''") {
                Some(before) => {
                    if !before.is_empty() {
                        parts.push(before.to_owned());
                    }
                    break;
                }
                None => parts.push(line.trim().to_owned()),
            }
        }
        return parts.join("\n");
    }
    if let Some(quoted) = value
        .strip_prefix('"')
        .and_then(|quoted| quoted.strip_suffix('"'))
    {
        return quoted.to_owned();
    }
    value.to_owned()
}

fn kv_entry(line: &str) -> Option<(&str, &str)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with("//") || line.starts_with('~') {
        return None;
    }
    let (name, value) = line.split_once(':')?;
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    Some((name, value.trim()))
}

fn kv_line(disabled: bool, name: &str, value: &str) -> String {
    let prefix = if disabled { "~" } else { "" };
    if value.contains('\n') {
        format!("  {prefix}{name}: '''\n{value}\n  '''")
    } else {
        format!("  {prefix}{name}: {value}")
    }
}

fn set_block_lines(blocks: &mut Vec<BruBlock>, name: &str, lines: Vec<String>) {
    if lines.is_empty() {
        blocks.retain(|block| block.name != name);
    } else if let Some(block) = blocks.iter_mut().find(|block| block.name == name) {
        block.lines = lines;
    } else {
        blocks.push(BruBlock {
            name: name.to_owned(),
            lines,
        });
    }
}

fn text_block_lines(data: &str) -> Vec<String> {
    data.split('\n')
        .map(|line| {
            if line.is_empty() {
                String::new()
            } else {
                format!("  {line}")
            }
        })
        .collect()
}

fn dedent_lines(lines: &[String]) -> String {
    let indent = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.len() - line.trim_start().len())
        .min()
        .unwrap_or(0);
    lines
        .iter()
        .map(|line| {
            if line.is_empty() {
                String::new()
            } else {
                line.get(indent.min(line.len())..).unwrap_or("").to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim_end()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::request::{
        RequestFileBody, RequestFileBodyType, RequestFileHeader, RequestFileParam,
    };
    use indoc::indoc;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_parse_bru_http_request() {
        let parsed = parse_bru_request(indoc! {r#"
            meta {
              name: Get users
              type: http
              seq: 1
            }

            get {
              url: https://api.example.com/users
            }

            headers {
              Content-Type: application/json
              ~X-Debug: 1
            }

            params:query {
              page: 2
              ~debug: true
            }

            body:json {
              {
                "page": 1
              }
            }

            docs {
              Some docs
            }
        "#})
        .unwrap();

        assert_eq!(parsed.request.http.method, "GET");
        assert_eq!(parsed.request.http.url, "https://api.example.com/users");
        assert_eq!(
            parsed.request.http.headers,
            vec![
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
            ]
        );
        assert_eq!(
            parsed.request.http.params,
            vec![
                RequestFileParam {
                    name: "page".to_string(),
                    value: "2".to_string(),
                    disabled: false,
                },
                RequestFileParam {
                    name: "debug".to_string(),
                    value: "true".to_string(),
                    disabled: true,
                },
            ]
        );
        assert_eq!(
            parsed.request.http.body,
            Some(RequestFileBody {
                r#type: RequestFileBodyType::Json,
                data: "{\n  \"page\": 1\n}".to_string(),
            })
        );
    }

    #[test]
    fn test_parse_bru_graphql_request() {
        let parsed = parse_bru_request(indoc! {r#"
            meta {
              name: countries
              type: graphql
              seq: 1
            }

            post {
              url: https://countries.trevorblades.com/graphql
            }

            body:graphql {
              query getCountry($code: ID!) {
                country(code: $code) {
                  capital
                }
              }
            }

            body:graphql:vars {
              {
                "code": "US"
              }
            }
        "#})
        .unwrap();

        assert_eq!(parsed.request.http.method, "POST");
        assert_eq!(
            parsed.request.http.url,
            "https://countries.trevorblades.com/graphql"
        );
        let body = parsed.request.http.body.unwrap();
        assert_eq!(body.r#type, RequestFileBodyType::Json);
        let envelope: Value = serde_json::from_str(&body.data).unwrap();
        assert_eq!(
            envelope.get("query").and_then(Value::as_str),
            Some("query getCountry($code: ID!) {\n  country(code: $code) {\n    capital\n  }\n}")
        );
        assert_eq!(
            envelope.get("variables"),
            Some(&serde_json::json!({ "code": "US" }))
        );
    }

    #[test]
    fn test_bru_graphql_without_method_block_defaults_to_post() {
        let parsed = parse_bru_request(indoc! {"
            meta {
              name: countries
              type: graphql
              seq: 1
            }

            body:graphql {
              query {
                country
              }
            }
        "})
        .unwrap();

        assert_eq!(parsed.request.http.method, "POST");
    }

    #[test]
    fn test_serialize_bru_round_trip() {
        let contents = indoc! {r#"
            meta {
              name: Get users
              type: http
              seq: 1
            }

            get {
              url: https://api.example.com/users
            }

            headers {
              Content-Type: application/json
              ~X-Debug: 1
            }

            params:query {
              page: 2
              ~debug: true
            }

            body:json {
              {
                "page": 1
              }
            }

            docs {
              Some docs
            }
        "#};

        let parsed = parse_bru_request(contents).unwrap();
        assert_eq!(
            serialize_bru_request(&parsed.request, &parsed.data),
            contents
        );
    }

    #[test]
    fn test_serialize_bru_graphql_round_trip() {
        let contents = indoc! {r#"
            meta {
              name: countries
              type: graphql
              seq: 1
            }

            post {
              url: https://countries.trevorblades.com/graphql
            }

            body:graphql {
              query getCountry($code: ID!) {
                country(code: $code) {
                  capital
                }
              }
            }

            body:graphql:vars {
              {
                "code": "US"
              }
            }
        "#};

        let parsed = parse_bru_request(contents).unwrap();
        assert_eq!(
            serialize_bru_request(&parsed.request, &parsed.data),
            contents
        );
    }

    #[test]
    fn test_serialize_bru_preserves_unknown_blocks() {
        let contents = indoc! {r#"
            meta {
              name: login
              type: http
              seq: 3
            }

            post {
              url: https://api.example.com/login
            }

            auth:basic {
              username: asd
              password: j
            }

            script:pre-request {
              bru.setVar("foo", "foo-world-2");
            }

            tests {
              test("should login", function() {
                expect(res.getStatus()).to.equal(200);
              });
            }
        "#};

        let parsed = parse_bru_request(contents).unwrap();
        let serialized = serialize_bru_request(&parsed.request, &parsed.data);
        assert_eq!(serialized, contents);

        let reparsed = parse_bru_request(&serialized).unwrap();
        assert_eq!(reparsed, parsed);
    }
}
