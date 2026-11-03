# Build: docker build -t vc-server .   Run: docker run -p 8080:8080 -v vcdata:/data -e VC_ADMIN_TOKEN=... -e VC_HOST=log.example.com vc-server
FROM rust:1-slim AS build
WORKDIR /src
COPY . .
RUN cargo build --release -p vc-server
FROM debian:stable-slim
COPY --from=build /src/target/release/vc-server /usr/local/bin/vc-server
ENV VC_DATA=/data VC_ADDR=0.0.0.0:8080
VOLUME /data
EXPOSE 8080
USER 10001
CMD ["vc-server"]
