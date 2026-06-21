# Stage 1: build Web admin UI.
FROM oven/bun:1 AS web-builder
WORKDIR /app/web
COPY server/web/package.json server/web/bun.lock ./
RUN bun install --frozen-lockfile
COPY server/web/index.html server/web/tsconfig.json server/web/tsconfig.node.json server/web/vite.config.ts ./
COPY server/web/src/ ./src/
RUN test -f src/App.vue \
    && test -f src/components/input/Input.vue \
    && test -f src/components/input/NumberInput.vue \
    && test -f src/components/input/Select.vue \
    && test -f src/components/layout/ModuleToolbar.vue \
    && test -f src/components/navigation/ActionMenu.vue \
    && test -f src/components/overlay/Dialog.vue \
    && test -f src/components/overlay/DialogActions.vue \
    && find src -maxdepth 3 -type f | sort
RUN bun run vite build

# Stage 2: prepare Rust dependency recipe.
FROM rust:1-slim AS chef
RUN cargo install cargo-chef --locked
WORKDIR /app

FROM chef AS planner
COPY crates/protocol/ /app/crates/protocol/
COPY server/Cargo.toml server/Cargo.lock server/build.rs /app/server/
COPY server/src/ /app/server/src/
WORKDIR /app/server
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: build backend.
FROM chef AS backend-builder
RUN apt-get update && apt-get install -y \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*
COPY --from=planner /app/server/recipe.json /app/server/recipe.json
WORKDIR /app/server
RUN SKIP_FRONTEND_BUILD=1 cargo chef cook --release --recipe-path recipe.json
COPY crates/protocol/ /app/crates/protocol/
COPY server/Cargo.toml server/Cargo.lock server/build.rs ./
COPY server/src/ ./src/
RUN SKIP_FRONTEND_BUILD=1 cargo build --release

# Stage 4: runtime image.
FROM debian:bookworm-slim AS runtime
RUN useradd --system --no-create-home --uid 1000 epaper
WORKDIR /app
COPY --from=backend-builder /app/server/target/release/inkframe-server .
COPY --from=web-builder /app/web/dist ./web/dist
COPY server/assets/ ./assets/
RUN mkdir -p /app/data/images/original /app/data/images/display /app/data/sprites && chown -R epaper:epaper /app/data
VOLUME ["/app/data"]
EXPOSE 3000
ENV INKFRAME_PRODUCTION=true
ENV LISTEN_HOST=0.0.0.0
ENV LISTEN_PORT=3000
ENV DATABASE_URL=sqlite:data/inkframe.db?mode=rwc
USER epaper
CMD ["./inkframe-server"]
