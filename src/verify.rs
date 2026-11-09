//! Actual checks run only from the CLI in Codex's native shell, never through MCP.
use crate::{
    config, knowledge, now_ms,
    store::{Store, event},
    tasks,
};
use anyhow::{Context, Result, ensure};
use rusqlite::params;
use serde_json::{Value, json};
use std::{
    fs,
    io::Read,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub fn plan(root: &Path, task: &str, criterion: &str) -> Result<Value> {
    let store = Store::open(root)?;
    let t = tasks::load(&store, task)?;
    tasks::reconcile(&store, &t)?;
    ensure!(
        t.criteria.iter().any(|c| c == criterion),
        "criterion not in this task"
    );
    let (settings, _) = config::load(root)?;
    let c = settings
        .mission
        .criteria
        .iter()
        .find(|c| c.id == criterion)
        .unwrap();
    let check = c
        .check
        .as_ref()
        .context("manual criterion needs independent human review; cannot fabricate a pass")?;
    let exe = std::env::current_exe()?;
    let launcher = exe
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("scripts/astra-watch"))
        .filter(|p| p.is_file())
        .unwrap_or(exe);
    Ok(
        json!({"argv":[launcher,"--root",root.to_str(),"verify","--task",task,"--criterion",criterion],"configured_check":settings.checks[check],"task_version":t.version,"permission_boundary":"Execute this exact absolute CLI argv through native Codex shell after reviewing the configured check. MCP does not execute arbitrary commands."}),
    )
}
#[cfg(unix)]
fn kill(child: &mut std::process::Child) {
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
    let _ = child.kill();
    let _ = child.wait();
}
struct OwnedChild(std::process::Child);
impl std::ops::Deref for OwnedChild {
    type Target = std::process::Child;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for OwnedChild {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        kill(&mut self.0);
    }
}
#[cfg(not(unix))]
fn kill(child: &mut std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}

pub fn run(root: &Path, task: &str, criterion: &str) -> Result<Value> {
    let mut store = Store::open(root)?;
    let t = tasks::load(&store, task)?;
    tasks::reconcile(&store, &t)?;
    ensure!(
        t.status == "running",
        "verification requires a running task"
    );
    ensure!(
        t.criteria.iter().any(|id| id == criterion),
        "criterion outside task contract"
    );
    let (settings, _) = config::load(root)?;
    let c = settings
        .mission
        .criteria
        .iter()
        .find(|c| c.id == criterion)
        .unwrap();
    let check_id = c
        .check
        .as_ref()
        .context("manual review is not executable evidence")?;
    let check = &settings.checks[check_id];
    let baseline = knowledge::snapshot(root)?;
    tasks::scope_changes(&t, &baseline)?;
    let failed:u32=store.db.query_row("SELECT count(*) FROM evidence WHERE task_id=?1 AND criterion_id=?2 AND json_extract(body,'$.slice')=?3 AND json_extract(body,'$.passed')=0",params![task,criterion,t.slice],|r|r.get(0))?;
    ensure!(
        failed <= settings.runtime.max_retries_per_check,
        "retry budget exhausted for this criterion and slice"
    );
    // Redirect to bounded-lifetime local files rather than pipes that can deadlock on large output.
    let out = tempfile::tempfile()?;
    let err = tempfile::tempfile()?;
    let mut cmd = Command::new(&check.program);
    cmd.args(&check.args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::from(out.try_clone()?))
        .stderr(Stdio::from(err.try_clone()?));
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let started = Instant::now();
    let mut child = OwnedChild(cmd.spawn().context("cannot spawn configured check")?);
    let mut interrupted = false;
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break Some(status);
        }
        if started.elapsed() > Duration::from_secs(check.timeout_secs) {
            timed_out = true;
            kill(&mut child);
            break None;
        }
        let live = tasks::load(&store, task)?;
        if live.version != t.version || live.status != "running" {
            interrupted = true;
            kill(&mut child);
            break None;
        }
        // An untrusted check could write unlimited output; stop it before logs consume the disk.
        if out.metadata()?.len() + err.metadata()?.len() > 8_000_000 {
            interrupted = true;
            kill(&mut child);
            break None;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let current = knowledge::snapshot(root)?;
    let live = tasks::load(&store, task)?;
    let unchanged = current == baseline;
    let scope_ok = tasks::scope_changes(&t, &current).is_ok();
    let profile_ok = tasks::reconcile(&store, &t).is_ok();
    let passed = status.is_some_and(|s| s.success())
        && !interrupted
        && !timed_out
        && unchanged
        && scope_ok
        && profile_ok
        && live.version == t.version
        && live.status == "running";
    fn excerpt(mut file: fs::File) -> Result<String> {
        use std::io::{Seek, SeekFrom};
        file.seek(SeekFrom::Start(0))?;
        let mut bytes = Vec::new();
        file.take(16000).read_to_end(&mut bytes)?;
        Ok(crate::telemetry::redact(&String::from_utf8_lossy(&bytes)))
    }
    let id = uuid::Uuid::new_v4().to_string();
    let e = json!({"id":id,"task":task,"criterion":criterion,"check":check_id,"slice":t.slice,"profile_digest":t.profile_digest,"mission_digest":t.mission_digest,"snapshot_digest":current.digest,"source_unchanged_during_check":unchanged,"passed":passed,"exit_code":status.and_then(|s|s.code()),"timed_out":timed_out,"interrupted":interrupted,"scope_ok":scope_ok,"profile_ok":profile_ok,"duration_ms":started.elapsed().as_millis(),"stdout":excerpt(out)?,"stderr":excerpt(err)?,"at_ms":now_ms(),"authority":"actual configured CLI subprocess, not model assertion"});
    store.write(|tx| {tx.execute("INSERT INTO evidence(id,task_id,criterion_id,body) VALUES(?1,?2,?3,?4)",params![id,task,criterion,serde_json::to_string(&e)?])?;event(tx,"verification.finished",&json!({"id":id,"task":task,"criterion":criterion,"passed":passed,"duration_ms":e["duration_ms"]}))})?;
    Ok(e)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup(program: &str, args: Vec<String>, timeout: u64) -> (tempfile::TempDir, Store) {
        let d = tempfile::tempdir().unwrap();
        crate::bootstrap::bootstrap(Some(d.path()), false).unwrap();
        let (mut c, _) = config::load(d.path()).unwrap();
        c.mission.outcome = "Test real evidence".into();
        c.checks.insert(
            "test".into(),
            config::Check {
                program: program.into(),
                args,
                timeout_secs: timeout,
            },
        );
        c.mission.criteria.push(config::Criterion {
            id: "works".into(),
            description: "Check passes".into(),
            check: Some("test".into()),
        });
        config::save(d.path(), &c).unwrap();
        let mut s = Store::open(d.path()).unwrap();
        let req = |action, version| tasks::Request {
            action,
            id: "a".into(),
            expected_version: version,
            title: Some("Slice".into()),
            criteria: Some(vec!["works".into()]),
            write_scope: Some(vec!["src".into()]),
            note: None,
        };
        tasks::mutate(&mut s, req(tasks::Action::Queue, None)).unwrap();
        tasks::mutate(&mut s, req(tasks::Action::Start, Some(1))).unwrap();
        (d, s)
    }
    #[test]
    fn passed_execution_is_current_and_new_edits_invalidate_it() {
        let (d, s) = setup("true", vec![], 2);
        assert_eq!(run(d.path(), "a", "works").unwrap()["passed"], true);
        tasks::completion(&s, &tasks::load(&s, "a").unwrap()).unwrap();
        fs::create_dir(d.path().join("src")).unwrap();
        fs::write(d.path().join("src/lib.rs"), "fn changed() {}\n").unwrap();
        assert!(tasks::completion(&s, &tasks::load(&s, "a").unwrap()).is_err());
    }
    #[test]
    fn timeout_and_failure_are_not_completion() {
        let (d, s) = setup("sleep", vec!["3".into()], 1);
        let e = run(d.path(), "a", "works").unwrap();
        assert_eq!(e["passed"], false);
        assert_eq!(e["timed_out"], true);
        assert!(tasks::completion(&s, &tasks::load(&s, "a").unwrap()).is_err());
    }
}
