//! Real CLI, actual compiler/check execution and MCP wire tests in disposable projects.
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::Duration,
};
fn binary() -> &'static str {
    env!("CARGO_BIN_EXE_astra-watch")
}
fn cli(root: &std::path::Path, args: &[&str]) -> Value {
    let r = Command::new(binary())
        .arg("--root")
        .arg(root)
        .args(args)
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    serde_json::from_slice(&r.stdout).unwrap()
}
fn task(root: &std::path::Path, request: Value) -> Value {
    cli(root, &["task", "--request", &request.to_string()])
}

#[test]
fn real_rust_delivery_cli_survives_reopen_and_requires_all_current_checks() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path();
    fs::create_dir(root.join("src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname='fixture_calculator'\nversion='0.1.0'\nedition='2024'\n",
    )
    .unwrap();
    fs::write(
        root.join("src/lib.rs"),
        "pub fn add(a:i32,b:i32)->i32 { a+b }\n#[test] fn works(){assert_eq!(add(20,22),42);}\n",
    )
    .unwrap();
    assert!(
        Command::new("cargo")
            .args(["test", "--offline"])
            .current_dir(root)
            .output()
            .unwrap()
            .status
            .success()
    );
    let boot = cli(root, &["bootstrap", "--no-native-config"]);
    assert_eq!(boot["native_prompt_replaced"], false);
    let (mut profile, _) = astra_watch::config::load(root).unwrap();
    profile.mission.outcome = "Verified calculator".into();
    profile
        .mission
        .criteria
        .push(astra_watch::config::Criterion {
            id: "sum".into(),
            description: "Correct addition under tests".into(),
            check: Some("rust-tests".into()),
        });
    astra_watch::config::save(root, &profile).unwrap();
    task(
        root,
        json!({"action":"queue","id":"slice1","title":"Calculator slice","criteria":["sum"],"write_scope":["src"]}),
    );
    task(
        root,
        json!({"action":"start","id":"slice1","expected_version":1}),
    );
    let e = cli(root, &["verify", "--task", "slice1", "--criterion", "sum"]);
    assert_eq!(e["passed"], true);
    let result = task(
        root,
        json!({"action":"complete","id":"slice1","expected_version":2}),
    );
    assert_eq!(result["task"]["status"], "complete");
    assert_eq!(cli(root, &["state"])["audit_chain"], "verified");
    assert_eq!(
        cli(root, &["context", "addition"])["index"]["semantic_lsp"],
        false
    );
}

struct Wire {
    child: Child,
    rx: mpsc::Receiver<Value>,
}
impl Drop for Wire {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Wire {
    fn new(root: &std::path::Path) -> Self {
        let mut child = Command::new(binary())
            .arg("--root")
            .arg(root)
            .arg("serve")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else {
                    break;
                };
                let parsed = serde_json::from_str(&line).expect("stdio must contain JSON-RPC only");
                if tx.send(parsed).is_err() {
                    break;
                }
            }
        });
        Self { child, rx }
    }
    fn send(&mut self, v: Value) {
        writeln!(self.child.stdin.as_mut().unwrap(), "{v}").unwrap();
        self.child.stdin.as_mut().unwrap().flush().unwrap();
    }
    fn request(&mut self, id: u64, method: &str, params: Value) -> Value {
        self.send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}));
        loop {
            let v = self
                .rx
                .recv_timeout(Duration::from_secs(10))
                .expect("MCP response deadline");
            if v["id"] == id {
                return v;
            }
        }
    }
}
#[test]
fn mcp_initialize_stable_names_schema_and_cross_project_binding() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let mut w = Wire::new(a.path());
    let init=w.request(1,"initialize",json!({"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"astra-test","version":"1"}}));
    assert_eq!(init["result"]["serverInfo"]["name"], "astra-watch");
    w.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    let first = w.request(2, "tools/list", json!({}));
    let names: Vec<_> = first["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(names.len(), 10);
    assert!(names.contains(&"watch_bootstrap"));
    assert!(names.contains(&"watch_verification"));
    let boot = w.request(
        3,
        "tools/call",
        json!({"name":"watch_bootstrap","arguments":{"root":b.path(),"no_native_config":true}}),
    );
    assert_eq!(boot["result"]["isError"], false);
    assert!(b.path().join("project.config.toml").exists());
    assert!(!a.path().join("project.config.toml").exists());
    let state = w.request(
        4,
        "tools/call",
        json!({"name":"watch_state","arguments":{"root":b.path()}}),
    );
    assert_eq!(state["result"]["structuredContent"]["mission_ready"], false);
    let second = w.request(5, "tools/list", json!({}));
    assert_eq!(first["result"]["tools"], second["result"]["tools"]);
    let bad = w.request(
        6,
        "tools/call",
        json!({"name":"watch_state","arguments":{"root":"relative-project"}}),
    );
    assert_eq!(bad["result"]["isError"], true);
}

#[test]
fn all_criteria_are_required_not_just_one_worker_pass() {
    let d = tempfile::tempdir().unwrap();
    cli(d.path(), &["bootstrap", "--no-native-config"]);
    let (mut c, _) = astra_watch::config::load(d.path()).unwrap();
    c.mission.outcome = "Two requirements".into();
    c.checks.insert(
        "ok".into(),
        astra_watch::config::Check {
            program: "true".into(),
            args: vec![],
            timeout_secs: 2,
        },
    );
    for id in ["a", "b"] {
        c.mission.criteria.push(astra_watch::config::Criterion {
            id: id.into(),
            description: id.into(),
            check: Some("ok".into()),
        });
    }
    astra_watch::config::save(d.path(), &c).unwrap();
    task(
        d.path(),
        json!({"action":"queue","id":"both","title":"Both","criteria":["a","b"],"write_scope":["src"]}),
    );
    task(
        d.path(),
        json!({"action":"start","id":"both","expected_version":1}),
    );
    cli(d.path(), &["verify", "--task", "both", "--criterion", "a"]);
    let rejected = Command::new(binary())
        .arg("--root")
        .arg(d.path())
        .args([
            "task",
            "--request",
            &json!({"action":"complete","id":"both","expected_version":2}).to_string(),
        ])
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    cli(d.path(), &["verify", "--task", "both", "--criterion", "b"]);
    task(
        d.path(),
        json!({"action":"complete","id":"both","expected_version":2}),
    );
}

#[test]
fn dashboard_cancellation_stops_an_actual_verification_process() {
    let d = tempfile::tempdir().unwrap();
    cli(d.path(), &["bootstrap", "--no-native-config"]);
    let (mut c, _) = astra_watch::config::load(d.path()).unwrap();
    c.mission.outcome = "Cancellation".into();
    c.checks.insert(
        "slow".into(),
        astra_watch::config::Check {
            program: "sleep".into(),
            args: vec!["10".into()],
            timeout_secs: 20,
        },
    );
    c.mission.criteria.push(astra_watch::config::Criterion {
        id: "slow".into(),
        description: "Stop safely".into(),
        check: Some("slow".into()),
    });
    astra_watch::config::save(d.path(), &c).unwrap();
    task(
        d.path(),
        json!({"action":"queue","id":"slow","title":"Slow check","criteria":["slow"],"write_scope":["src"]}),
    );
    task(
        d.path(),
        json!({"action":"start","id":"slow","expected_version":1}),
    );
    let mut dashboard = Command::new(binary())
        .arg("--root")
        .arg(d.path())
        .arg("dashboard")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let err = dashboard.stderr.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        BufReader::new(err).read_line(&mut line).unwrap();
        tx.send(line).unwrap();
    });
    let address = rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let private_url = address
        .trim()
        .strip_prefix("Astra Watch dashboard: ")
        .unwrap();
    let (base, token) = private_url.split_once('#').unwrap();
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .unwrap();
    let api = format!("{base}api/");
    assert_eq!(
        client.get(format!("{api}state")).send().unwrap().status(),
        401
    );
    assert_eq!(
        client
            .get(format!("{api}state"))
            .bearer_auth(token)
            .header("Origin", "https://foreign.example")
            .send()
            .unwrap()
            .status(),
        403
    );
    let state: Value = client
        .get(format!("{api}state"))
        .bearer_auth(token)
        .send()
        .unwrap()
        .json()
        .unwrap();
    assert_eq!(state["tasks"][0]["status"], "running");
    let mut child = Command::new(binary())
        .arg("--root")
        .arg(d.path())
        .args(["verify", "--task", "slow", "--criterion", "slow"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let cancelled = client
        .post(format!("{api}control"))
        .bearer_auth(token)
        .header("Origin", base.trim_end_matches('/'))
        .json(&json!({"action":"cancel","id":"slow","expected_version":2}))
        .send()
        .unwrap();
    assert!(cancelled.status().is_success());
    let _ = dashboard.kill();
    let _ = dashboard.wait();
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        if std::time::Instant::now() > deadline {
            let _ = child.kill();
            panic!("cancel did not stop verification");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let out = child.wait_with_output().unwrap();
    assert!(!out.status.success());
    let e: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(e["interrupted"], true);
    assert_eq!(e["passed"], false);
}

#[test]
fn otlp_http_export_has_valid_ids_cursor_and_explicit_delivery() {
    use std::io::Read;
    let d = tempfile::tempdir().unwrap();
    cli(d.path(), &["bootstrap", "--no-native-config"]);
    let store = astra_watch::store::Store::open(d.path()).unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1/traces", listener.local_addr().unwrap());
    let receiver = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut length = 0;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
            if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
                length = value.trim().parse::<usize>().unwrap();
            }
        }
        assert!((1..1_000_000).contains(&length));
        let mut bytes = vec![0; length];
        reader.read_exact(&mut bytes).unwrap();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        for span in body["resourceSpans"][0]["scopeSpans"][0]["spans"]
            .as_array()
            .unwrap()
        {
            assert_eq!(span["traceId"].as_str().unwrap().len(), 32);
            assert_eq!(span["spanId"].as_str().unwrap().len(), 16);
            assert!(
                span["startTimeUnixNano"]
                    .as_str()
                    .unwrap()
                    .parse::<u128>()
                    .unwrap()
                    > 0
            );
        }
        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}").unwrap();
        body
    });
    let result = astra_watch::telemetry::export(&store, &url, false, 0).unwrap();
    assert_eq!(result["exported"], true);
    assert!(result["cursor_after"].as_i64().unwrap() > 0);
    receiver.join().unwrap();
    assert!(
        astra_watch::telemetry::export(&store, "https://example.com/v1/traces", false, 0).is_err()
    );
}
