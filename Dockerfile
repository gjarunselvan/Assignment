FROM rust:1.86 AS builder
WORKDIR /app

COPY Cargo.toml .
COPY Cargo.lock .
COPY src ./src

RUN cargo build --release

FROM debian:bookworm-slim
WORKDIR /app

RUN apt-get update \
    && apt-get install --yes --no-install-recommends curl ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/assignment /usr/local/bin/assignment

EXPOSE 8080

CMD ["assignment"]
