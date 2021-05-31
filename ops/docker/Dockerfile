FROM rust:1.49-alpine3.12 as build
WORKDIR /build
RUN apk add musl-dev
COPY . .
RUN cargo build --release

FROM alpine:3.13.5 as rdy
COPY --from=build /build/target/release/s3exporter /s3exporter
ENTRYPOINT ["/s3exporter"]
