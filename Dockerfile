FROM docker.io/flyway/flyway:10.7.1 AS migrations
COPY database/ /flyway/
CMD ["migrate"]

FROM docker.io/rust:1-slim-bookworm AS cargo-build
WORKDIR /src/

# Accept build arguments for enabling features
ARG CARGO_BUILD_FEATURES=""
ARG RUSTFLAGS=""

# Install dependencies
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked apt-get update && \
    apt-get install -y git libssl-dev pkg-config build-essential
# Install Rust toolchain
RUN rustup install stable && rustup default stable

# Copy and Build Code
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry --mount=type=cache,target=/src/target \
    CARGO_PROFILE_RELEASE_DEBUG=1 RUSTFLAGS="${RUSTFLAGS}" cargo build --release ${CARGO_BUILD_FEATURES} && \
    cp target/release/autopilot / && \
    cp target/release/driver / && \
    cp target/release/orderbook / && \
    cp target/release/refunder / && \
    cp target/release/solvers / && \
    cp target/release/euler-solver /

# Create an intermediate image to extract the binaries
FROM docker.io/debian:bookworm-slim AS intermediate
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked apt-get update && \
    apt-get install -y ca-certificates tini gettext-base && \
    apt-get clean

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

FROM intermediate AS euler-solver
COPY --from=cargo-build /euler-solver /usr/local/bin/euler-solver
ENTRYPOINT [ "euler-solver" ]

# Extract Binary
FROM intermediate

RUN apt-get update && apt-get install -y netcat-openbsd
COPY --from=cargo-build /autopilot /usr/local/bin/autopilot
COPY --from=cargo-build /driver /usr/local/bin/driver
COPY --from=cargo-build /orderbook /usr/local/bin/orderbook
COPY --from=cargo-build /refunder /usr/local/bin/refunder
COPY --from=cargo-build /solvers /usr/local/bin/solvers
COPY --from=cargo-build /euler-solver /usr/local/bin/euler-solver


ENTRYPOINT ["/usr/bin/tini", "-s", "--"]
