use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::Serialize;

use crate::domain::ToolRole;

#[derive(Debug, Clone, Serialize)]
pub(super) struct ToolDeclaration {
    pub(super) name: String,
    pub(super) role: ToolRole,
    pub(super) release: String,
    pub(super) image: Option<String>,
    pub(super) source_path: String,
    pub(super) target_path: String,
    pub(super) tag_suffix: Option<String>,
    pub(super) image_ref: Option<ImageReference>,
    pub(super) definition_files: BTreeSet<PathBuf>,
    pub(super) target_ids: BTreeSet<String>,
}

impl ToolDeclaration {
    pub(super) fn image_tag_reference(&self) -> Option<String> {
        self.image_ref
            .as_ref()
            .map(ImageReference::tag_reference)
            .or_else(|| self.image.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct ToolKey {
    pub(super) name: String,
    pub(super) role: String,
    pub(super) release: String,
    pub(super) image: Option<String>,
    pub(super) source_path: String,
    pub(super) target_path: String,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct ImageReference {
    pub(super) registry: String,
    pub(super) repository: String,
    pub(super) tag: String,
    pub(super) digest: String,
}

impl ImageReference {
    pub(super) fn tag_reference(&self) -> String {
        format!("{}/{}:{}", self.registry, self.repository, self.tag)
    }

    pub(super) fn with_tag_and_digest(&self, tag: &str, digest: &str) -> String {
        format!("{}/{}:{tag}@{digest}", self.registry, self.repository)
    }
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct ToolCheck {
    pub(super) name: String,
    pub(super) status: ToolStatus,
    pub(super) current_release: String,
    pub(super) latest_release: String,
    pub(super) current_image: String,
    pub(super) latest_image: String,
    pub(super) current_digest: String,
    pub(super) latest_digest: String,
    pub(super) update_release: Option<String>,
    pub(super) update_image: Option<String>,
    pub(super) definition_files: BTreeSet<PathBuf>,
}

impl ToolCheck {
    pub(super) fn needs_attention(&self) -> bool {
        self.status != ToolStatus::Current
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ToolStatus {
    Current,
    DigestChanged,
    UpdateAvailable,
    MajorSkipped,
}

impl ToolStatus {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::DigestChanged => "digest-changed",
            Self::UpdateAvailable => "update-available",
            Self::MajorSkipped => "major-skipped",
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct PlannedToolUpdate {
    pub(super) name: String,
    pub(super) old_release: String,
    pub(super) new_release: String,
    pub(super) old_image: String,
    pub(super) new_image: String,
    pub(super) definition_files: BTreeSet<PathBuf>,
}

impl PlannedToolUpdate {
    pub(super) fn from_check(check: &ToolCheck) -> Option<Self> {
        Some(Self {
            name: check.name.clone(),
            old_release: check.current_release.clone(),
            new_release: check.update_release.clone()?,
            old_image: check.current_image.clone(),
            new_image: check.update_image.clone()?,
            definition_files: check.definition_files.clone(),
        })
    }
}
