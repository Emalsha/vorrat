# Build: docker build -t vorrat .
# Run:   docker run -p 127.0.0.1:8087:8087 -v vorrat-data:/data vorrat
FROM rust:1.88-slim-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src src
COPY templates templates
COPY static static
COPY locales locales
RUN cargo build --release --locked && mkdir /data

FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=build /src/target/release/vorrat /usr/local/bin/vorrat
# The data folder must belong to the nonroot user (65532), or a fresh volume is read-only.
COPY --from=build --chown=65532:65532 /data /data
ENV VORRAT_DATA_DIR=/data VORRAT_LISTEN=0.0.0.0:8087
VOLUME /data
EXPOSE 8087
USER nonroot
ENTRYPOINT ["/usr/local/bin/vorrat"]
