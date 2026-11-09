//! Native execution and external adapters are explicit plans, never fictitious capabilities.
use crate::{config, store::Store, tasks};
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::{path::Path, process::Command};

fn git(root: &Path, args: &[&str]) -> Result<String> {
    let result = Command::new("git")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args(["-c", "core.fsmonitor=false"])
        .arg("-C")
        .arg(root)
        .args(args)
        .output()?;
    ensure!(
        result.status.success(),
        "Git inspection failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    Ok(String::from_utf8(result.stdout)?)
}
pub fn git_plan(store: &Store, task: Option<&str>) -> Result<Value> {
    let branch = git(&store.root, &["branch", "--show-current"])?;
    let status = git(&store.root, &["status", "--short"])?;
    let (settings, _) = config::load(&store.root)?;
    let evidence = task
        .map(|id| tasks::load(store, id).and_then(|t| tasks::completion(store, &t)))
        .transpose();
    let verified = evidence.as_ref().is_ok_and(|v| v.is_some());
    let protected = settings
        .git
        .protected_branches
        .iter()
        .any(|b| b == branch.trim());
    Ok(
        json!({"branch":branch.trim(),"status":crate::telemetry::redact(&status),"no_op":status.trim().is_empty(),"verified_slice":verified,"protected_branch":protected,"mutation_executed":false,"checkpoint_proposal":if verified && !protected && !status.trim().is_empty(){json!({"review":["git diff --check","git diff --stat","git diff"],"next":"Review scoped paths and ask for authority before staging/committing; no automatic push, merge, amend or release."})}else{Value::Null},"verification_gap":evidence.err().map(|e|e.to_string()),"native_schedule_prompt":"In this isolated repository, inspect Git status and existing Astra Watch evidence. Report changed paths, stale evidence and blockers. An unchanged repository is a valid no-op. Do not invent milestones, commit, push, merge, release, run unconfigured commands, or interfere with active work. Stop at the user-approved expiry and run count.","schedule":"Manually trial this prompt first. Use native Desktop automations only after the user chooses cadence, finite run bounds, expiry and isolated checkout. Astra Watch does not install a second scheduler."}),
    )
}
pub fn docs_plan(root: &Path) -> Result<Value> {
    let (settings, _) = config::load(root)?;
    let mut commands = Vec::<Value>::new();
    if settings.languages.iter().any(|l| l == "rust") {
        commands.push(
            json!({"argv":["cargo","doc","--no-deps"],"kind":"rustdoc","artifact":"target/doc/"}),
        );
    }
    if settings.languages.iter().any(|l| l == "go") {
        commands.push(json!({"argv":["go","doc"],"kind":"current-package docs; select packages explicitly for a multi-package repo","artifact":"stdout; a site generator is a separate choice"}));
    }
    Ok(
        json!({"native_shell_commands":commands,"execute":false,"recordings":{"enabled":settings.docs.recordings_enabled,"terminal":"Use available asciinema in a deterministic, secret-free fixture; review the .cast before publishing.","browser":"Use a configured Playwright/browser recorder; review screenshots/video and scrub private data."},"github_pages":{"enabled":settings.docs.pages_enabled,"steps":["Discover existing docs language/site configuration","Confirm docstring and generator scope","Build and validate site and recordings locally","Obtain explicit publication authority","Use existing GitHub Pages workflow or installed Spit Roast publication skill","Verify deployment URL and record evidence"],"automatic_deployment":false},"adapter_status":"rustdoc/go doc command plans; optional installed Spit Roast, asciinema, Playwright and site generators remain separate dependencies"}),
    )
}
pub fn state(store: &Store) -> Result<Value> {
    let (settings, profile) = config::load(&store.root)?;
    store.audit()?;
    let mut tasks = tasks::list(store)?;
    for t in &mut tasks {
        t["profile_current"] = json!(t["profile_digest"] == profile);
    }
    Ok(
        json!({"root":store.root,"profile_digest":profile,"mission_digest":settings.mission_digest()?,"mission_ready":settings.mission_ready(),"mission":settings.mission,"tasks":tasks,"recent_events":store.events(last_cursor(store).saturating_sub(20),20)?,"last_event_cursor":last_cursor(store),"audit_chain":"verified","telemetry":crate::telemetry::summary(store)?,"capabilities":{"native_execution":"Codex shell, goals, subagents, worktrees and automations","retrieval":"local incremental lexical/symbol index","graph":"Tree-sitter syntax containment","semantic_lsp":"not built in; configure a native MCP adapter if needed","model_runtime":"native Codex; no background Astra model worker","verification":"CLI-only actual configured checks","controls":"cooperative ledger + verification cancellation; native turn interrupt remains native"}}),
    )
}
fn last_cursor(store: &Store) -> i64 {
    store
        .db
        .query_row("SELECT COALESCE(max(seq),0) FROM events", [], |r| r.get(0))
        .unwrap_or(0)
}
