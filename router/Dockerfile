FROM rust:1 AS build
WORKDIR /src
COPY . .
RUN cargo build --release --example backend

FROM debian:trixie-slim
ARG VERSION=v1
ENV VERSION=$VERSION
COPY --from=build /src/target/release/examples/backend /usr/local/bin/backend
EXPOSE 8080
ENTRYPOINT ["sh", "-c", "exec backend 8080 \"$VERSION\""]
