import json
import os
from http.server import BaseHTTPRequestHandler, HTTPServer
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen


def get_public_gists(user):
    request = Request(
        f"https://api.github.com/users/{user}/gists",
        headers={
            "Accept": "application/vnd.github+json",
            "X-GitHub-Api-Version": "2022-11-28",
            "User-Agent": "equal-experts-operability-assignment",
        },
    )

    try:
        with urlopen(request) as response:
            gists = json.load(response)
    except HTTPError as error:
        if error.code == 404:
            raise ValueError(f"GitHub user '{user}' was not found") from error
        raise RuntimeError(f"GitHub API returned status {error.code}") from error
    except URLError as error:
        raise RuntimeError(f"GitHub API request failed: {error.reason}") from error

    return [
        {
            "id": gist["id"],
            "description": gist["description"],
            "html_url": gist["html_url"],
            "files": sorted(gist["files"].keys()),
        }
        for gist in gists
    ]


class GistHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        status, body = handle_request(self.path, get_public_gists)
        self.send_json(status, body)

    def log_message(self, format, *args):
        return

    def send_json(self, status, body):
        payload = json.dumps(body).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)


def run():
    port = int(os.environ.get("PORT", "8080"))
    server = HTTPServer(("0.0.0.0", port), GistHandler)
    print(f"listening on http://0.0.0.0:{port}")
    server.serve_forever()


def handle_request(path, gist_fetcher):
    user = path.lstrip("/")

    if not user or "/" in user:
        return 400, {"error": "Path must be /<USER>"}

    try:
        return 200, gist_fetcher(user)
    except ValueError as error:
        return 404, {"error": str(error)}
    except RuntimeError as error:
        return 502, {"error": str(error)}


if __name__ == "__main__":
    run()
