FROM rust:latest as builder
RUN apt-get update && apt-get install -y musl-tools
WORKDIR /usr/src/app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN rustup target add x86_64-unknown-linux-musl
RUN cargo build --release --target x86_64-unknown-linux-musl

FROM alpine:latest
RUN apk update && apk add --no-cache libssl3 && rm -rf /var/cache/apk/*
COPY --from=builder /usr/src/app/target/x86_64-unknown-linux-musl/release/bookshelf-api /usr/local/bin/
RUN chmod +x /usr/local/bin/bookshelf-api
EXPOSE 3000
CMD ["bookshelf-api"]
