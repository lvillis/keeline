# Images Layout

This directory stores image metadata and generated Dockerfiles consumed by the
Rust CLI.

- `images/debian/<debian-major>/` stores Debian base image contexts.
- `images/java/<java-line>/<debian-line>/` stores Java image contexts on a
  Debian line.
- `images/scratch/<image-line>/` stores scratch-based tool image contexts.
- Each image context must include an `image.toml`. That file defines package
  metadata, tags, upstream tarballs, checksums, variant settings, and release
  state.
- Each image definition declares bundled runtime tools under `[tools.*]`.
  `role = "init"` drives the image entrypoint, `role = "healthcheck"` marks
  the health probe helper, and `role = "motd"` provides startup or MOTD-style
  template rendering.
- Keeline currently bundles `tino`, `salus`, and `motdyn` in every published
  image.
- Variants render beside the default Dockerfile, for example `Dockerfile` and
  `slim.Dockerfile`.
- Use `publish = true|false` to control whether an image target can enter the
  default release flow.
- Use `status = "stable" | "experimental" | "disabled"` to mark maturity.
- The default release matrix only includes targets with `publish = true` and
  `status = "stable"`.
- Java variants may override runtime-specific fields such as `runtime_packages`,
  `lang`, `language`, `lc_all`, and `generate_locales` to make `slim`
  meaningfully smaller than `default`.
- Generated Dockerfiles must not be edited by hand. Use `cargo run -- render`
  after changing metadata, then `cargo run -- verify` to confirm the tree is in
  sync.
