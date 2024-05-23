FROM docker.io/flyway/flyway:10.7.1 as migrations
COPY database/ /flyway/
CMD ["migrate"]

FROM docker.io/rust:1-slim-bookworm as cargo-build
WORKDIR /src/

# Install dependencies
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked apt-get update && \
    apt-get install -y git libssl-dev pkg-config git

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
FROM docker.io/debian:bookworm-slim as intermediate
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked apt-get update && \
    apt-get install -y ca-certificates tini && \
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
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked apt-get update && \
    apt-get install -y valgrind && \
    apt-get clean
COPY --from=cargo-build /orderbook /usr/local/bin/orderbook
ENTRYPOINT ["valgrind", "--tool=massif", "/usr/local/bin/orderbook"]

FROM intermediate as refunder
COPY --from=cargo-build /refunder /usr/local/bin/refunder
ENTRYPOINT [ "refunder" ]

FROM intermediate as solvers
COPY --from=cargo-build /solvers /usr/local/bin/solvers
ENTRYPOINT [ "solvers" ]

# Extract Binary
FROM intermediate
COPY --from=cargo-build /alerter /usr/local/bin/alerter
COPY --from=cargo-build /autopilot /usr/local/bin/autopilot
COPY --from=cargo-build /driver /usr/local/bin/driver
COPY --from=cargo-build /orderbook /usr/local/bin/orderbook
COPY --from=cargo-build /refunder /usr/local/bin/refunder
COPY --from=cargo-build /solvers /usr/local/bin/solvers

CMD echo "Specify binary..."
ENTRYPOINT ["/usr/bin/tini", "--"]
