FROM docker.io/flyway/flyway:10.7.1 AS migrations
COPY database/ /flyway/
CMD ["migrate"]

FROM docker.io/rust:1-slim-bookworm AS cargo-build
WORKDIR /src/

# Install dependencies
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked apt-get update && \
    apt-get install -y git libssl-dev pkg-config
# Install Rust toolchain
RUN rustup install stable && rustup default stable

# Copy and Build Code
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/src/target \
    CARGO_PROFILE_RELEASE_DEBUG=1 cargo build --release && \
    cp target/release/alerter / && \
    cp target/release/autopilot / && \
    cp target/release/driver / && \
    cp target/release/orderbook / && \
    cp target/release/refunder / && \
    cp target/release/solvers /

# Create an intermediate image to extract the binaries
FROM docker.io/debian:bookworm-slim AS intermediate
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked apt-get update && \
    apt-get install -y ca-certificates tini gettext-base && \
    apt-get clean

FROM intermediate AS alerter
COPY --from=cargo-build /alerter /usr/local/bin/alerter
ENTRYPOINT [ "alerter" ]

FROM intermediate AS autopilot
COPY --from=cargo-build /autopilot /usr/local/bin/autopilot
ENTRYPOINT [ "autopilot" ]

FROM intermediate AS driver
COPY --from=cargo-build /driver /usr/local/bin/driver
ENTRYPOINT [ "driver" ]

FROM intermediate AS orderbook
COPY --from=cargo-build /orderbook /usr/local/bin/orderbook
ENTRYPOINT [ "orderbook" ]

FROM intermediate AS refunder
COPY --from=cargo-build /refunder /usr/local/bin/refunder
ENTRYPOINT [ "refunder" ]

FROM intermediate AS solvers
COPY --from=cargo-build /solvers /usr/local/bin/solvers
ENTRYPOINT [ "solvers" ]

# Extract Binary
FROM intermediate
RUN apt-get update && \
    apt-get install -y build-essential cmake git zlib1g-dev libelf-dev libdw-dev libboost-dev libboost-iostreams-dev libboost-program-options-dev libboost-system-dev libboost-filesystem-dev libunwind-dev libzstd-dev git
RUN git clone https://invent.kde.org/sdk/heaptrack.git /heaptrack && \
    mkdir /heaptrack/build && cd /heaptrack/build && \
    cmake -DCMAKE_BUILD_TYPE=Release -DBUILD_GUI=OFF .. && \
    make -j$(nproc) && \
    make install && \
    cd / && rm -rf /heaptrack
COPY --from=cargo-build /alerter /usr/local/bin/alerter
COPY --from=cargo-build /autopilot /usr/local/bin/autopilot
COPY --from=cargo-build /driver /usr/local/bin/driver
COPY --from=cargo-build /orderbook /usr/local/bin/orderbook
COPY --from=cargo-build /refunder /usr/local/bin/refunder
COPY --from=cargo-build /solvers /usr/local/bin/solvers
COPY ./entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["/entrypoint.sh"]
