FROM ubuntu:noble
ARG USERNAME
ARG REPO

ENV DEV_USERNAME="$USERNAME" DEV_REPO="$REPO"

# install zsh, sudo and git for convenience
ARG DEBIAN_FRONTEND=noninteractive
RUN set -eux; apt update; apt full-upgrade -y; \
    apt install -y ca-certificates libc6-dev pkg-config libssl-dev \
        build-essential zsh sudo git wget; \
    apt autoremove -y; apt clean; rm -rf /var/lib/apt/lists/*

# install llvm
RUN set -eux; \
    wget -qO- https://apt.llvm.org/llvm-snapshot.gpg.key | tee /etc/apt/trusted.gpg.d/apt.llvm.org.asc ; \
    echo "deb http://apt.llvm.org/noble/ llvm-toolchain-noble-21 main" | tee /etc/apt/sources.list.d/llvm.list ; \
    apt update; apt install -y \
        libllvm-21-ocaml-dev libllvm21 llvm-21 llvm-21-dev llvm-21-doc llvm-21-examples llvm-21-runtime \
        clang-21 clang-tools-21 clang-21-doc libclang-common-21-dev libclang-21-dev libclang1-21 clang-format-21 python3-clang-21 clangd-21 clang-tidy-21 \
        libclang-rt-21-dev libpolly-21-dev libfuzzer-21-dev lldb-21 lld-21 libc++-21-dev libc++abi-21-dev libomp-21-dev \
        libclc-21-dev libunwind-21-dev libmlir-21-dev mlir-21-tools libbolt-21-dev bolt-21 flang-21 libllvmlibc-21-dev; \
    apt autoremove -y; apt clean; rm -rf /var/lib/apt/lists/*
ENV PATH="/usr/lib/llvm-21/bin:$PATH"

# install zlib and zstd libraries
RUN set -eux; \
    apt update; apt install -y libzstd-dev zlib1g-dev; \
    apt autoremove -y; apt clean; rm -rf /var/lib/apt/lists/*

# delete default user on new ubuntu images
RUN set -eux; \
    if getent passwd ubuntu; then \
        touch /var/mail/ubuntu && \
        chown ubuntu /var/mail/ubuntu && \
        userdel -r ubuntu; \
    fi

# create non-root sudo user
RUN set -eux; \
    useradd --create-home --user-group --no-log-init "$DEV_USERNAME"; \
    echo "$USERNAME ALL=(root) NOPASSWD:ALL" > "/etc/sudoers.d/$DEV_USERNAME"; \
    chmod 0440 "/etc/sudoers.d/$DEV_USERNAME"

# create workspace dir and fix ownership
RUN set -eux; \
    mkdir -p "$DEV_REPO"; \
    chown -R "$DEV_USERNAME:$DEV_USERNAME" "$DEV_REPO" "/home/$DEV_USERNAME"
WORKDIR "$DEV_REPO"

# change user
USER "$DEV_USERNAME"

# install rust
ARG TOOLCHAIN=nightly
RUN set -eux; wget -qO - https://sh.rustup.rs | sh -s -- -y --default-toolchain "$TOOLCHAIN"
ENV PATH="/home/$DEV_USERNAME/.local/bin:/home/$DEV_USERNAME/.cargo/bin:$PATH"

# install pest LSP
RUN set -eux; cargo install pest-language-server

# replace entrypoint
CMD sleep infinity
