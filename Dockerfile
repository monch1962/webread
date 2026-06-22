# Minimal static image — no glibc, no busybox, just the binary
FROM scratch
COPY target/release/webread /webread
ENTRYPOINT ["/webread"]
