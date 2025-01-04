# Step 1: Build the Rust app in a build container
FROM messense/rust-musl-cross:x86_64-musl as builder

# Set the working directory
WORKDIR /app

# Copy the Cargo.toml and Cargo.lock files to cache dependencies
COPY Cargo.toml Cargo.lock ./

# Download the dependencies (this will be cached if there are no changes to Cargo.toml)
RUN mkdir -p src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release --target x86_64-unknown-linux-musl
RUN rm src/*.rs && find ./target -name "owntracks*exporter*rs*" | xargs rm -rf

# Copy the source code into the container
COPY ./src ./src

# Build the application in release mode with static linking
RUN cargo build --release --target x86_64-unknown-linux-musl

# Step 2: Create a minimal runtime image using scratch
FROM scratch

# Copy the compiled binary from the builder image
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/owntracks-exporter-rs /usr/local/bin/owntracks-exporter-rs

# Set the command to run your application
CMD ["/usr/local/bin/owntracks-exporter-rs"]
