//! Loopback-only live human controls; never claims to kill the native Codex model turn.
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Html,
    routing::{get, post},
};
use serde_json::Value;
use std::{path::PathBuf, sync::Arc};
#[derive(Clone)]
struct App {
    root: PathBuf,
    token: String,
    origin: String,
    host: String,
}
fn authorize(a: &App, h: &HeaderMap) -> Result<(), StatusCode> {
    if h.get("host").and_then(|v| v.to_str().ok()) != Some(&a.host) {
        return Err(StatusCode::FORBIDDEN);
    }
    if h.get("authorization").and_then(|v| v.to_str().ok()) != Some(&format!("Bearer {}", a.token))
    {
        return Err(StatusCode::UNAUTHORIZED);
    }
    if h.get("origin")
        .is_some_and(|v| v.to_str().ok() != Some(&a.origin))
    {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(())
}
async fn state(
    State(a): State<Arc<App>>,
    h: HeaderMap,
) -> Result<Json<Value>, (StatusCode, String)> {
    authorize(&a, &h).map_err(|s| (s, "local authorization required".into()))?;
    tokio::task::spawn_blocking(move || {
        crate::integration::state(&crate::store::Store::open(&a.root)?)
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map(Json)
    .map_err(|e| {
        (
            StatusCode::CONFLICT,
            crate::telemetry::redact(&e.to_string()),
        )
    })
}
async fn control(
    State(a): State<Arc<App>>,
    h: HeaderMap,
    Json(r): Json<crate::tasks::Request>,
) -> Result<Json<Value>, (StatusCode, String)> {
    authorize(&a, &h).map_err(|s| (s, "local authorization required".into()))?;
    tokio::task::spawn_blocking(move || {
        crate::tasks::mutate(&mut crate::store::Store::open(&a.root)?, r)
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map(Json)
    .map_err(|e| {
        (
            StatusCode::CONFLICT,
            crate::telemetry::redact(&e.to_string()),
        )
    })
}
pub async fn run(root: PathBuf, port: u16) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await?;
    let address = listener.local_addr()?;
    let token = uuid::Uuid::new_v4().simple().to_string();
    let app = Arc::new(App {
        root,
        token: token.clone(),
        host: address.to_string(),
        origin: format!("http://{address}"),
    });
    eprintln!(
        "Astra Watch dashboard: http://{address}/#{token}\nLocal bearer access; do not share the URL. Ctrl-C stops the dashboard, not native Codex turns."
    );
    let routes = Router::new()
        .route("/", get(|| async { Html(PAGE) }))
        .route("/api/state", get(state))
        .route("/api/control", post(control))
        .layer(axum::extract::DefaultBodyLimit::max(32000))
        .with_state(app);
    axum::serve(listener, routes)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
const PAGE: &str = r##"<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>Astra Watch</title><style>body{font:16px system-ui;max-width:1100px;margin:2rem auto;padding:0 1rem;background:#101720;color:#e7edf5}button,input,textarea{font:inherit;padding:.5rem;margin:.3rem;color:inherit;background:#263548;border:1px solid #8292a6;border-radius:.3rem}button{cursor:pointer}pre{white-space:pre-wrap;overflow-wrap:anywhere;background:#192635;padding:1rem}article{border:1px solid #8292a6;padding:1rem;margin:1rem 0}label{display:block}textarea{width:85%;height:5rem}a{color:#9ad6ff} .error{color:#ffadad}</style><h1>Astra Watch</h1><p>Current project contracts, checks and controls. Pause/cancel stops Astra verification; use Codex's own interrupt button for a native model turn. Leases are cooperative, not an OS write sandbox.</p><p id="status" role="status" aria-live="polite"></p><h2>Tasks</h2><div id="tasks"></div><details><summary>Queue a task under the accepted mission</summary><form id="queue"><label>ID <input name="id" required></label><label>Title <input name="title" required></label><label>Criterion IDs (comma-separated) <input name="criteria" required></label><label>Allowed relative paths (comma-separated; no globs) <input name="scope" required></label><button>Queue</button></form></details><h2>Mission and telemetry</h2><pre id="summary"></pre><h2>Recent audit events</h2><pre id="events"></pre><script>
const token=location.hash.slice(1);history.replaceState(null,'',location.pathname);let busy=false;
async function api(path,body){const r=await fetch('/api/'+path,{method:body?'POST':'GET',headers:{Authorization:'Bearer '+token,'Content-Type':'application/json'},body:body?JSON.stringify(body):undefined});if(!r.ok)throw Error(await r.text());return r.json();}
function status(t,error=false){const e=document.querySelector('#status');e.textContent=t;e.className=error?'error':'';}
async function control(r){try{await api('control',r);await refresh();}catch(e){status(e.message,true);}}
function button(label,fn){const b=document.createElement('button');b.textContent=label;b.onclick=fn;return b;}
async function refresh(){if(busy)return;busy=true;try{const s=await api('state');document.querySelector('#summary').textContent=JSON.stringify({root:s.root,mission:s.mission,mission_ready:s.mission_ready,telemetry:s.telemetry},null,2);document.querySelector('#events').textContent=JSON.stringify(s.recent_events,null,2);const box=document.querySelector('#tasks');box.replaceChildren();for(const t of s.tasks){const a=document.createElement('article');const h=document.createElement('h3');h.textContent=t.id+' · '+t.status+' · v'+t.version+' · slice '+t.slice;a.append(h);const p=document.createElement('p');p.textContent=t.title+' | scope: '+t.write_scope.join(', ')+' | profile '+(t.profile_current?'current':'CHANGED');a.append(p);for(const [label,action] of [['Start','start'],['Pause','pause'],['Resume','resume'],['Cancel verification/task','cancel'],['Complete with evidence','complete']])a.append(button(label,()=>{if(action==='cancel'&&!confirm('Cancel this task and its active Astra check?'))return;control({action,id:t.id,expected_version:t.version});}));a.append(button('Steer',()=>{const note=prompt('Tactic adjustment (mission and write scope stay fixed)');if(note)control({action:'steer',id:t.id,expected_version:t.version,note});}));box.append(a);}status('Live · audit '+s.audit_chain+' · cursor '+s.last_event_cursor);}catch(e){status(e.message,true);}finally{busy=false;}}
document.querySelector('#queue').onsubmit=e=>{e.preventDefault();const f=new FormData(e.target);control({action:'queue',id:f.get('id'),title:f.get('title'),criteria:f.get('criteria').split(',').map(x=>x.trim()),write_scope:f.get('scope').split(',').map(x=>x.trim())});};refresh();setInterval(refresh,2000);
</script></html>"##;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_auth_rejects_foreign_origins_and_dns_rebinding() {
        let a = App {
            root: "/tmp".into(),
            token: "secret".into(),
            origin: "http://127.0.0.1:1".into(),
            host: "127.0.0.1:1".into(),
        };
        let mut h = HeaderMap::new();
        h.insert("host", "127.0.0.1:1".parse().unwrap());
        h.insert("authorization", "Bearer secret".parse().unwrap());
        assert!(authorize(&a, &h).is_ok());
        h.insert("origin", "https://evil.example".parse().unwrap());
        assert!(authorize(&a, &h).is_err());
        h.remove("origin");
        h.insert("host", "evil.example".parse().unwrap());
        assert!(authorize(&a, &h).is_err());
    }
}
