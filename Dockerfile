FROM rust:slim-trixie AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock build.rs ./
COPY proto ./proto
COPY src ./src
RUN cargo build --release

FROM debian:trixie-slim
COPY --from=build /app/target/release/withhuman /usr/local/bin/withhuman
EXPOSE 8080 9000
CMD ["withhuman"]
