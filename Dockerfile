# Build stage
FROM rust:slim-bookworm as builder

WORKDIR /usr/src/app
# Copy manifest dan source code
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY dashboard ./dashboard
COPY migrations ./migrations

# Install dependencies untuk compile (termasuk OpenSSL jika diperlukan sqlx/rumqttc)
RUN apt-get update && apt-get install -y pkg-config libssl-dev

# Build rilis
RUN cargo build --locked --release --bin backend-iot

# Production stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime SSL (dibutuhkan untuk konek ke Azure PostgreSQL & EMQX Mqtt)
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*

# Copy hasil build dari stage pertama
COPY --from=builder /usr/src/app/target/release/backend-iot /app/backend-iot
COPY --from=builder /usr/src/app/dashboard /app/dashboard

# Set environment
ENV PORT=80
EXPOSE 80

# Jalankan server
CMD ["./backend-iot"]