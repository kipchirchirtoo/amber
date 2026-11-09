# Amber Stores server image.
# Build from the project root:  docker build -f deploy/Dockerfile -t amber-server .
# (docker compose does this for you; see deploy/README.md)

FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY engine engine
COPY cli cli
COPY server server
RUN cargo build --release -p amber_server

FROM debian:bookworm-slim
# libssl3 + ca-certificates: the server talks HTTPS to the AI service and can use TLS to a managed database.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --no-create-home amber
COPY --from=build /src/target/release/amber_server /usr/local/bin/amber_server
# Supabase's public root certificate, so the database connection can be verified.
COPY deploy/supabase-ca.crt /etc/amber/supabase-ca.crt
USER amber
ENV BIND=0.0.0.0:8080
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 CMD ["amber_server", "healthcheck"]
CMD ["amber_server"]
