//! Mission-bound task contracts, optimistic concurrency and cooperative run controls.
use crate::{
    config, files,
    knowledge::{self, Snapshot},
    now_ms,
    store::{Store, event},
};
use anyhow::{Context, Result, ensure};
use rusqlite::{OptionalExtension, params};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub criteria: Vec<String>,
    pub write_scope: Vec<String>,
    pub profile_digest: String,
    pub mission_digest: String,
    pub contract_id: String,
    pub status: String,
    pub version: u64,
    pub slice: u32,
    pub baseline: Snapshot,
    pub checkpoint: Option<String>,
    pub steering: Vec<String>,
}
impl Task {
    pub fn view(&self) -> Value {
        json!({"id":self.id,"title":self.title,"criteria":self.criteria,"write_scope":self.write_scope,"profile_digest":self.profile_digest,"mission_digest":self.mission_digest,"contract_id":self.contract_id,"status":self.status,"version":self.version,"slice":self.slice,"baseline_digest":self.baseline.digest,"checkpoint":self.checkpoint,"steering":self.steering})
    }
}
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Queue,
    Start,
    Pause,
    Resume,
    Cancel,
    Steer,
    Checkpoint,
    Complete,
}
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub action: Action,
    pub id: String,
    /// Required for mutations of an existing task; stale updates are rejected.
    pub expected_version: Option<u64>,
    pub title: Option<String>,
    /// Queue requires nonempty criterion IDs from the accepted project profile.
    pub criteria: Option<Vec<String>>,
    /// Relative directories or exact files, no globs, traversal or project-root wildcard.
    pub write_scope: Option<Vec<String>>,
    pub note: Option<String>,
}
pub fn load(store: &Store, id: &str) -> Result<Task> {
    let (body, version, status): (String, u64, String) = store
        .db
        .query_row(
            "SELECT body,version,status FROM tasks WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .context("task not found")?;
    let task: Task = serde_json::from_str(&body)?;
    ensure!(
        task.id == id && task.version == version && task.status == status,
        "task state integrity mismatch"
    );
    Ok(task)
}
pub fn list(store: &Store) -> Result<Vec<Value>> {
    let mut stmt = store
        .db
        .prepare("SELECT body FROM tasks ORDER BY id LIMIT 200")?;
    stmt.query_map([], |r| r.get::<_, String>(0))?
        .map(|row| Ok(serde_json::from_str::<Task>(&row?)?.view()))
        .collect()
}
fn overlap(a: &str, b: &str) -> bool {
    a == b || a.starts_with(&format!("{b}/")) || b.starts_with(&format!("{a}/"))
}
pub fn reconcile(store: &Store, t: &Task) -> Result<()> {
    let (settings, profile) = config::load(&store.root)?;
    ensure!(
        t.profile_digest == profile && t.mission_digest == settings.mission_digest()?,
        "project profile or mission changed; cancel/replan the task under a new immutable contract"
    );
    let contract = store.read_artifact(&t.contract_id)?;
    ensure!(
        contract["id"] == t.id
            && contract["criteria"] == serde_json::to_value(&t.criteria)?
            && contract["write_scope"] == serde_json::to_value(&t.write_scope)?
            && contract["profile_digest"] == t.profile_digest
            && contract["mission_digest"] == t.mission_digest
            && contract["baseline_digest"] == t.baseline.digest
            && crate::digest(serde_json::to_vec(&t.baseline.files)?) == t.baseline.digest,
        "task contract or baseline integrity mismatch"
    );
    Ok(())
}
pub fn scope_changes(t: &Task, current: &Snapshot) -> Result<Vec<String>> {
    let paths: std::collections::BTreeSet<_> = t
        .baseline
        .files
        .keys()
        .chain(current.files.keys())
        .collect();
    let changed: Vec<String> = paths
        .into_iter()
        .filter(|p| t.baseline.files.get(*p) != current.files.get(*p))
        .cloned()
        .collect();
    let outside: Vec<_> = changed
        .iter()
        .filter(|p| {
            !t.write_scope
                .iter()
                .any(|s| *p == s || p.starts_with(&format!("{s}/")))
        })
        .collect();
    ensure!(
        outside.is_empty(),
        "out-of-scope changes detected (including concurrent changes): {outside:?}; reconcile explicitly, do not widen silently"
    );
    Ok(changed)
}

pub fn mutate(store: &mut Store, r: Request) -> Result<Value> {
    config::identifier(&r.id)?;
    if matches!(r.action, Action::Queue) {
        ensure!(
            r.expected_version.is_none(),
            "queue does not accept a previous version"
        );
        let (settings, profile) = config::load(&store.root)?;
        ensure!(
            settings.mission_ready(),
            "complete mission intake in project.config.toml before queueing delivery"
        );
        let title = r.title.context("queue needs title")?;
        ensure!(
            !title.trim().is_empty() && title.len() <= 500,
            "title must be 1..500 bytes"
        );
        let criteria = r.criteria.context("queue needs criteria")?;
        ensure!(
            !criteria.is_empty() && criteria.len() <= 100,
            "queue needs 1..100 criteria"
        );
        let mut ids = std::collections::BTreeSet::new();
        for id in &criteria {
            ensure!(
                ids.insert(id) && settings.mission.criteria.iter().any(|c| &c.id == id),
                "duplicate or unknown criterion: {id}"
            );
        }
        let scope = r.write_scope.context("queue needs write_scope")?;
        ensure!(
            !scope.is_empty() && scope.len() <= 100,
            "queue needs 1..100 explicit paths"
        );
        for p in &scope {
            files::relative(p)?;
            files::inside(&store.root, p)?;
            ensure!(
                !p.starts_with(".astra-watch/") && p != "project.config.toml",
                "runtime and profile cannot be worker write scope"
            );
        }
        let baseline = knowledge::snapshot(&store.root)?;
        let contract=store.artifact("task",&json!({"id":r.id,"title":title,"criteria":criteria,"write_scope":scope,"profile_digest":profile,"mission":settings.mission,"mission_digest":settings.mission_digest()?,"baseline_digest":baseline.digest}))?;
        let t = Task {
            id: r.id,
            title,
            criteria,
            write_scope: scope,
            profile_digest: profile,
            mission_digest: settings.mission_digest()?,
            contract_id: contract["id"].as_str().unwrap().into(),
            status: "queued".into(),
            version: 1,
            slice: 0,
            baseline,
            checkpoint: None,
            steering: vec![],
        };
        store.write(|tx| {
            tx.execute(
                "INSERT INTO tasks(id,version,status,body) VALUES(?1,?2,?3,?4)",
                params![t.id, t.version, t.status, serde_json::to_string(&t)?],
            )
            .context("task ID already exists; inspect it rather than recreate")?;
            event(tx, "task.queued", &t.view())
        })?;
        return Ok(t.view());
    }
    let mut t = load(store, &r.id)?;
    let expected = r
        .expected_version
        .context("existing task mutations require expected_version")?;
    ensure!(
        expected == t.version,
        "stale task version; read watch_state before retrying"
    );
    ensure!(
        !matches!(t.status.as_str(), "complete" | "cancelled"),
        "task is terminal; create a new contract for new work"
    );
    if !matches!(r.action, Action::Cancel | Action::Pause) {
        reconcile(store, &t)?;
    }
    let (settings, _) = config::load(&store.root)?;
    let kind = match r.action {
        Action::Start | Action::Resume => {
            ensure!(
                if matches!(r.action, Action::Start) {
                    t.status == "queued"
                } else {
                    t.status == "paused"
                },
                "invalid start/resume state"
            );
            ensure!(
                t.slice < settings.runtime.max_slices,
                "slice budget exhausted; obtain a revised mission/profile instead of silently extending it"
            );
            if t.status == "queued" {
                t.slice += 1;
            }
            t.status = "running".into();
            "task.started"
        }
        Action::Pause => {
            ensure!(t.status == "running", "only running tasks can pause");
            t.status = "paused".into();
            "task.paused"
        }
        Action::Cancel => {
            t.status = "cancelled".into();
            "task.cancelled"
        }
        Action::Steer => {
            let note = r.note.context("steer needs a concise instruction")?;
            ensure!(
                !note.trim().is_empty() && note.len() <= 4000,
                "steering note must be 1..4000 bytes"
            );
            ensure!(
                t.steering.len() < 100,
                "steering history bound reached; checkpoint/replan"
            );
            t.steering.push(crate::telemetry::redact(&note));
            "task.steered"
        }
        Action::Checkpoint => {
            ensure!(t.status == "running", "checkpoint needs a running task");
            let note = r
                .note
                .context("checkpoint needs observed progress, blockers and next action")?;
            ensure!(note.len() <= 12000, "checkpoint too large");
            let current = knowledge::snapshot(&store.root)?;
            scope_changes(&t, &current)?;
            let artifact=store.artifact("handoff",&json!({"task":t.view(),"note":crate::telemetry::redact(&note),"snapshot":current.digest,"at_ms":now_ms()}))?;
            t.checkpoint = Some(artifact["id"].as_str().unwrap().into());
            ensure!(
                t.slice < settings.runtime.max_slices,
                "slice budget exhausted; pause and request a revised bound"
            );
            t.slice += 1;
            "task.checkpointed"
        }
        Action::Complete => {
            ensure!(t.status == "running", "completion needs a running task");
            completion(store, &t)?;
            t.status = "complete".into();
            "task.completed"
        }
        Action::Queue => unreachable!(),
    };
    t.version += 1;
    store.write(|tx| {
        // Take the lease inside the same transaction as the update; concurrent starts cannot both win.
        if t.status == "running" {
            let mut stmt = tx.prepare(
                "SELECT body FROM tasks WHERE id!=?1 AND status IN ('running','paused')",
            )?;
            for row in stmt.query_map([&t.id], |r| r.get::<_, String>(0))? {
                let other: Task = serde_json::from_str(&row?)?;
                ensure!(
                    !t.write_scope
                        .iter()
                        .any(|a| other.write_scope.iter().any(|b| overlap(a, b))),
                    "write lease overlaps task {}; use separate worktrees or serialize",
                    other.id
                );
            }
        }
        ensure!(
            tx.execute(
                "UPDATE tasks SET version=?1,status=?2,body=?3 WHERE id=?4 AND version=?5",
                params![
                    t.version,
                    t.status,
                    serde_json::to_string(&t)?,
                    t.id,
                    expected
                ]
            )? == 1,
            "concurrent task update; read state and retry"
        );
        event(tx, kind, &t.view())
    })?;
    Ok(
        json!({"task":t.view(),"control_boundary":"cooperative task ledger; Astra verification watches pause/cancel. Native Codex turns require native interrupt/steer controls."}),
    )
}

pub fn completion(store: &Store, t: &Task) -> Result<Value> {
    reconcile(store, t)?;
    let current = knowledge::snapshot(&store.root)?;
    let changed = scope_changes(t, &current)?;
    let (settings, _) = config::load(&store.root)?;
    let mut accepted = Vec::new();
    for id in &t.criteria {
        let criterion = settings
            .mission
            .criteria
            .iter()
            .find(|c| &c.id == id)
            .unwrap();
        let check = criterion
            .check
            .as_ref()
            .context("manual criterion remains pending; v0.1 does not auto-attest human review")?;
        let mut stmt=store.db.prepare("SELECT body FROM evidence WHERE task_id=?1 AND criterion_id=?2 ORDER BY rowid DESC LIMIT 1")?;
        let body: String = stmt
            .query_row(params![t.id, id], |r| r.get(0))
            .optional()?
            .context(format!("no executed evidence for {id}"))?;
        let e: Value = serde_json::from_str(&body)?;
        ensure!(
            e["passed"] == true
                && e["snapshot_digest"] == current.digest
                && e["profile_digest"] == t.profile_digest
                && e["mission_digest"] == t.mission_digest
                && e["check"] == *check,
            "missing, failed or stale verification for criterion {id}"
        );
        accepted.push(e["id"].clone());
    }
    Ok(json!({"snapshot_digest":current.digest,"evidence":accepted,"changed_paths":changed}))
}

pub fn decision(
    store: &mut Store,
    title: &str,
    decision: &str,
    alternatives: &[String],
    supersedes: Option<&str>,
) -> Result<Value> {
    ensure!(
        !title.trim().is_empty()
            && title.len() <= 500
            && !decision.trim().is_empty()
            && decision.len() <= 12000
            && alternatives.len() <= 20,
        "invalid decision bounds"
    );
    if let Some(id) = supersedes {
        store.read_artifact(id)?;
    }
    let mut a=store.artifact("decision",&json!({"title":crate::telemetry::redact(title),"decision":crate::telemetry::redact(decision),"alternatives":alternatives.iter().map(|s|crate::telemetry::redact(s)).collect::<Vec<_>>(),"supersedes":supersedes,"at_ms":now_ms()}))?;
    let markdown_path = format!(
        ".astra-watch/records/decision-{}.md",
        a["id"].as_str().unwrap()
    );
    let choices = alternatives
        .iter()
        .map(|s| format!("- {}", crate::telemetry::redact(s)))
        .collect::<Vec<_>>()
        .join("\n");
    let markdown = format!(
        "# {}\n\nRecord: `{}`. Supersedes: `{}`.\n\n## Decision\n\n{}\n\n## Alternatives considered\n\n{}\n\nThis is an immutable projection. Create a superseding decision instead of editing it. The JSON record is canonical.\n",
        crate::telemetry::redact(title),
        a["id"].as_str().unwrap(),
        supersedes.unwrap_or("none"),
        crate::telemetry::redact(decision),
        choices
    );
    files::immutable(&store.root, &markdown_path, markdown.as_bytes())?;
    a["markdown_path"] = json!(markdown_path);
    store.write(|tx| event(tx, "decision.recorded", &a))?;
    Ok(a)
}

#[cfg(test)]
mod tests {
    use super::*;
    pub fn fixture() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        crate::bootstrap::bootstrap(Some(d.path()), false).unwrap();
        let (mut s, _) = config::load(d.path()).unwrap();
        s.mission.outcome = "Deliver a verified calculator".into();
        s.checks.insert(
            "test".into(),
            config::Check {
                program: "true".into(),
                args: vec![],
                timeout_secs: 2,
            },
        );
        s.mission.criteria.push(config::Criterion {
            id: "works".into(),
            description: "passes tests".into(),
            check: Some("test".into()),
        });
        config::save(d.path(), &s).unwrap();
        d
    }
    pub fn request(action: Action, id: &str, version: Option<u64>) -> Request {
        Request {
            action,
            id: id.into(),
            expected_version: version,
            title: Some("Calculator".into()),
            criteria: Some(vec!["works".into()]),
            write_scope: Some(vec!["src".into()]),
            note: Some("Observed failure; adjust tactics, keep outcome".into()),
        }
    }
    #[test]
    fn leases_cas_mission_drift_and_no_asserted_completion() {
        let d = fixture();
        let mut s = Store::open(d.path()).unwrap();
        mutate(&mut s, request(Action::Queue, "a", None)).unwrap();
        mutate(&mut s, request(Action::Queue, "b", None)).unwrap();
        mutate(&mut s, request(Action::Start, "a", Some(1))).unwrap();
        assert!(mutate(&mut s, request(Action::Start, "b", Some(1))).is_err());
        assert!(mutate(&mut s, request(Action::Pause, "a", Some(1))).is_err());
        assert!(mutate(&mut s, request(Action::Complete, "a", Some(2))).is_err());
        let (mut profile, _) = config::load(d.path()).unwrap();
        profile.mission.outcome = "Different".into();
        config::save(d.path(), &profile).unwrap();
        assert!(reconcile(&s, &load(&s, "a").unwrap()).is_err());
        mutate(&mut s, request(Action::Cancel, "a", Some(2))).unwrap();
    }
    #[test]
    fn scope_detects_concurrent_or_unexpected_changes_and_budget_is_real() {
        let d = fixture();
        let (mut settings, _) = config::load(d.path()).unwrap();
        settings.runtime.max_slices = 1;
        config::save(d.path(), &settings).unwrap();
        let mut s = Store::open(d.path()).unwrap();
        mutate(&mut s, request(Action::Queue, "a", None)).unwrap();
        mutate(&mut s, request(Action::Start, "a", Some(1))).unwrap();
        std::fs::write(d.path().join("outside.txt"), "unexpected").unwrap();
        assert!(
            scope_changes(
                &load(&s, "a").unwrap(),
                &knowledge::snapshot(d.path()).unwrap()
            )
            .is_err()
        );
        std::fs::remove_file(d.path().join("outside.txt")).unwrap();
        assert!(mutate(&mut s, request(Action::Checkpoint, "a", Some(2))).is_err());
    }
}
