FROM rust:bookworm as builder
WORKDIR /usr/src/app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /usr/src/app/target/release/bookshelf-api /usr/local/bin/
RUN apt update && \
    apt upgrade -y && \
    apt install -y openssl && \
    chmod +x /usr/local/bin/bookshelf-api
EXPOSE 3000
CMD ["bookshelf-api"]
