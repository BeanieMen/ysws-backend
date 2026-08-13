# Stage 1: Build binary
FROM rust:1-alpine as builder
RUN apk add --no-cache musl-dev postgresql-dev

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY database ./database

RUN cargo build --release

# Stage 2: Minimal runtime image
FROM alpine:3.19
RUN apk add --no-cache ca-certificates libgcc postgresql-client

WORKDIR /app
COPY --from=builder /app/target/release/test-instance /app/test-instance
COPY database ./database

EXPOSE 8000
CMD ["/app/test-instance"]
