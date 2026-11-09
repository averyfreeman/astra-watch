//! A typed plugin namespace coexists with other project.config.toml consumers.
use crate::{digest, files};
use anyhow::{Context, Result, ensure};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub schema_version: u32,
    pub prompt_file: String,
    pub languages: Vec<String>,
    pub mission: Mission,
    pub checks: BTreeMap<String, Check>,
    pub runtime: Runtime,
    pub knowledge: Knowledge,
    pub git: GitPolicy,
    pub docs: DocsPolicy,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            prompt_file: ".astra-watch/project_prompt.md".into(),
            languages: vec![],
            mission: Mission::default(),
            checks: BTreeMap::new(),
            runtime: Runtime::default(),
            knowledge: Knowledge::default(),
            git: GitPolicy::default(),
            docs: DocsPolicy::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Mission {
    pub outcome: String,
    pub non_goals: Vec<String>,
    pub constraints: Vec<String>,
    pub criteria: Vec<Criterion>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Criterion {
    /// Stable acceptance criterion ID, independent of a worker or task.
    pub id: String,
    pub description: String,
    /// Configured check ID, or null for a separately approved manual review.
    pub check: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Check {
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}
fn default_timeout() -> u64 {
    120
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Runtime {
    pub max_slices: u32,
    pub max_retries_per_check: u32,
}
impl Default for Runtime {
    fn default() -> Self {
        Self {
            max_slices: 80,
            max_retries_per_check: 2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Knowledge {
    pub max_files: usize,
    pub max_file_bytes: usize,
    pub max_chunks: usize,
    pub max_hits: usize,
}
impl Default for Knowledge {
    fn default() -> Self {
        Self {
            max_files: 10000,
            max_file_bytes: 2_000_000,
            max_chunks: 30000,
            max_hits: 8,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct GitPolicy {
    pub protected_branches: Vec<String>,
    pub checkpoint_mode: String,
}
impl Default for GitPolicy {
    fn default() -> Self {
        Self {
            protected_branches: vec!["main".into(), "master".into()],
            checkpoint_mode: "verified-slice-proposal".into(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct DocsPolicy {
    pub pages_enabled: bool,
    pub recordings_enabled: bool,
}

pub fn identifier(id: &str) -> Result<()> {
    ensure!(
        !id.is_empty()
            && id.len() <= 80
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'),
        "invalid stable identifier"
    );
    Ok(())
}

impl Settings {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == 1,
            "unsupported Astra Watch profile version"
        );
        files::relative(&self.prompt_file)?;
        ensure!(
            (1..=500).contains(&self.runtime.max_slices),
            "max_slices must be 1..500"
        );
        ensure!(
            self.runtime.max_retries_per_check <= 10,
            "retry budget must be at most ten"
        );
        ensure!(
            (1..=50000).contains(&self.knowledge.max_files),
            "max_files must be 1..50000"
        );
        ensure!(
            (256..=10_000_000).contains(&self.knowledge.max_file_bytes),
            "invalid file byte limit"
        );
        ensure!(
            (1..=100000).contains(&self.knowledge.max_chunks),
            "invalid chunk limit"
        );
        ensure!(
            (1..=40).contains(&self.knowledge.max_hits),
            "max_hits must be 1..40"
        );
        ensure!(
            self.git.checkpoint_mode == "verified-slice-proposal",
            "only evidence-based Git proposals are supported"
        );
        let mut ids = BTreeSet::new();
        for c in &self.mission.criteria {
            identifier(&c.id)?;
            ensure!(ids.insert(&c.id), "duplicate acceptance criterion");
            ensure!(
                !c.description.trim().is_empty(),
                "criterion needs a description"
            );
            if let Some(check) = &c.check {
                ensure!(
                    self.checks.contains_key(check),
                    "criterion names an unknown check: {check}"
                );
            }
        }
        for (id, check) in &self.checks {
            identifier(id)?;
            ensure!(
                !check.program.trim().is_empty() && !check.program.contains('\0'),
                "check needs an executable program"
            );
            ensure!(
                (1..=3600).contains(&check.timeout_secs),
                "check timeout must be 1..3600 seconds"
            );
            ensure!(
                check.args.iter().all(|a| !a.contains('\0')),
                "check arguments cannot contain NUL"
            );
        }
        Ok(())
    }
    pub fn mission_ready(&self) -> bool {
        !self.mission.outcome.trim().is_empty() && !self.mission.criteria.is_empty()
    }
    pub fn mission_digest(&self) -> Result<String> {
        Ok(digest(serde_json::to_vec(&self.mission)?))
    }
}

pub fn load(root: &Path) -> Result<(Settings, String)> {
    let text = fs::read_to_string(files::inside(root, "project.config.toml")?)
        .context("bootstrap this project first")?;
    let parsed: toml::Value =
        toml::from_str(&text).context("project.config.toml must be valid TOML")?;
    let settings: Settings = parsed
        .get("astra_watch")
        .context("profile has no [astra_watch] namespace")?
        .clone()
        .try_into()?;
    settings.validate()?;
    Ok((settings, digest(text)))
}

pub fn defaults() -> Result<(Settings, Option<PathBuf>)> {
    if let Some(explicit) = std::env::var_os("ASTRA_WATCH_DEFAULTS") {
        ensure!(
            Path::new(&explicit).is_file(),
            "ASTRA_WATCH_DEFAULTS names a missing or non-file profile"
        );
    }
    let path = std::env::var_os("ASTRA_WATCH_DEFAULTS")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")))
                .map(|p| p.join("astra-watch/defaults.toml"))
        });
    if let Some(path) = path.filter(|p| p.exists()) {
        let value: toml::Value = toml::from_str(&fs::read_to_string(&path)?)?;
        let settings: Settings = value
            .get("astra_watch")
            .unwrap_or(&value)
            .clone()
            .try_into()?;
        settings.validate()?;
        Ok((settings, Some(path)))
    } else {
        Ok((Settings::default(), None))
    }
}

pub fn save(root: &Path, settings: &Settings) -> Result<()> {
    settings.validate()?;
    let path = files::inside(root, "project.config.toml")?;
    let existing = if path.exists() {
        fs::read_to_string(path)?
    } else {
        String::new()
    };
    let mut document = existing.parse::<toml_edit::DocumentMut>()?;
    let own = toml_edit::ser::to_document(settings)?.into_table();
    document["astra_watch"] = toml_edit::Item::Table(own);
    files::replace(root, "project.config.toml", document.to_string().as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_other_consumers_and_comments() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("project.config.toml"),
            "# important\n[other]\nanswer = 42\n",
        )
        .unwrap();
        save(dir.path(), &Settings::default()).unwrap();
        let text = fs::read_to_string(dir.path().join("project.config.toml")).unwrap();
        assert!(text.contains("# important"));
        assert!(text.contains("answer = 42"));
        let (settings, _) = load(dir.path()).unwrap();
        assert_eq!(settings, Settings::default());
    }
    #[test]
    fn missing_and_duplicate_check_bindings_fail() {
        let mut s = Settings::default();
        s.mission.criteria.push(Criterion {
            id: "correct".into(),
            description: "Works".into(),
            check: Some("absent".into()),
        });
        assert!(s.validate().is_err());
        s.mission.criteria[0].check = None;
        s.mission.criteria.push(s.mission.criteria[0].clone());
        assert!(s.validate().is_err());
    }
}
