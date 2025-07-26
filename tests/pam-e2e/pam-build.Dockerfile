FROM rust:1.90-trixie AS builder

ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        libpam0g-dev \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /work
COPY Cargo.toml Cargo.lock ./
COPY bin bin
COPY crates crates
COPY tests tests

RUN cargo build --release --locked -p pam-socket

FROM scratch AS artifact
COPY --from=builder /work/target/release/libsnas_pam_socket.so /
