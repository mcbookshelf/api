FROM rust:latest as builder
WORKDIR /usr/src/app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

FROM debian:bullseye-slim
COPY --from=builder /usr/src/app/target/release/bookshelf-api /usr/local/bin/
RUN chmod +x bookshelf-api
EXPOSE 8080
CMD ["bookshelf-api"]
