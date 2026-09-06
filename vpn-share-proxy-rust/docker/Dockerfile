FROM messense/cargo-zigbuild:latest AS builder

RUN rustup target add aarch64-unknown-linux-musl

WORKDIR /workspace

COPY Cargo.toml ./
COPY src ./src
COPY docs ./docs

RUN cargo zigbuild --target aarch64-unknown-linux-musl --release && \
    mkdir -p /dist && \
    cp target/aarch64-unknown-linux-musl/release/vspd /dist/ && \
    cp target/aarch64-unknown-linux-musl/release/vsp-ctl /dist/

CMD ["cp", "-r", "/dist/.", "/output/"]
