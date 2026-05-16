use std::process::Command;

use serde::{Deserialize, Serialize};
use tiny_http::{Header, Method, Response, Server, StatusCode};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct GistSummary {
    id: String,
    description: Option<String>,
    html_url: String,
    files: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubGist {
    id: String,
    description: Option<String>,
    html_url: String,
    files: std::collections::BTreeMap<String, serde_json::Value>,
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let server = Server::http(format!("0.0.0.0:{port}"))?;

    println!("listening on http://0.0.0.0:{port}");

    for request in server.incoming_requests() {
        let (status, body) = if request.method() != &Method::Get {
            error_response(400, "Only GET requests are supported")
        } else {
            handle_path(request.url(), fetch_gists)
        };

        let response = Response::from_string(body)
            .with_status_code(StatusCode(status))
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap());

        let _ = request.respond(response);
    }

    Ok(())
}

fn handle_path(
    path: &str,
    fetcher: fn(&str) -> Result<Vec<GistSummary>, (u16, String)>,
) -> (u16, String) {
    let Some(user) = path.strip_prefix('/') else {
        return error_response(400, "Path must be /{user}");
    };

    if user.is_empty() || user.contains('/') {
        return error_response(400, "Path must be /{user}");
    }

    match fetcher(user) {
        Ok(gists) => (200, serde_json::to_string(&gists).unwrap()),
        Err((status, message)) => error_response(status, &message),
    }
}

fn fetch_gists(user: &str) -> Result<Vec<GistSummary>, (u16, String)> {
    let output = Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--location",
            "--header",
            "Accept: application/vnd.github+json",
            "--header",
            "X-GitHub-Api-Version: 2022-11-28",
            "--header",
            "User-Agent: equal-experts-operability-assignment",
            "--write-out",
            "\n%{http_code}",
            &format!("https://api.github.com/users/{user}/gists"),
        ])
        .output()
        .map_err(|error| {
            (
                502,
                json_error(&format!("GitHub API request failed: {error}")),
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err((
            502,
            json_error(&format!("GitHub API request failed: {}", stderr.trim())),
        ));
    }

    let stdout = String::from_utf8(output.stdout).map_err(|error| {
        (
            502,
            json_error(&format!("Invalid GitHub response: {error}")),
        )
    })?;

    let (body, status) = stdout.trim_end().rsplit_once('\n').ok_or_else(|| {
        (
            502,
            json_error("GitHub response did not include a status code"),
        )
    })?;

    match status {
        "200" => {
            let gists: Vec<GitHubGist> = serde_json::from_str(body).map_err(|error| {
                (
                    502,
                    json_error(&format!("Failed to parse GitHub response: {error}")),
                )
            })?;

            Ok(gists
                .into_iter()
                .map(|gist| GistSummary {
                    id: gist.id,
                    description: gist.description,
                    html_url: gist.html_url,
                    files: gist.files.into_keys().collect(),
                })
                .collect())
        }
        "404" => Err((
            404,
            json_error(&format!("GitHub user '{user}' was not found")),
        )),
        code => Err((
            502,
            json_error(&format!("GitHub API returned status {code}")),
        )),
    }
}

fn error_response(status: u16, message: &str) -> (u16, String) {
    (status, json_error(message))
}

fn json_error(message: &str) -> String {
    serde_json::json!({ "error": message }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_gists_for_octocat() {
        let response = handle_path("/octocat", |_| {
            Ok(vec![GistSummary {
                id: "1".to_string(),
                description: Some("octocat gist".to_string()),
                html_url: "https://gist.github.com/octocat/1".to_string(),
                files: vec!["hello.txt".to_string()],
            }])
        });

        assert_eq!(response.0, 200);
        assert!(response.1.contains("\"id\":\"1\""));
    }

    #[test]
    fn returns_not_found_for_missing_user() {
        let response = handle_path("/missing-user", |_| {
            Err((404, json_error("GitHub user 'missing-user' was not found")))
        });

        assert_eq!(response.0, 404);
        assert!(response.1.contains("missing-user"));
    }
}
