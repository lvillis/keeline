# Images Layout

This directory stores image metadata and generated Dockerfiles consumed by the
Rust CLI.

- `images/debian/<debian-major>/` stores Debian base image contexts.
- `images/jdk/<jdk-major>/<debian-line>/` stores JDK image contexts on a
  Debian line.
- Each image context must include an `image.toml`. That file defines package
  metadata, tags, upstream tarballs, checksums, and variant settings.
- Variants render beside the default Dockerfile, for example `Dockerfile` and
  `slim.Dockerfile`.
- Generated Dockerfiles must not be edited by hand. Use `cargo run -- render`
  after changing metadata, then `cargo run -- verify` to confirm the tree is in
  sync.
