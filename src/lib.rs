use std::{
    collections::BTreeMap,
    error::Error,
    fmt,
    io::{BufRead, BufReader, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct GistSummary {
    pub id: String,
    pub description: Option<String>,
    pub html_url: String,
    pub files: Vec<String>,
}

pub trait GistService: Send + Sync {
    fn list_public_gists(&self, user: &str) -> Result<Vec<GistSummary>, AppError>;
}

#[derive(Debug, Clone)]
pub enum AppError {
    BadRequest(String),
    UserNotFound(String),
    Upstream(String),
}

impl AppError {
    fn status_code(&self) -> u16 {
        match self {
            Self::BadRequest(_) => 400,
            Self::UserNotFound(_) => 404,
            Self::Upstream(_) => 502,
        }
    }

    fn reason_phrase(&self) -> &'static str {
        match self {
            Self::BadRequest(_) => "Bad Request",
            Self::UserNotFound(_) => "Not Found",
            Self::Upstream(_) => "Bad Gateway",
        }
    }

    fn message(&self) -> String {
        match self {
            Self::BadRequest(message) => message.clone(),
            Self::UserNotFound(user) => format!("GitHub user '{user}' was not found"),
            Self::Upstream(message) => message.clone(),
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl Error for AppError {}

pub struct GitHubClient;

impl GitHubClient {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GitHubClient {
    fn default() -> Self {
        Self::new()
    }
}

impl GistService for GitHubClient {
    fn list_public_gists(&self, user: &str) -> Result<Vec<GistSummary>, AppError> {
        let url = format!("https://api.github.com/users/{user}/gists");
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
                &url,
            ])
            .output()
            .map_err(|error| {
                AppError::Upstream(format!(
                    "Failed to invoke curl for GitHub API call: {error}"
                ))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(AppError::Upstream(format!(
                "GitHub API request failed: {}",
                if stderr.is_empty() {
                    "curl exited with a non-zero status".to_string()
                } else {
                    stderr
                }
            )));
        }

        let stdout = String::from_utf8(output.stdout).map_err(|error| {
            AppError::Upstream(format!("GitHub response was not valid UTF-8: {error}"))
        })?;

        let (body, status_code) = split_body_and_status(&stdout)?;
        match status_code {
            200 => parse_gists(body),
            404 => Err(AppError::UserNotFound(user.to_string())),
            code => Err(AppError::Upstream(format!(
                "GitHub API returned unexpected status code {code}"
            ))),
        }
    }
}

fn split_body_and_status(response: &str) -> Result<(&str, u16), AppError> {
    let trimmed = response.trim_end_matches(['\r', '\n']);
    let split_index = trimmed.rfind('\n').ok_or_else(|| {
        AppError::Upstream("GitHub response did not include an HTTP status code".to_string())
    })?;

    let (body, status) = trimmed.split_at(split_index);
    let status_code = status.trim().parse::<u16>().map_err(|error| {
        AppError::Upstream(format!("Could not parse GitHub status code: {error}"))
    })?;

    Ok((body, status_code))
}

fn parse_gists(body: &str) -> Result<Vec<GistSummary>, AppError> {
    let raw_gists = serde_json::from_str::<Vec<GitHubGist>>(body)
        .map_err(|error| AppError::Upstream(format!("Failed to parse GitHub response: {error}")))?;

    Ok(raw_gists.into_iter().map(GistSummary::from).collect())
}

#[derive(Debug, Deserialize)]
struct GitHubGist {
    id: String,
    description: Option<String>,
    html_url: String,
    files: BTreeMap<String, serde_json::Value>,
}

impl From<GitHubGist> for GistSummary {
    fn from(value: GitHubGist) -> Self {
        let files = value.files.into_keys().collect::<Vec<_>>();

        Self {
            id: value.id,
            description: value.description,
            html_url: value.html_url,
            files,
        }
    }
}

pub fn run_server(listener: TcpListener, service: Arc<dyn GistService>, shutdown: Arc<AtomicBool>) {
    listener
        .set_nonblocking(true)
        .expect("failed to configure listener");

    while !shutdown.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => {
                let service = Arc::clone(&service);
                thread::spawn(move || {
                    if let Err(error) = handle_connection(stream, service) {
                        eprintln!("connection error: {error}");
                    }
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => {
                eprintln!("listener error: {error}");
                break;
            }
        }
    }
}

pub fn handle_connection(
    mut stream: TcpStream,
    service: Arc<dyn GistService>,
) -> Result<(), Box<dyn Error>> {
    let mut request_line = String::new();
    {
        let mut reader = BufReader::new(&mut stream);
        reader.read_line(&mut request_line)?;

        if request_line.is_empty() {
            write_response(
                &mut stream,
                400,
                "Bad Request",
                json_error("Request line was empty"),
            )?;
            return Ok(());
        }

        let mut discard = String::new();
        loop {
            discard.clear();
            let bytes_read = reader.read_line(&mut discard)?;
            if bytes_read == 0 || discard == "\r\n" {
                break;
            }
        }
    }

    let response = match route_request(request_line.trim_end(), service) {
        Ok(body) => (200, "OK", body),
        Err(error) => (
            error.status_code(),
            error.reason_phrase(),
            json_error(&error.message()),
        ),
    };

    write_response(&mut stream, response.0, response.1, response.2)?;
    Ok(())
}

fn route_request(request_line: &str, service: Arc<dyn GistService>) -> Result<String, AppError> {
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| AppError::BadRequest("Request line is incomplete".to_string()))?;
    let path = parts
        .next()
        .ok_or_else(|| AppError::BadRequest("Request path is missing".to_string()))?;

    if method != "GET" {
        return Err(AppError::BadRequest(
            "Only GET requests are supported".to_string(),
        ));
    }

    let user = path
        .strip_prefix('/')
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::BadRequest("Path must be /{user}".to_string()))?;

    if user.contains('/') {
        return Err(AppError::BadRequest(
            "Path must contain a single user segment".to_string(),
        ));
    }

    let gists = service.list_public_gists(user)?;
    serde_json::to_string(&gists)
        .map_err(|error| AppError::Upstream(format!("Failed to serialize response body: {error}")))
}

fn write_response(
    stream: &mut TcpStream,
    status_code: u16,
    reason_phrase: &str,
    body: String,
) -> Result<(), Box<dyn Error>> {
    let response = format!(
        "HTTP/1.1 {status_code} {reason_phrase}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );

    stream.write_all(response.as_bytes())?;
    stream.flush()?;
    Ok(())
}

fn json_error(message: &str) -> String {
    serde_json::json!({ "error": message }).to_string()
}

pub fn local_address(listener: &TcpListener) -> SocketAddr {
    listener
        .local_addr()
        .expect("listener should have a local address")
}

pub fn read_http_response(mut stream: TcpStream) -> String {
    let mut buffer = String::new();
    stream
        .read_to_string(&mut buffer)
        .expect("failed to read response");
    buffer
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubGistService {
        response: Result<Vec<GistSummary>, AppError>,
    }

    impl GistService for StubGistService {
        fn list_public_gists(&self, _user: &str) -> Result<Vec<GistSummary>, AppError> {
            self.response.clone()
        }
    }

    #[test]
    fn parses_github_gists_into_summaries() {
        let body = r#"
        [
          {
            "id": "1",
            "description": "octocat gist",
            "html_url": "https://gist.github.com/octocat/1",
            "files": {
              "zeta.txt": {},
              "alpha.txt": {}
            }
          }
        ]
        "#;

        let gists = parse_gists(body).expect("gists should parse");

        assert_eq!(
            gists,
            vec![GistSummary {
                id: "1".to_string(),
                description: Some("octocat gist".to_string()),
                html_url: "https://gist.github.com/octocat/1".to_string(),
                files: vec!["alpha.txt".to_string(), "zeta.txt".to_string()],
            }]
        );
    }

    #[test]
    fn returns_gists_for_a_user() {
        let service = Arc::new(StubGistService {
            response: Ok(vec![GistSummary {
                id: "1".to_string(),
                description: Some("octocat gist".to_string()),
                html_url: "https://gist.github.com/octocat/1".to_string(),
                files: vec!["hello.txt".to_string()],
            }]),
        });

        let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
        let address = local_address(&listener);
        let shutdown = Arc::new(AtomicBool::new(false));
        let server_shutdown = Arc::clone(&shutdown);
        let server_service = service.clone();

        let server = thread::spawn(move || {
            run_server(listener, server_service, server_shutdown);
        });

        let mut stream = TcpStream::connect(address).expect("client should connect");
        stream
            .write_all(b"GET /octocat HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .expect("request should write");

        let response = read_http_response(stream);
        shutdown.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(address);
        server.join().expect("server should stop cleanly");

        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.contains("\"id\":\"1\""));
        assert!(response.contains("\"html_url\":\"https://gist.github.com/octocat/1\""));
    }

    #[test]
    fn returns_not_found_for_unknown_users() {
        let service = Arc::new(StubGistService {
            response: Err(AppError::UserNotFound("missing-user".to_string())),
        });

        let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
        let address = local_address(&listener);
        let shutdown = Arc::new(AtomicBool::new(false));
        let server_shutdown = Arc::clone(&shutdown);
        let server_service = service.clone();

        let server = thread::spawn(move || {
            run_server(listener, server_service, server_shutdown);
        });

        let mut stream = TcpStream::connect(address).expect("client should connect");
        stream
            .write_all(b"GET /missing-user HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .expect("request should write");

        let response = read_http_response(stream);
        shutdown.store(true, Ordering::Relaxed);
        let _ = TcpStream::connect(address);
        server.join().expect("server should stop cleanly");

        assert!(response.starts_with("HTTP/1.1 404 Not Found"));
        assert!(response.contains("GitHub user 'missing-user' was not found"));
    }
}
