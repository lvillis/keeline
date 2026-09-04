use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result, bail, ensure};

static NEXT_FILE_ID: AtomicU64 = AtomicU64::new(0);

struct StagedFile {
    target: PathBuf,
    temporary: PathBuf,
    backup: PathBuf,
    had_original: bool,
    installed: bool,
    backup_moved: bool,
}

pub(crate) fn replace_files(files: BTreeMap<PathBuf, String>) -> Result<()> {
    let mut staged = Vec::with_capacity(files.len());

    for (target, contents) in files {
        match stage_file(target, &contents) {
            Ok(file) => staged.push(file),
            Err(error) => {
                cleanup_temporaries(&staged);
                return Err(error);
            }
        }
    }

    for index in 0..staged.len() {
        if let Err(error) = install(&mut staged[index]) {
            let rollback = rollback(&mut staged[..=index]);
            cleanup_temporaries(&staged);
            return match rollback {
                Ok(()) => Err(error.context("batch replacement failed; previous files restored")),
                Err(rollback_error) => bail!(
                    "batch replacement failed: {error:#}; rollback also failed: {rollback_error:#}"
                ),
            };
        }
    }

    let mut cleanup_failures = Vec::new();
    for file in &staged {
        if file.backup_moved
            && let Err(error) = fs::remove_file(&file.backup)
        {
            cleanup_failures.push(format!("{}: {error}", file.backup.display()));
        }
    }
    cleanup_temporaries(&staged);
    ensure!(
        cleanup_failures.is_empty(),
        "files were replaced successfully, but backup cleanup failed: {}",
        cleanup_failures.join("; ")
    );

    Ok(())
}

fn stage_file(target: PathBuf, contents: &str) -> Result<StagedFile> {
    let parent = target
        .parent()
        .with_context(|| format!("file has no parent directory: {}", target.display()))?;
    ensure!(
        parent.is_dir(),
        "parent directory does not exist for {}",
        target.display()
    );

    let temporary = sidecar_path(&target, "tmp")?;
    let backup = sidecar_path(&target, "bak")?;
    let had_original = target.exists();
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .with_context(|| format!("failed to stage {}", target.display()))?;
    if let Err(error) = file
        .write_all(contents.as_bytes())
        .and_then(|()| file.sync_all())
    {
        let _ = fs::remove_file(&temporary);
        return Err(error).with_context(|| format!("failed to stage {}", target.display()));
    }

    if let Ok(metadata) = fs::metadata(&target)
        && let Err(error) = fs::set_permissions(&temporary, metadata.permissions())
    {
        let _ = fs::remove_file(&temporary);
        return Err(error)
            .with_context(|| format!("failed to preserve permissions for {}", target.display()));
    }

    Ok(StagedFile {
        target,
        temporary,
        backup,
        had_original,
        installed: false,
        backup_moved: false,
    })
}

fn install(file: &mut StagedFile) -> Result<()> {
    if file.had_original {
        fs::rename(&file.target, &file.backup)
            .with_context(|| format!("failed to back up {}", file.target.display()))?;
        file.backup_moved = true;
    }

    fs::rename(&file.temporary, &file.target)
        .with_context(|| format!("failed to replace {}", file.target.display()))?;
    file.installed = true;
    Ok(())
}

fn rollback(files: &mut [StagedFile]) -> Result<()> {
    let mut failures = Vec::new();

    for file in files.iter_mut().rev() {
        if file.installed {
            match fs::remove_file(&file.target) {
                Ok(()) => file.installed = false,
                Err(error) => {
                    failures.push(format!(
                        "failed to remove replacement {}: {error}",
                        file.target.display()
                    ));
                    continue;
                }
            }
        }
        if file.backup_moved {
            match fs::rename(&file.backup, &file.target) {
                Ok(()) => file.backup_moved = false,
                Err(error) => failures.push(format!(
                    "failed to restore {} from {}: {error}",
                    file.target.display(),
                    file.backup.display()
                )),
            }
        }
    }

    ensure!(
        failures.is_empty(),
        "could not restore every file: {}",
        failures.join("; ")
    );
    Ok(())
}

fn cleanup_temporaries(files: &[StagedFile]) {
    for file in files {
        let _ = fs::remove_file(&file.temporary);
    }
}

fn sidecar_path(path: &Path, extension: &str) -> Result<PathBuf> {
    let name = path
        .file_name()
        .with_context(|| format!("file has no name: {}", path.display()))?;
    let id = NEXT_FILE_ID.fetch_add(1, Ordering::Relaxed);
    let mut sidecar = OsString::from(".");
    sidecar.push(name);
    sidecar.push(format!(".keeline-{}-{id}.{extension}", std::process::id()));
    Ok(path.with_file_name(sidecar))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{StagedFile, replace_files, rollback};

    #[test]
    fn replaces_existing_and_new_files_as_one_batch() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("keeline-file-batch-{unique}"));
        fs::create_dir_all(&root).unwrap();
        let existing = root.join("existing.txt");
        let created = root.join("created.txt");
        fs::write(&existing, "old").unwrap();

        replace_files(BTreeMap::from([
            (existing.clone(), "new".to_string()),
            (created.clone(), "created".to_string()),
        ]))
        .unwrap();

        assert_eq!(fs::read_to_string(existing).unwrap(), "new");
        assert_eq!(fs::read_to_string(created).unwrap(), "created");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn removes_staged_files_when_batch_preparation_fails() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("keeline-file-batch-failure-{unique}"));
        fs::create_dir_all(&root).unwrap();
        let existing = root.join("a-existing.txt");
        fs::write(&existing, "old").unwrap();

        let result = replace_files(BTreeMap::from([
            (existing.clone(), "new".to_string()),
            (
                root.join("z-missing/created.txt"),
                "unreachable".to_string(),
            ),
        ]));

        assert!(result.is_err());
        assert_eq!(fs::read_to_string(existing).unwrap(), "old");
        assert!(fs::read_dir(&root).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains("keeline-")
        }));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rollback_restores_the_original_file() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("keeline-file-rollback-{unique}"));
        fs::create_dir_all(&root).unwrap();
        let target = root.join("target.txt");
        let backup = root.join("backup.txt");
        fs::write(&target, "replacement").unwrap();
        fs::write(&backup, "original").unwrap();
        let mut staged = [StagedFile {
            target: target.clone(),
            temporary: root.join("temporary.txt"),
            backup,
            had_original: true,
            installed: true,
            backup_moved: true,
        }];

        rollback(&mut staged).unwrap();

        assert_eq!(fs::read_to_string(target).unwrap(), "original");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rollback_reports_failure_and_retains_the_backup() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("keeline-file-rollback-failure-{unique}"));
        let target = root.join("target");
        let backup = root.join("backup.txt");
        fs::create_dir_all(&target).unwrap();
        fs::write(&backup, "original").unwrap();
        let mut staged = [StagedFile {
            target,
            temporary: root.join("temporary.txt"),
            backup: backup.clone(),
            had_original: true,
            installed: true,
            backup_moved: true,
        }];

        let error = rollback(&mut staged).unwrap_err();

        assert!(error.to_string().contains("could not restore every file"));
        assert_eq!(fs::read_to_string(backup).unwrap(), "original");
        fs::remove_dir_all(root).unwrap();
    }
}
