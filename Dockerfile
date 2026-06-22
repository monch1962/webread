FROM scratch
COPY webread /webread
ENTRYPOINT ["/webread"]
