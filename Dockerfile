FROM debian:10 as build-env
USER root
RUN apt-get update && apt-get install -y libssl-dev pkg-config git curl clang libclang-dev

ARG rust_toolchain="nightly-2021-08-04"
RUN curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain ${rust_toolchain} -y
ENV PATH=/root/.cargo/bin:$PATH

COPY . /opt

RUN cd /opt/src && cargo build --release

ENTRYPOINT [ "/opt/target/release/tezedge-snapshots" ]