FROM rust AS builder

WORKDIR /code
COPY src ./src
COPY Cargo.lock .
COPY Cargo.toml .

RUN apt update
RUN apt install -y libdbus-1-dev pkg-config
RUN cargo build --release
RUN ls /code/target/release

FROM rust AS runner
COPY --from=builder /code/target/release/govee-exporter /usr/local/bin/govee-exporter
RUN apt update
RUN apt install -y libdbus-1-3

ENTRYPOINT /usr/local/bin/govee-exporter

