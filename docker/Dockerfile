FROM clux/muslrust:stable as cargo-build
WORKDIR /usr/src/oba-services

# Copy and Build Code
COPY . .
RUN cargo build --target x86_64-unknown-linux-musl --release

# Extract Binary
FROM alpine:latest

# Handle signal handlers properly
RUN apk add --no-cache tini postgresql
COPY --from=cargo-build /usr/src/oba-services/target/x86_64-unknown-linux-musl/release/orderbook /usr/local/bin/orderbook
COPY --from=cargo-build /usr/src/oba-services/target/x86_64-unknown-linux-musl/release/solver /usr/local/bin/solver

COPY database /usr/database

CMD echo "Specify binary - either solver or orderbook"
ENTRYPOINT ["/sbin/tini", "--"]
