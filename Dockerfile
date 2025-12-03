# builder 
FROM rust:1-bookworm AS builder

WORKDIR /app

COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

RUN cargo build --release

RUN rm src/*.rs
COPY ./src ./src

RUN rm ./target/release/deps/yorjik*
RUN cargo build --release

# runner
FROM debian:bookworm-slim AS runner

WORKDIR /app

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    sqlite3 && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/yorjik ./yorjik

CMD ["./yorjik"]