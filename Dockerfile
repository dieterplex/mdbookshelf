# Source: https://github.com/hendrikmaus/rust-workflows

# https://github.com/GoogleContainerTools/distroless/blob/main/cc/README.md
FROM gcr.io/distroless/cc-debian11 as runtime

#
# build args used to determine which binary to use
#   these are passed in to the container image build in github actions
#   to manually build the image:
#
#   compile the binary:
#     cargo build --release --target x86_64-unknown-linux-musl
#
#   build the container image:
#     docker build \
#       --build-arg bin=rust-workflows \
#       --build-arg bin_path=target/x86_64-unknown-linux-musl/release/rust-workflows \
#       --tag image:tag .
#
ARG bin
ARG bin_path

# https://github.com/opencontainers/image-spec/blob/main/annotations.md
LABEL org.opencontainers.image.authors="d1t2 <dieterplex@gmail.com>"
LABEL org.opencontainers.image.url="https://github.com/dieterplex/mdbookshelf"

# uid 65532
USER nonroot
COPY ${bin_path:-"target/release/${bin}"} /usr/local/bin/app
CMD ["/usr/local/bin/app"]