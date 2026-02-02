# see https://dioxuslabs.com/learn/0.7/tutorial/deploy#building-a-dockerfile

FROM rust:1 AS chef
RUN cargo install cargo-chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo +nightly-2025-08-14 chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo +nightly-2025-08-14 chef cook --release --recipe-path recipe.json
COPY . .

# Install `dx`
RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
RUN cargo binstall dioxus-cli --root /.cargo -y --force
ENV PATH="/.cargo/bin:$PATH"

# Create the final bundle folder. Bundle with release build profile to enable optimizations.
RUN dx bundle --web --release

FROM chef AS runtime
COPY --from=builder /app/target/dx/filen-relay/release/web/ /usr/local/app

# set our port and make sure to listen for all connections
ENV PORT=80
ENV IP=0.0.0.0

# expose the port 80
EXPOSE 80

WORKDIR /usr/local/app
ENTRYPOINT [ "/usr/local/app/filen-relay" ]