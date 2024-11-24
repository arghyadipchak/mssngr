FROM rust:latest AS build

WORKDIR /app

COPY . .

RUN cargo build --release

FROM debian:latest

WORKDIR /app

RUN \
  apt-get update && \
  apt-get install -y --no-install-recommends \
  ca-certificates tzdata && \
  rm -rf /var/lib/apt/lists/*

COPY --from=build /app/target/release/mssngr .

EXPOSE 8080

CMD ["./mssngr"]
