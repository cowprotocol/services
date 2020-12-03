FROM clux/muslrust as cargo-build
WORKDIR /usr/src/oba-services

# Copy and Build Code
COPY . .
RUN cargo build --target x86_64-unknown-linux-musl --release

# Extract Binary
FROM alpine:latest

# Handle signal handlers properly
RUN apk add --no-cache tini
COPY --from=cargo-build /usr/src/oba-services/target/x86_64-unknown-linux-musl/release/orderbook /usr/local/bin/orderbook
COPY --from=cargo-build /usr/src/oba-services/target/x86_64-unknown-linux-musl/release/solver /usr/local/bin/solver

CMD echo "Specify binary - either solver or orderbook"
ENTRYPOINT ["/sbin/tini", "--"]
