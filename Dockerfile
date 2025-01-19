FROM rust:bookworm AS builder
WORKDIR /usr/src/app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
RUN mkdir /app && \
    apt update && \
    apt upgrade -y && \
    apt install -y openssl git
COPY --from=builder /usr/src/app/target/release/bookshelf-api /app/
WORKDIR /app
RUN chmod +x /app/bookshelf-api && \
    git clone https://github.com/mcbookshelf/api-data.git /app/data
EXPOSE 3000
CMD ["./bookshelf-api"]
