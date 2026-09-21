USER root

# JDK 25
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        openjdk-25-jdk \
        curl \
        ca-certificates \
        build-essential \
        pkg-config \
        libclang-dev \
    && rm -rf /var/lib/apt/lists/*

USER yolo

# Rust toolchain owned by the yolo user
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --default-toolchain stable

RUN git config --global user.name "Kelvin Tan"
RUN git config --global user.email "superkelvint@gmail.com"

ENV PATH="/home/yolo/.cargo/bin:${PATH}"
ENV CARGO_TARGET_DIR=target/devcontainer
ENV SEARCHKERNEL_BUILD_ENV=devcontainer
