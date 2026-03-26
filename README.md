# Keeline

Keeline provides Debian-based runtime images published to GHCR, with `tino`
integrated as the default PID 1 init process and `salus` bundled for health
checks across the image line.

## Images

Current image packages:

- `ghcr.io/lvillis/keeline-debian`
- `ghcr.io/lvillis/keeline-jdk`

Current published image lines:

- `ghcr.io/lvillis/keeline-debian:13`
- `ghcr.io/lvillis/keeline-debian:13-slim`
- `ghcr.io/lvillis/keeline-jdk:17-trixie`
- `ghcr.io/lvillis/keeline-jdk:17-trixie-slim`
- `ghcr.io/lvillis/keeline-jdk:21-trixie`
- `ghcr.io/lvillis/keeline-jdk:21-trixie-slim`
- `ghcr.io/lvillis/keeline-jdk:8u372-trixie`
- `ghcr.io/lvillis/keeline-jdk:8u372-trixie-slim`

## Tag Rules

- Package names express the image family.
- Image tags express version and variant.
- Debian tags use forms like `13` and `13-slim`.
- JDK tags use forms like `21-trixie`, `21.0.10-trixie`, `8u372-trixie`, and `8u372-trixie-slim`.
- `latest` is not published.

## Usage

Examples:

```bash
docker pull ghcr.io/lvillis/keeline-debian:13
docker pull ghcr.io/lvillis/keeline-jdk:21-trixie
docker pull ghcr.io/lvillis/keeline-jdk:8u372-trixie
```

For strongly reproducible deployments, pin by digest.

## Scope

- Debian images provide a clean Debian 13 base.
- JDK images provide Debian 13 based Java runtimes built for stable consumption.
- All images include `tino` at `/sbin/tino` and start through `ENTRYPOINT ["/sbin/tino", "-g", "-s", "--"]`.
- All images include `salus` at `/bin/salus` for downstream `HEALTHCHECK` and Kubernetes `exec` probes.
- JDK `slim` images reduce runtime packages and use `C.UTF-8` instead of generated `en_US.UTF-8` locales.
- The project keeps image families separate instead of mixing them into one package with complex tags.
