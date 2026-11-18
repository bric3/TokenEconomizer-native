FROM rust:1.100-alpine@sha256:8d0e82abfdf9a38071e07cabc5afba4ddbe077631a80e4e742a760c38beadc33 AS build
RUN apk add --no-cache build-base
WORKDIR /work
ARG TARGETARCH
COPY Cargo.toml Cargo.lock LICENSE ./
COPY src/ src/
COPY validation/console_probe.rs validation/console_probe.rs
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,id=te-helper-target-${TARGETARCH},target=/work/target cargo build --locked --release --bin te-net-guard
RUN --mount=type=cache,id=te-helper-target-${TARGETARCH},target=/work/target install -D -m 0755 target/release/te-net-guard /out/te-net-guard
RUN rustc -vV > /out/toolchain.txt

FROM scratch
COPY --from=build /out/ /
