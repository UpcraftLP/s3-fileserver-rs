# syntax=docker/dockerfile:1

ARG RUST_VERSION=1.77.0
ARG NODE_VERSION=20.9.0
ARG API_URL="http://localhost:3001"
ARG VERSION="development"

################################################################################
# Build the backend server

FROM rust:${RUST_VERSION}-alpine AS build
WORKDIR /app

# Install host build dependencies.
RUN apk add --no-cache \
    git \
    pkgconf \
    openssl-dev \
    clang \
    lld \
    musl-dev

ARG VERSION
ENV VERSION=$VERSION

# Build the backend application.
# Leverage a cache mount to /usr/local/cargo/registry/
# for downloaded dependencies, a cache mount to /usr/local/cargo/git/db
# for git repository dependencies, and a cache mount to /app/target/ for
# compiled dependencies which will speed up subsequent builds.
# Leverage a bind mount to the src directory to avoid having to copy the
# source code into the container. Once built, copy the executable to an
# output directory before the cache mounted /app/target is unmounted.
RUN --mount=type=bind,source=src,target=src \
    --mount=type=bind,source=Cargo.toml,target=Cargo.toml \
    --mount=type=bind,source=Cargo.lock,target=Cargo.lock \
    --mount=type=bind,source=build.rs,target=build.rs \
    --mount=type=cache,target=/app/target/ \
    --mount=type=cache,target=/usr/local/cargo/git/db \
    --mount=type=cache,target=/usr/local/cargo/registry/ \
RUSTFLAGS="-C target-feature=-crt-static" \
cargo build --locked --release && \
cp target/release/backend /bin/server

################################################################################
# Build the frontend SPA

FROM node:${NODE_VERSION}-alpine AS frontend
ARG API_URL
ENV API_URL=$API_URL
WORKDIR /usr/src/app

# Download dependencies as a separate step to take advantage of Docker's caching.
# Leverage a cache mount to /root/.npm to speed up subsequent builds.
# Leverage bind mounts to package.json and package-lock.json to avoid having to copy them
# into this layer.
RUN --mount=type=bind,source=frontend/package.json,target=package.json \
    --mount=type=bind,source=frontend/package-lock.json,target=package-lock.json \
    --mount=type=cache,target=/root/.npm \
    # npm ci --omit=dev
    npm ci

# Copy the rest of the source files into the image.
COPY ./frontend/ .
# Run the build script.
RUN npm run build

################################################################################

FROM alpine:3.18 AS final

RUN apk add --no-cache \
    ca-certificates \
    libgcc \
    openssl

# Create a non-privileged user that the app will run under.
# See https://docs.docker.com/go/dockerfile-user-best-practices/
ARG UID=10001
RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    appuser
USER appuser

ENV FRONTEND_PATH="/static"
ENV API_URL="http://localhost:3001"

# Copy the main executable.
COPY --from=build /bin/server /bin/

# Copy the frontend build.
COPY --from=frontend /usr/src/app/build /static

# Expose the port that the application listens on.
EXPOSE 3001

# What the container should run when it is started.
CMD ["/bin/server"]
