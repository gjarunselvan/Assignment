# Operability Take-Home Exercise

This solution provides a small HTTP API that returns the public GitHub gists for a requested user.
The implementation is intentionally minimal so it is easy to review.

## Requirements

- Rust toolchain
- Docker (optional, for containerised execution)
- `curl` available on the host if you run the binary locally

## Run locally

```bash
cargo run
```

The API listens on `http://localhost:8080`.
You can override the port locally if `8080` is already in use:

- PowerShell: `$env:PORT='18080'; cargo run`
- Bash: `PORT=18080 cargo run`

Example request:

```bash
curl http://localhost:8080/octocat
```

## Run tests

```bash
cargo test
```

## Build and run with Docker

```bash
docker build -t operability-assignment .
docker run --rm -p 8080:8080 operability-assignment
```

## API behaviour

- `GET /{user}` returns a JSON array of public gists for that GitHub user.
- Each gist includes its `id`, `description`, `html_url`, and a sorted list of file names.
- Unknown GitHub users return `404`.
- Unexpected GitHub API failures return `502`.

## Notes on implementation

- The app exposes `GET /{user}` and returns a simplified list of that user's public gists.
- GitHub API calls are performed with `curl`.
- Tests verify the endpoint logic without depending on live GitHub access.
