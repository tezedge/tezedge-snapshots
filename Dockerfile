FROM debian:10 as build-env
USER root
RUN apt-get update && apt-get install -y libssl-dev pkg-config git curl clang libclang-dev

ARG rust_toolchain="nightly-2021-08-04"
RUN curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain ${rust_toolchain} -y
ENV PATH=/root/.cargo/bin:$PATH

ARG tezedge_snapshots_git="https://github.com/tezedge/tezedge-snapshots.git"

RUN git clone ${tezedge_snapshots_git} && cd tezedge-snapshots && pwd && cargo build --release

FROM gcr.io/distroless/cc-debian10

COPY --from=build-env /tezedge-snapshots/target/release/tezedge-snapshots /

ENTRYPOINT [ "/tezedge-snapshots" ]
