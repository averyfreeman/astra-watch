//! Measured local events; native model usage is imported, never guessed.
use crate::{
    digest, now_ms,
    store::{Store, event},
};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;

pub fn redact(text: &str) -> String {
    use std::sync::OnceLock;
    static PATTERNS: OnceLock<Vec<regex::Regex>> = OnceLock::new();
    let patterns=PATTERNS.get_or_init(||vec![
        regex::Regex::new(r"(?i)(api[_-]?key|password|secret|access[_-]?token|authorization)(\s*[=:]\s*)[^\s,;]+" ).unwrap(),
        regex::Regex::new(r"\b(?:sk-[A-Za-z0-9_-]{16,}|gh[pousr]_[A-Za-z0-9]{16,})\b").unwrap(),
    ]);
    let mut out = text.to_string();
    out = patterns[0].replace_all(&out, "$1$2[REDACTED]").into_owned();
    out = patterns[1].replace_all(&out, "[REDACTED]").into_owned();
    out
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Usage {
    pub source: String,
    pub event_id: String,
    pub input_tokens: Option<u64>,
    pub cached_input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub reasoning_output_tokens: Option<u64>,
    /// Unknown unless a real source explicitly exposes a distinct write count.
    pub prompt_cache_write_tokens: Option<u64>,
    /// MCP tool metadata cache latency, not a token count.
    pub mcp_tool_cache_write_duration_ms: Option<f64>,
}
pub fn import(store: &mut Store, usage: Usage) -> Result<Value> {
    ensure!(
        usage.source.starts_with("codex:")
            && !usage.event_id.is_empty()
            && usage.event_id.len() <= 200,
        "source must identify the native Codex log/schema and stable event ID"
    );
    ensure!(
        usage.prompt_cache_write_tokens.is_none(),
        "Codex prompt-cache-write token coverage is not established; leave null rather than infer it"
    );
    if let (Some(c), Some(i)) = (usage.cached_input_tokens, usage.input_tokens) {
        ensure!(
            c <= i,
            "cached input is a subset of input, not additional input"
        );
    }
    if let (Some(r), Some(o)) = (usage.reasoning_output_tokens, usage.output_tokens) {
        ensure!(r <= o, "reasoning output is a subset of output");
    }
    if let Some(ms) = usage.mcp_tool_cache_write_duration_ms {
        ensure!(ms.is_finite() && ms >= 0., "invalid cache timing");
    }
    let id = digest(format!("{}:{}", usage.source, usage.event_id));
    let body = serde_json::to_string(
        &json!({"usage":usage,"id":id,"at_ms":now_ms(),"provenance":"explicit local import; caller must select a real native usage event"}),
    )?;
    store.write(|tx| {
        tx.execute(
            "INSERT OR IGNORE INTO telemetry(id,body) VALUES(?1,?2)",
            rusqlite::params![i64::from_str_radix(&id[..15], 16)?, body],
        )?;
        event(tx, "telemetry.imported", &json!({"id":id}))
    })?;
    Ok(json!({"id":id,"prompt_cache_write_tokens":null}))
}
pub fn summary(store: &Store) -> Result<Value> {
    let mut stmt = store
        .db
        .prepare("SELECT body FROM telemetry ORDER BY id LIMIT 10000")?;
    let mut input: Option<u64> = None;
    let mut cached: Option<u64> = None;
    let mut output: Option<u64> = None;
    let mut reasoning: Option<u64> = None;
    let mut count = 0;
    let mut present = std::collections::BTreeMap::<String, u64>::new();
    for row in stmt.query_map([], |r| r.get::<_, String>(0))? {
        let value: Value = serde_json::from_str(&row?)?;
        let u = &value["usage"];
        for (sum, key) in [
            (&mut input, "input_tokens"),
            (&mut cached, "cached_input_tokens"),
            (&mut output, "output_tokens"),
            (&mut reasoning, "reasoning_output_tokens"),
        ] {
            if let Some(n) = u[key].as_u64() {
                *present.entry(key.into()).or_default() += 1;
                *sum = Some(
                    sum.unwrap_or(0)
                        .checked_add(n)
                        .ok_or_else(|| anyhow::anyhow!("token counter overflow"))?,
                );
            }
        }
        count += 1;
    }
    Ok(
        json!({"imported_events":count,"field_presence_counts":present,"sums_are_reported_values_only":true,"input_tokens":input,"cached_input_tokens":cached,"output_tokens":output,"reasoning_output_tokens":reasoning,"prompt_cache_write_tokens":null,"coverage":"explicit native imports only; missing values are unknown, not zero. Partial sums do not reconstruct absent values. Input includes cached input; output may include reasoning output.","otel_export":"local spans only; configure native Codex telemetry at user/host scope separately","dashboard_is_flamegraph":false}),
    )
}
pub fn otlp(store: &Store, after: i64) -> Result<Value> {
    let events = store.events(after, 1000)?;
    let mut spans = Vec::new();
    for e in events {
        let hash = e["hash"].as_str().unwrap();
        let start = e["at_ms"].as_str().unwrap().parse::<u128>()? * 1_000_000;
        let ms = e["body"]["duration_ms"].as_u64().unwrap_or(0) as u128;
        let start = start.saturating_sub(ms * 1_000_000);
        let status = if e["body"]["passed"] == false || e["body"]["ok"] == false {
            2
        } else {
            0
        };
        spans.push(json!({"traceId":&hash[..32],"spanId":&hash[32..48],"name":e["kind"],"startTimeUnixNano":start.to_string(),"endTimeUnixNano":(start+ms*1_000_000+1).to_string(),"attributes":[{"key":"astra.event.seq","value":{"intValue":e["seq"].to_string()}},{"key":"astra.project.id","value":{"stringValue":digest(store.root.to_string_lossy().as_bytes())}}],"status":{"code":status}}));
    }
    Ok(
        json!({"resourceSpans":[{"resource":{"attributes":[{"key":"service.name","value":{"stringValue":"astra-watch"}}]},"scopeSpans":[{"scope":{"name":"astra-watch","version":env!("CARGO_PKG_VERSION")},"spans":spans}]}]}),
    )
}
pub fn export(store: &Store, url: &str, allow_remote: bool, after: i64) -> Result<Value> {
    let parsed = reqwest::Url::parse(url)?;
    ensure!(
        matches!(parsed.scheme(), "http" | "https"),
        "OTLP endpoint must be HTTP(S)"
    );
    ensure!(
        parsed.username().is_empty() && parsed.password().is_none(),
        "do not put credentials into an export URL"
    );
    ensure!(
        allow_remote
            || matches!(
                parsed.host_str(),
                Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
            ),
        "remote telemetry export needs --allow-remote; project data stays local by default"
    );
    let batch = store.events(after, 1000)?;
    let next_cursor = batch
        .last()
        .and_then(|e| e["seq"].as_i64())
        .unwrap_or(after);
    let mut payload = otlp(store, after)?;
    payload["resourceSpans"][0]["scopeSpans"][0]["spans"]
        .as_array_mut()
        .unwrap()
        .truncate(batch.len());
    let response = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .build()?
        .post(url)
        .json(&payload)
        .send()?
        .error_for_status()?;
    let body: Value = response.json()?;
    ensure!(
        body.get("partialSuccess").is_none_or(|v| v
            .get("rejectedSpans")
            .is_none_or(|n| n.as_str() == Some("0") || n.as_u64() == Some(0))),
        "collector rejected spans"
    );
    Ok(
        json!({"exported":true,"response":body,"cursor_after":next_cursor,"span_count":batch.len(),"delivery":"explicit bounded batch; caller advances cursor only after acknowledgment; repeated exports may duplicate spans"}),
    )
}
pub fn from_file(store: &mut Store, path: &Path) -> Result<Value> {
    let bytes = std::fs::read(path)?;
    ensure!(
        bytes.len() <= 1_000_000,
        "usage import exceeds one megabyte"
    );
    import(store, serde_json::from_slice(&bytes)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cache_counters_are_distinct_nullable_and_deduplicated() {
        let d = tempfile::tempdir().unwrap();
        let mut s = Store::open(d.path()).unwrap();
        let u = Usage {
            source: "codex:fixture".into(),
            event_id: "1".into(),
            input_tokens: Some(100),
            cached_input_tokens: Some(80),
            output_tokens: Some(10),
            reasoning_output_tokens: None,
            prompt_cache_write_tokens: None,
            mcp_tool_cache_write_duration_ms: Some(1.),
        };
        import(&mut s, u.clone()).unwrap();
        import(&mut s, u.clone()).unwrap();
        assert_eq!(summary(&s).unwrap()["cached_input_tokens"], 80);
        assert_eq!(summary(&s).unwrap()["imported_events"], 1);
        assert!(summary(&s).unwrap()["prompt_cache_write_tokens"].is_null());
        let mut invalid = u;
        invalid.prompt_cache_write_tokens = Some(80);
        assert!(import(&mut s, invalid).is_err());
    }
    #[test]
    fn common_secrets_are_redacted() {
        assert!(!redact("PASSWORD=abc api_key=123 sk-abcdefghijklmnopqrstuvwxyz").contains("abc"));
    }
}
