# Operability Take-Home Exercise

This solution provides a small HTTP API that returns the public GitHub gists for a requested user.
The implementation is intentionally simple and uses only Python's standard library.

## Overview

- Language: Python
- HTTP server: Python standard library `http.server`
- GitHub integration: Python standard library `urllib.request`
- Container: Docker

The API accepts requests in the form `GET /<USER>` and returns a JSON array of that GitHub user's public gists.

Each gist in the response includes:

- `id`
- `description`
- `html_url`
- `files` as a sorted list of file names

## Project files

- `app.py`: application entry point and HTTP request handling
- `test_app.py`: automated tests
- `Dockerfile`: container image definition
- `.gitignore`: ignores local-only files such as caches and IDE folders

## Requirements

- Python 3
- Docker, only if you want to run the container locally

## Run locally

```bash
python app.py
```

The API listens on `http://localhost:8080`.
If `8080` is already in use locally, you can override it with `PORT` while keeping the default behavior unchanged:

- PowerShell: `$env:PORT='8088'; python app.py`
- Bash: `PORT=8088 python app.py`

Example request:

```bash
curl http://localhost:8080/octocat
```

Example success response:

```json
[
  {
    "id": "6cad326836d38bd3a7ae",
    "description": "Hello world!",
    "html_url": "https://gist.github.com/octocat/6cad326836d38bd3a7ae",
    "files": ["hello_world.rb"]
  }
]
```

Example not found response:

```json
{
  "error": "GitHub user 'missing-user' was not found"
}
```

## Run tests

```bash
python -m unittest
```

The tests verify:

- a successful request using `octocat`-style data
- a missing-user response that returns `404`

## Build and run with Docker

```bash
docker build -t operability-assignment .
docker run --rm -p 8080:8080 operability-assignment
```

If port `8080` is already in use on your machine, you can still test the container by mapping a different host port:

```bash
docker run --rm -p 8088:8080 operability-assignment
```

## API behaviour

- `GET /{user}` returns a JSON array of public gists for that GitHub user.
- Each gist includes its `id`, `description`, `html_url`, and a sorted list of file names.
- Invalid paths return `400`.
- Unknown GitHub users return `404`.
- Unexpected GitHub API failures return `502`.

## Notes on implementation

- The app exposes `GET /{user}` and returns a simplified list of that user's public gists.
- GitHub API calls are made with `urllib.request`.
- Tests verify the endpoint behavior without depending on live GitHub access.
- The default port is `8080` to match the exercise, with an optional local override via `PORT`.
