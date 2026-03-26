use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use keeline::domain::ImageCatalog;
use keeline::render;

#[test]
fn discovers_expected_image_targets() {
    let catalog = ImageCatalog::discover(Path::new("images")).unwrap();
    assert_eq!(catalog.targets.len(), 8);
}

#[test]
fn validates_repository_image_catalog() {
    let catalog = ImageCatalog::discover(Path::new("images")).unwrap();
    catalog.validate().unwrap();
}

#[test]
fn keeps_rendered_dockerfiles_in_sync() {
    let catalog = ImageCatalog::discover(Path::new("images")).unwrap();
    render::check_catalog(&catalog).unwrap();
}

#[test]
fn all_repository_images_declare_tino_init() {
    let catalog = ImageCatalog::discover(Path::new("images")).unwrap();

    for target in &catalog.targets {
        let init = target.init.as_ref().unwrap();
        assert_eq!(init.provider, "tino");
        assert_eq!(init.binary_path, "/sbin/tino");
        assert_eq!(init.entrypoint, vec!["/sbin/tino", "-g", "-s", "--"]);

        let healthcheck = target.healthcheck.as_ref().unwrap();
        assert_eq!(healthcheck.provider, "salus");
        assert_eq!(healthcheck.binary_path, "/bin/salus");
    }
}

#[test]
fn rendered_images_include_tino_entrypoint() {
    let catalog = ImageCatalog::discover(Path::new("images")).unwrap();
    let debian = catalog.target("debian-13").unwrap();
    let jdk = catalog.target("jdk-21-trixie").unwrap();

    let debian_rendered = render::render(debian).unwrap();
    let jdk_rendered = render::render(jdk).unwrap();

    assert!(debian_rendered.contains("COPY --from=init /out/tino /sbin/tino"));
    assert!(debian_rendered.contains("COPY --from=healthcheck /out/salus /bin/salus"));
    assert!(debian_rendered.contains("ENTRYPOINT [\"/sbin/tino\",\"-g\",\"-s\",\"--\"]"));
    assert!(jdk_rendered.contains("COPY --from=init /out/tino /sbin/tino"));
    assert!(jdk_rendered.contains("COPY --from=healthcheck /out/salus /bin/salus"));
    assert!(jdk_rendered.contains("ENTRYPOINT [\"/sbin/tino\",\"-g\",\"-s\",\"--\"]"));
}

#[test]
fn jdk_slim_targets_override_runtime_packages_and_locale_defaults() {
    let catalog = ImageCatalog::discover(Path::new("images")).unwrap();
    let target = catalog.target("jdk-21-trixie-slim").unwrap();
    let java = target.java.as_ref().unwrap();

    assert_eq!(
        java.runtime_packages,
        vec![
            "binutils".to_string(),
            "ca-certificates".to_string(),
            "tzdata".to_string()
        ]
    );
    assert_eq!(java.lang, "C.UTF-8");
    assert_eq!(java.language, "C.UTF-8");
    assert_eq!(java.lc_all, "C.UTF-8");
    assert!(!java.generate_locales);
}

#[test]
fn render_recreates_missing_generated_dockerfiles() {
    let root = unique_temp_dir();
    let image_dir = root.join("debian/13");
    fs::create_dir_all(&image_dir).unwrap();
    fs::write(
        image_dir.join("image.toml"),
        r#"
schema = 1
family = "debian"
line = "13"
version = "13"
distro = "trixie"
id = "debian-13"
package = "keeline-debian"
platforms = ["linux/amd64"]

[init]
provider = "tino"
release = "0.1.15"
binary_path = "/sbin/tino"
entrypoint = ["/sbin/tino", "-g", "-s", "--"]

[[init.archives]]
platform = "linux/amd64"
url = "https://example.com/tino.tar.gz"
sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"

[healthcheck]
provider = "salus"
release = "0.1.5"
binary_path = "/bin/salus"

[[healthcheck.archives]]
platform = "linux/amd64"
url = "https://example.com/salus.tar.gz"
sha256 = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"

[[variants]]
name = "default"
dockerfile = "Dockerfile"
base_image = "docker.io/library/debian:13"
title = "Keeline Debian 13"
description = "Keeline Debian 13 (trixie) base image"
command = ["bash"]
canonical = ["13"]
"#,
    )
    .unwrap();

    let catalog = ImageCatalog::discover(&root).unwrap();
    catalog.validate().unwrap();
    assert!(!image_dir.join("Dockerfile").exists());

    let summary = render::sync_catalog(&catalog).unwrap();
    assert_eq!(summary.generated, 1);
    assert_eq!(summary.updated, 0);
    assert_eq!(summary.unchanged, 0);
    render::check_catalog(&catalog).unwrap();

    fs::remove_dir_all(root).unwrap();
}

fn unique_temp_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("keeline-render-test-{nanos}"))
}
