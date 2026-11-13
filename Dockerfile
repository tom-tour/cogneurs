# Game server + page in one image.
FROM rust:1-slim AS build
WORKDIR /app
RUN rustup target add wasm32-unknown-unknown
COPY . .
RUN cargo build --release -p cogneurs-server \
 && cargo build --release -p cogneurs-engine --target wasm32-unknown-unknown \
 && cp target/wasm32-unknown-unknown/release/cogneurs_engine.wasm web/engine.wasm

FROM debian:bookworm-slim
WORKDIR /app
COPY --from=build /app/target/release/cogneurs-server /app/cogneurs-server
COPY --from=build /app/web /app/web
ENV COGNEURS_DATA=/data/cogneurs.json
VOLUME /data
EXPOSE 8137
CMD ["/app/cogneurs-server"]
