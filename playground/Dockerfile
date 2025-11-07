FROM debian:bookworm AS chef
WORKDIR /src/
RUN apt-get update && apt-get install -y curl git clang mold libssl-dev pkg-config git && apt-get clean
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="$PATH:/root/.cargo/bin"
RUN rustup component add clippy rustfmt
# configure to use the fast linker
RUN echo "\
[build]\n \
target = \"$(rustc -Vv | grep host | awk '{print $2}')\"\n \
rustflags = [\"-C\", \"link-arg=-fuse-ld=/usr/bin/mold\", \"--cfg\", \"tokio_unstable\"]\n \
[target.$(rustc -Vv | grep host | awk '{print $2}')]\n \
linker = \"clang\"\n \
" > ~/.cargo/config.toml

RUN cargo install cargo-chef

FROM chef AS localdev
# envs/tools suitable for running the stack locally
ENV RUST_BACKTRACE=1
RUN cargo install cargo-watch

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM docker.io/flyway/flyway:10.7.1 AS migrations
COPY database/ /flyway/
CMD ["migrate"]

FROM chef AS builder
COPY --from=planner /src/recipe.json recipe.json
COPY --from=chef /.cargo /.cargo
RUN CARGO_PROFILE_RELEASE_DEBUG=1 cargo chef cook --release --recipe-path recipe.json

# Copy only the library crates for now
COPY --from=chef /.cargo /.cargo
COPY Cargo.toml Cargo.lock ./
COPY ./crates/app-data/ ./crates/app-data
COPY ./crates/database/ ./crates/database
COPY ./crates/bytes-hex/ ./crates/bytes-hex
COPY ./crates/contracts/ ./crates/contracts
COPY ./crates/shared/ ./crates/shared
COPY ./crates/number/ ./crates/number
COPY ./crates/model/ ./crates/model
COPY ./crates/rate-limit/ ./crates/rate-limit
COPY ./crates/chain/ ./crates/chain
COPY ./crates/ethrpc/ ./crates/ethrpc
COPY ./crates/observe/ ./crates/observe
COPY ./crates/order-validation/ ./crates/order-validation
RUN CARGO_PROFILE_RELEASE_DEBUG=1 cargo build --release --package shared
COPY ./crates/solver/ ./crates/solver
RUN CARGO_PROFILE_RELEASE_DEBUG=1 cargo build --release --package solver

# Create an base image for all the binaries
FROM docker.io/debian:bookworm-slim AS base
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked apt-get update && \
    apt-get install -y ca-certificates tini gettext-base && \
    apt-get clean

FROM builder AS alerter-build
COPY --from=chef /.cargo /.cargo
COPY ./crates/alerter/ ./crates/alerter
RUN CARGO_PROFILE_RELEASE_DEBUG=1 cargo build --release --package alerter

FROM base AS alerter
COPY --from=alerter-build /src/target/release/alerter /usr/local/bin/alerter
ENTRYPOINT [ "alerter" ]

FROM builder AS autopilot-build
RUN CARGO_PROFILE_RELEASE_DEBUG=1 cargo build --release --package autopilot

FROM base AS autopilot
COPY --from=chef /.cargo /.cargo
COPY ./crates/autopilot/ ./crates/autopilot
COPY --from=autopilot-build /src/target/release/autopilot /usr/local/bin/autopilot
ENTRYPOINT [ "autopilot" ]

FROM builder AS driver-build
COPY ./crates/driver/ ./crates/driver
RUN CARGO_PROFILE_RELEASE_DEBUG=1 cargo build --release --package driver

FROM base AS driver
COPY --from=driver-build /src/target/release/driver /usr/local/bin/driver
ENTRYPOINT [ "driver" ]

FROM builder AS orderbook-build
COPY --from=chef /.cargo /.cargo
COPY ./crates/orderbook/ ./crates/orderbook
RUN CARGO_PROFILE_RELEASE_DEBUG=1 cargo build --release --package orderbook

FROM base AS orderbook
COPY --from=orderbook-build /src/target/release/orderbook /usr/local/bin/orderbook
ENTRYPOINT [ "orderbook" ]

FROM builder AS refunder-build
COPY --from=chef /.cargo /.cargo
COPY ./crates/refunder/ ./crates/refunder
RUN CARGO_PROFILE_RELEASE_DEBUG=1 cargo build --release --package refunder

FROM base AS refunder
COPY --from=refunder-build /src/target/release/refunder /usr/local/bin/refunder
ENTRYPOINT [ "refunder" ]

FROM builder AS solvers-build
COPY --from=chef /.cargo /.cargo
COPY ./crates/solvers/ ./crates/solvers
RUN CARGO_PROFILE_RELEASE_DEBUG=1 cargo build --release --package solvers

FROM base AS solvers
COPY --from=solvers-build /src/target/release/solvers /usr/local/bin/solvers
ENTRYPOINT [ "solvers" ]

# Extract Binary
FROM base
RUN apt-get update && \
    apt-get install -y build-essential cmake git zlib1g-dev libelf-dev libdw-dev libboost-dev libboost-iostreams-dev libboost-program-options-dev libboost-system-dev libboost-filesystem-dev libunwind-dev libzstd-dev git
RUN git clone https://invent.kde.org/sdk/heaptrack.git /heaptrack && \
    mkdir /heaptrack/build && cd /heaptrack/build && \
    cmake -DCMAKE_BUILD_TYPE=Release -DBUILD_GUI=OFF .. && \
    make -j$(nproc) && \
    make install && \
    cd / && rm -rf /heaptrack
COPY --from=alerter-build /src/target/release/alerter /usr/local/bin/alerter
COPY --from=autopilot-build /src/target/release/autopilot /usr/local/bin/autopilot
COPY --from=driver-build /src/target/release/driver /usr/local/bin/driver
COPY --from=orderbook-build /src/target/release/orderbook /usr/local/bin/orderbook
COPY --from=refunder-build /src/target/release/refunder /usr/local/bin/refunder
COPY --from=solvers-build /src/target/release/solvers /usr/local/bin/solvers
COPY ./entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["/entrypoint.sh"]
