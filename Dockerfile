FROM docker.io/flyway/flyway:10.7.1 as migrations
COPY database/ /flyway/
CMD ["migrate"]

FROM docker.io/rust:1-slim-bookworm as rust-chef
# Install Rust toolchain
RUN rustup install stable && rustup default stable
# Install dependencies
RUN --mount=type=cache,id=apt,target=/var/cache/apt,sharing=locked apt-get update && \
    apt-get install -y git libssl-dev pkg-config
#RUN cargo install --locked cargo-chef sccache
ENV RUSTC_WRAPPER=sccache SCCACHE_DIR=/sccache

#FROM rust-chef AS planner
#WORKDIR /src/
#COPY . .
#RUN CARGO_PROFILE_RELEASE_DEBUG=1 cargo chef prepare --recipe-path recipe.json

FROM rust-chef as cargo-build
WORKDIR /src/

# Compile deps
#COPY --from=planner /src/recipe.json recipe.json
#RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
#    --mount=type=cache,id=cargo-target,target=/src/target,sharing=locked \
#    CARGO_PROFILE_RELEASE_DEBUG=1 cargo chef cook --release --recipe-path recipe.json

# Copy and Build Code
COPY . .
RUN --mount=type=cache,id=sccache,target=/sccache,sharing=locked \
    CARGO_PROFILE_RELEASE_DEBUG=1 cargo build --release && \
    cp target/release/alerter / && \
    cp target/release/autopilot / && \
    cp target/release/driver / && \
    cp target/release/orderbook / && \
    cp target/release/refunder / && \
    cp target/release/solvers /

# Create an intermediate image to extract the binaries
FROM docker.io/debian:bookworm-slim as intermediate
RUN --mount=type=cache,id=apt,target=/var/cache/apt,sharing=locked apt-get update && \
    apt-get install -y ca-certificates tini gettext-base && \
    apt-get clean

FROM intermediate as alerter
COPY --from=cargo-build /alerter /usr/local/bin/alerter
ENTRYPOINT [ "alerter" ]

FROM intermediate as autopilot
COPY --from=cargo-build /autopilot /usr/local/bin/autopilot
ENTRYPOINT [ "autopilot" ]

FROM intermediate as driver
COPY --from=cargo-build /driver /usr/local/bin/driver
ENTRYPOINT [ "driver" ]

FROM intermediate as orderbook
COPY --from=cargo-build /orderbook /usr/local/bin/orderbook
ENTRYPOINT [ "orderbook" ]

FROM intermediate as refunder
COPY --from=cargo-build /refunder /usr/local/bin/refunder
ENTRYPOINT [ "refunder" ]

FROM intermediate as solvers
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

CMD echo "Specify binary..."
ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["/entrypoint.sh"]