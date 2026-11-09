//! A finite guard for native scheduled Git inspection, not a scheduler or Git writer.
use crate::{
    config, now_ms,
    store::{Store, event},
};
use anyhow::{Result, ensure};
use serde_json::{Value, json};
pub fn create(
    store: &mut Store,
    id: &str,
    max_runs: u32,
    expires_unix: u64,
    isolated: bool,
) -> Result<Value> {
    config::identifier(id)?;
    ensure!(
        isolated,
        "review a separate checkout and explicitly confirm --isolated-checkout before scheduling"
    );
    ensure!((1..=1000).contains(&max_runs), "max_runs must be 1..1000");
    let now = (now_ms() / 1000) as u64;
    ensure!(
        expires_unix > now && expires_unix <= now + 30 * 86400,
        "expiry must be in the next 30 days"
    );
    ensure!(
        store.db.query_row(
            "SELECT count(*) FROM tasks WHERE status IN ('running','paused')",
            [],
            |r| r.get::<_, u64>(0)
        )? == 0,
        "do not set up scheduled experiments in an active Astra task checkout"
    );
    let plan = crate::integration::git_plan(store, None)?;
    ensure!(
        plan["protected_branch"] == false && !plan["branch"].as_str().unwrap_or("").is_empty(),
        "scheduled experiments require a named non-protected branch"
    );
    let definition = json!({"id":id,"max_runs":max_runs,"expires_unix":expires_unix,"root_id":crate::digest(store.root.to_string_lossy().as_bytes()),"policy":"read-only inspection; no mutation or milestone target"});
    let artifact = store.artifact("cadence", &definition)?;
    let body = json!({"definition":definition,"artifact_id":artifact["id"]});
    store.write(|tx| {
        tx.execute(
            "INSERT INTO cadences(id,body,runs) VALUES(?1,?2,0)",
            rusqlite::params![id, body.to_string()],
        )?;
        event(tx, "cadence.created", &body)
    })?;
    Ok(
        json!({"cadence":definition,"artifact":artifact,"native_scheduler_installed":false,"manual_trial_argv":["<plugin-root>/scripts/astra-watch","--root",store.root,"cadence","tick","--id",id],"next":"Run this command manually. Schedule the same command/prompt with the native Desktop scheduler only after the user chooses cadence. When tick returns stop=true, end the run and pause/remove the schedule using native controls; do not fabricate a checkpoint."}),
    )
}
pub fn tick(store: &mut Store, id: &str) -> Result<Value> {
    let (body, count): (String, u32) =
        store
            .db
            .query_row("SELECT body,runs FROM cadences WHERE id=?1", [id], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?;
    let value: Value = serde_json::from_str(&body)?;
    let accepted = store.read_artifact(value["artifact_id"].as_str().unwrap())?;
    ensure!(
        accepted == value["definition"],
        "cadence definition integrity mismatch"
    );
    let d = &value["definition"];
    let expired = now_ms() / 1000 >= d["expires_unix"].as_u64().unwrap() as u128;
    let exhausted = count >= d["max_runs"].as_u64().unwrap() as u32;
    if expired || exhausted {
        return Ok(
            json!({"id":id,"stop":true,"expired":expired,"run_budget_exhausted":exhausted,"runs":count,"mutation_executed":false}),
        );
    }
    store.write(|tx| {
        ensure!(
            tx.execute(
                "UPDATE cadences SET runs=runs+1 WHERE id=?1 AND runs=?2",
                rusqlite::params![id, count]
            )? == 1,
            "concurrent cadence tick; retry state inspection"
        );
        event(tx, "cadence.tick", &json!({"id":id,"run":count+1}))
    })?;
    let active = store.db.query_row(
        "SELECT count(*) FROM tasks WHERE status IN ('running','paused')",
        [],
        |r| r.get::<_, u64>(0),
    )? > 0;
    let plan = if active {
        json!({"skipped":"active task detected; preserve its workflow","mutation_executed":false})
    } else {
        crate::integration::git_plan(store, None)?
    };
    Ok(
        json!({"id":id,"stop":count+1>=d["max_runs"].as_u64().unwrap() as u32,"runs":count+1,"inspection":plan,"milestone_achieved":null,"mutation_executed":false}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finite_native_schedule_guard_stops_without_git_mutation() {
        let d = tempfile::tempdir().unwrap();
        crate::bootstrap::bootstrap(Some(d.path()), false).unwrap();
        assert!(
            std::process::Command::new("git")
                .args(["init", "-b", "astra-trial"])
                .arg(d.path())
                .output()
                .unwrap()
                .status
                .success()
        );
        let mut s = Store::open(d.path()).unwrap();
        assert!(create(&mut s, "trial", 1, (now_ms() / 1000) as u64 + 3600, false).is_err());
        create(&mut s, "trial", 1, (now_ms() / 1000) as u64 + 3600, true).unwrap();
        assert_eq!(tick(&mut s, "trial").unwrap()["stop"], true);
        let end = tick(&mut s, "trial").unwrap();
        assert_eq!(end["run_budget_exhausted"], true);
        assert_eq!(end["runs"], 1);
        assert_eq!(end["mutation_executed"], false);
    }
}
