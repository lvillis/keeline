use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use keeline::domain::{ImageCatalog, ToolRole};
use keeline::render;

#[test]
fn discovers_expected_image_targets() {
    let catalog = ImageCatalog::discover(Path::new("images")).unwrap();
    assert_eq!(catalog.targets.len(), 9);
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
fn all_repository_images_declare_runtime_tools() {
    let catalog = ImageCatalog::discover(Path::new("images")).unwrap();

    for target in &catalog.targets {
        let init = target.tool_by_role(ToolRole::Init).unwrap();
        assert_eq!(init.name, "tino");
        assert!(
            init.image
                .as_deref()
                .unwrap()
                .starts_with("ghcr.io/lvillis/tino:")
        );
        assert_eq!(init.target_path, "/sbin/tino");
        assert_eq!(init.entrypoint, vec!["/sbin/tino", "-g", "-s", "--"]);

        let healthcheck = target.tool_by_role(ToolRole::Healthcheck).unwrap();
        assert_eq!(healthcheck.name, "salus");
        assert!(
            healthcheck
                .image
                .as_deref()
                .unwrap()
                .starts_with("ghcr.io/lvillis/salus:")
        );
        assert_eq!(healthcheck.target_path, "/bin/salus");

        let motd = target.tool_by_role(ToolRole::Motd).unwrap();
        assert_eq!(motd.name, "motdyn");
        assert!(
            motd.image
                .as_deref()
                .unwrap()
                .starts_with("ghcr.io/lvillis/motdyn:")
        );
        assert_eq!(motd.target_path, "/usr/local/bin/motdyn");
    }
}

#[test]
fn rendered_images_include_bundled_tools() {
    let catalog = ImageCatalog::discover(Path::new("images")).unwrap();
    let debian = catalog.target("debian-13").unwrap();
    let java = catalog.target("java-21-trixie").unwrap();
    let scratch = catalog.target("scratch-1").unwrap();

    let debian_rendered = render::render(debian).unwrap();
    let java_rendered = render::render(java).unwrap();
    let scratch_rendered = render::render(scratch).unwrap();

    for tool_name in ["tino", "salus", "motdyn"] {
        let tool = debian.tool(tool_name).unwrap();
        let image = tool.image.as_deref().unwrap();
        assert!(debian_rendered.contains(&format!("FROM {image} AS {tool_name}")));
    }
    assert!(debian_rendered.contains("COPY --from=tino /sbin/tino /sbin/tino"));
    assert!(debian_rendered.contains("COPY --from=salus /bin/salus /bin/salus"));
    assert!(
        debian_rendered.contains("COPY --from=motdyn /usr/local/bin/motdyn /usr/local/bin/motdyn")
    );
    assert!(debian_rendered.contains("ENTRYPOINT [\"/sbin/tino\",\"-g\",\"-s\",\"--\"]"));
    assert!(java_rendered.contains("COPY --from=tino /sbin/tino /sbin/tino"));
    assert!(java_rendered.contains("COPY --from=salus /bin/salus /bin/salus"));
    assert!(
        java_rendered.contains("COPY --from=motdyn /usr/local/bin/motdyn /usr/local/bin/motdyn")
    );
    assert!(java_rendered.contains("ENTRYPOINT [\"/sbin/tino\",\"-g\",\"-s\",\"--\"]"));
    assert!(
        java_rendered.find("ARG KEELINE_IMAGE_SOURCE=").unwrap()
            > java_rendered.find("    javac --version\n\n").unwrap()
    );
    assert!(scratch_rendered.contains("FROM scratch"));
    assert!(scratch_rendered.contains("COPY --from=tino /sbin/tino /sbin/tino"));
    assert!(scratch_rendered.contains("COPY --from=salus /bin/salus /bin/salus"));
    assert!(
        scratch_rendered.contains("COPY --from=motdyn /usr/local/bin/motdyn /usr/local/bin/motdyn")
    );
    assert!(scratch_rendered.contains("CMD [\"/bin/salus\",\"--version\"]"));
}

#[test]
fn java_slim_targets_override_runtime_packages_and_locale_defaults() {
    let catalog = ImageCatalog::discover(Path::new("images")).unwrap();
    let target = catalog.target("java-21-trixie-slim").unwrap();
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

[tools.tino]
role = "init"
release = "0.1.26"
image = "ghcr.io/lvillis/tino:0.1.26@sha256:8ad7b87083aee56d97f68c355bf57ad0a55ad5b00508f87dd86e148dcf91374b"
source_path = "/sbin/tino"
target_path = "/sbin/tino"
entrypoint = ["/sbin/tino", "-g", "-s", "--"]

[tools.salus]
role = "healthcheck"
release = "0.1.8"
image = "ghcr.io/lvillis/salus:0.1.8@sha256:c8469182df00b34dec2467776c86c22b36b235f3c4f6c93c3fff441f1b3ee568"
source_path = "/bin/salus"
target_path = "/bin/salus"

[tools.motdyn]
role = "motd"
release = "1.0.14"
image = "ghcr.io/lvillis/motdyn:1.0.14-slim@sha256:68eb88ae6031b08afaade56e04a0497f6139f80445bb6b3d27dc03a294ed1ef6"
source_path = "/usr/local/bin/motdyn"
target_path = "/usr/local/bin/motdyn"

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
