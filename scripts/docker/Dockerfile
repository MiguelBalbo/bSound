# Ambiente para compilar o bSound para Linux (x86_64) e Windows (x86_64, MinGW).
FROM --platform=linux/amd64 rust:1-bookworm
RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config libasound2-dev libgtk-3-dev libxdo-dev libayatana-appindicator3-dev \
        mingw-w64 zip \
    && rm -rf /var/lib/apt/lists/*
RUN rustup target add x86_64-pc-windows-gnu
ENV CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc
WORKDIR /src
