use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use keeline::domain::ImageCatalog;
use keeline::render;

#[test]
fn discovers_expected_image_targets() {
    let catalog = ImageCatalog::discover(Path::new("images")).unwrap();
    assert_eq!(catalog.targets.len(), 7);
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

[[variants]]
name = "default"
dockerfile = "Dockerfile"
base_image = "debian:13"
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
