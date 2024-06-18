FROM rust:latest

RUN rustup target add x86_64-unknown-linux-musl

RUN apt-get -y update
RUN apt-get install -y --no-install-recommends \
    musl-tools \
    musl-dev

WORKDIR /build

COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

COPY ./crates ./crates

# Currently, we only provide a build for the CLI
RUN cargo build --bin cli --target=x86_64-unknown-linux-musl --release

