FROM rust:1.95-slim-bookworm AS builder

WORKDIR /usr/src/oxide-shield

COPY . .

RUN cargo build --release


FROM debian:bookworm-slim

RUN groupadd --gid 1000 oxide && useradd --uid 1000 --gid oxide --shell /bin/false oxide

COPY --from=builder /usr/src/oxide-shield/target/release/oxide-shield /usr/local/bin/oxide-shield

USER oxide

CMD [ "oxide-shield" ]