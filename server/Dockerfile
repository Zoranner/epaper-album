# Stage 1: build Web admin UI.
FROM oven/bun:1 AS web-builder
WORKDIR /app/web
COPY web/package.json web/bun.lock ./
RUN bun install --frozen-lockfile
COPY web/ .
RUN bun run build

# Stage 2: prepare Rust dependency recipe.
FROM rust:1-slim AS chef
RUN cargo install cargo-chef --locked
WORKDIR /app

FROM chef AS planner
COPY Cargo.toml Cargo.lock build.rs ./
COPY src/ ./src/
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: build backend.
FROM chef AS backend-builder
RUN apt-get update && apt-get install -y \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*
COPY --from=planner /app/recipe.json recipe.json
RUN SKIP_FRONTEND_BUILD=1 cargo chef cook --release --recipe-path recipe.json
COPY Cargo.toml Cargo.lock build.rs ./
COPY src/ ./src/
RUN SKIP_FRONTEND_BUILD=1 cargo build --release

# Stage 4: runtime image.
FROM debian:bookworm-slim AS runtime
RUN useradd --system --no-create-home --uid 1000 epaper
WORKDIR /app
COPY --from=backend-builder /app/target/release/epaper-album-server .
COPY --from=web-builder /app/web/dist ./web/dist
COPY assets/ ./assets/
RUN mkdir -p /app/data/origin /app/data/display && chown -R epaper:epaper /app/data
VOLUME ["/app/data"]
EXPOSE 3000
ENV LISTEN_PORT=3000
ENV DATABASE_URL=sqlite:data/epaper-album.db?mode=rwc
USER epaper
CMD ["./epaper-album-server"]
