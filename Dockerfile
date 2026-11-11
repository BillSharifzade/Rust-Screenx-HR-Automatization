FROM rust:1.90-slim-bookworm AS builder
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev curl unzip \
    && rm -rf /var/lib/apt/lists/*

ENV SQLX_OFFLINE=true

# Prime the dependency cache and provide SQLx offline data
COPY Cargo.toml Cargo.lock ./
COPY .sqlx ./.sqlx/
RUN mkdir -p src && echo "fn main() {}" > src/main.rs
RUN cargo build --release && rm -rf src

# Copy application sources and perform the final build
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim AS runtime
WORKDIR /app

# poppler-utils and libreoffice-writer read CVs (PDF / DOC / DOCX / ODT).
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        fonts-liberation \
        poppler-utils \
        libreoffice-writer \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/recruitment-backend /usr/local/bin/recruitment-backend
COPY --from=builder /app/migrations ./migrations
COPY --from=builder /app/scripts ./scripts

ENV SERVER_ADDRESS=0.0.0.0:8080 \
    RUST_LOG=info

EXPOSE 8080

CMD ["/usr/local/bin/recruitment-backend"]