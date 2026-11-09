//! Ten stable tools. Every call has an explicit project binding, not a mutable last-root.
use crate::{
    files,
    store::{Store, event},
};
use rmcp::{
    RoleServer, ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Implementation, ServerCapabilities, ServerConfig},
    service::RequestContext,
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc, time::Instant};

#[derive(Clone)]
pub struct Server {
    default_root: PathBuf,
    tool_router: ToolRouter<Self>,
    limit: Arc<tokio::sync::Semaphore>,
}
#[derive(Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Root {
    /// Absolute connected project root; omission uses the server launch directory.
    pub root: Option<PathBuf>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Bootstrap {
    pub root: Option<PathBuf>,
    #[serde(default)]
    pub no_native_config: bool,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Context {
    pub root: Option<PathBuf>,
    pub query: String,
    pub limit: Option<usize>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Graph {
    pub root: Option<PathBuf>,
    pub query: Option<String>,
    pub limit: Option<usize>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub root: Option<PathBuf>,
    pub request: crate::tasks::Request,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Verification {
    pub root: Option<PathBuf>,
    pub task: String,
    pub criterion: String,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    pub root: Option<PathBuf>,
    pub title: String,
    pub decision: String,
    pub alternatives: Vec<String>,
    pub supersedes: Option<String>,
}
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Git {
    pub root: Option<PathBuf>,
    pub task: Option<String>,
}

impl Server {
    pub fn new(default_root: PathBuf) -> Self {
        Self {
            default_root,
            tool_router: Self::tool_router(),
            limit: Arc::new(tokio::sync::Semaphore::new(4)),
        }
    }
    async fn call(
        &self,
        requested: Option<PathBuf>,
        name: &'static str,
        ctx: RequestContext<RoleServer>,
        f: impl FnOnce(PathBuf) -> anyhow::Result<Value> + Send + 'static,
    ) -> CallToolResult {
        let root = requested.unwrap_or_else(|| self.default_root.clone());
        if !root.is_absolute() {
            return CallToolResult::structured_error(
                json!({"code":"invalid_root","message":"pass an absolute project root"}),
            );
        }
        let permit = tokio::select! {r=self.limit.clone().acquire_owned()=>r.ok(),_=ctx.ct.cancelled()=>None};
        let Some(permit) = permit else {
            return CallToolResult::structured_error(json!({"code":"cancelled"}));
        };
        let token = ctx.ct.clone();
        let _drop_guard = token.clone().drop_guard();
        let worker_token = token.clone();
        let job = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            crate::operation::scoped(worker_token, || {
                let root = files::project_root(Some(&root))?;
                let started = Instant::now();
                let value = f(root.clone());
                if name != "watch_bootstrap"
                    && let Ok(mut store) = Store::open(&root)
                {
                    let _=store.write(|tx|event(tx,"mcp.tool",&json!({"tool":name,"ok":value.is_ok(),"duration_ms":started.elapsed().as_millis()})));
                }
                value
            })
        });
        let result = tokio::select! {r=job=>r.map_err(anyhow::Error::from).and_then(|r|r),_=token.cancelled()=>Err(anyhow::anyhow!("request cancelled"))};
        match result {
            Ok(value) => CallToolResult::structured(value),
            Err(e) => CallToolResult::structured_error(
                json!({"code":"operation_rejected","message":crate::telemetry::redact(&format!("{e:#}")),"next":"inspect current state/profile, fix the reported condition, then retry; do not bypass safety gates"}),
            ),
        }
    }
}
#[tool_router]
impl Server {
    #[tool(
        description = "Idempotently bootstrap the connected project: preserve user files, add a namespaced profile and AGENTS bridge, suggest native roles. No model execution or global trust/config changes.",
        annotations(
            read_only_hint = false,
            idempotent_hint = true,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn watch_bootstrap(
        &self,
        Parameters(p): Parameters<Bootstrap>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.call(p.root, "watch_bootstrap", ctx, move |r| {
            crate::bootstrap::bootstrap(Some(&r), !p.no_native_config)
        })
        .await
    }
    #[tool(
        description = "Read current mission/profile, task versions, cooperative controls, recent auditable events and nullable native usage. Reconcile before each bounded delivery slice.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn watch_state(
        &self,
        Parameters(p): Parameters<Root>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.call(p.root, "watch_state", ctx, |r| {
            crate::integration::state(&Store::open(&r)?)
        })
        .await
    }
    #[tool(
        description = "Retrieve bounded current source/doc evidence with content hashes, paths and lines. Incrementally refreshes local project storage; lexical/symbol search, no embeddings or semantic LSP.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn watch_context(
        &self,
        Parameters(p): Parameters<Context>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.call(p.root, "watch_context", ctx, move |r| {
            crate::knowledge::context(&mut Store::open(&r)?, &p.query, p.limit)
        })
        .await
    }
    #[tool(
        description = "Return portable Mermaid syntax-containment graph and source hashes, up to 80 nodes. Not a resolved call/dependency graph. Use configured LSP separately for semantic references.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn watch_graph(
        &self,
        Parameters(p): Parameters<Graph>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.call(p.root, "watch_graph", ctx, move |r| {
            crate::knowledge::graph(
                &mut Store::open(&r)?,
                p.query.as_deref(),
                p.limit.unwrap_or(40),
            )
        })
        .await
    }
    #[tool(
        description = "Queue/start/pause/resume/cancel/steer/checkpoint/complete a mission-bound task. Existing mutations require expected_version. Enforces leases, slice bounds and current per-criterion executed evidence; native Codex owns actual agents.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn watch_task(
        &self,
        Parameters(p): Parameters<Task>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.call(p.root, "watch_task", ctx, move |r| {
            crate::tasks::mutate(&mut Store::open(&r)?, p.request)
        })
        .await
    }
    #[tool(
        description = "Plan the CLI verification invocation for native Codex shell; DOES NOT execute commands. Review trusted project check argv first. Only actual CLI execution can record a passing check.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn watch_verification(
        &self,
        Parameters(p): Parameters<Verification>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.call(p.root, "watch_verification", ctx, move |r| {
            crate::verify::plan(&r, &p.task, &p.criterion)
        })
        .await
    }
    #[tool(
        description = "Write an immutable concise decision with alternatives and optional supersedes ID. Prior decisions are never updated. Record evidence and decisions, not private reasoning transcripts.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            open_world_hint = false
        )
    )]
    async fn watch_decision(
        &self,
        Parameters(p): Parameters<Decision>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.call(p.root, "watch_decision", ctx, move |r| {
            crate::tasks::decision(
                &mut Store::open(&r)?,
                &p.title,
                &p.decision,
                &p.alternatives,
                p.supersedes.as_deref(),
            )
        })
        .await
    }
    #[tool(
        description = "Read Git state, verify optional slice evidence, propose a guarded checkpoint and native scheduling trial. Never stages, commits, pushes or schedules. No-op is a valid outcome.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn watch_git(
        &self,
        Parameters(p): Parameters<Git>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.call(p.root, "watch_git", ctx, move |r| {
            crate::integration::git_plan(&Store::open(&r)?, p.task.as_deref())
        })
        .await
    }
    #[tool(
        description = "Plan language docs, optional terminal/browser recordings and explicitly approved GitHub Pages publication using installed generators/skills. Does not mutate source or deploy.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn watch_docs(
        &self,
        Parameters(p): Parameters<Root>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.call(p.root, "watch_docs", ctx, |r| {
            crate::integration::docs_plan(&r)
        })
        .await
    }
    #[tool(
        description = "Summarize explicitly imported native token usage and local telemetry coverage. cached_input_tokens are distinct from unknown prompt-cache writes and MCP cache-write timing. No network export.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn watch_telemetry(
        &self,
        Parameters(p): Parameters<Root>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.call(p.root, "watch_telemetry", ctx, |r| {
            crate::telemetry::summary(&Store::open(&r)?)
        })
        .await
    }
}
#[tool_handler(router = self.tool_router)]
impl ServerHandler for Server {
    fn get_info(&self) -> ServerConfig {
        let mut config = ServerConfig::default();
        config.capabilities = ServerCapabilities::builder().enable_tools().build();
        config.server_info = Implementation::new("astra-watch", env!("CARGO_PKG_VERSION"));
        config.instructions=Some("Bind every operation to the absolute connected project root. Native Codex owns model execution, permissions, subagents, worktrees and scheduling; Astra Watch owns project evidence/contracts only.".into());
        config
    }
}
pub async fn serve(root: PathBuf) -> anyhow::Result<()> {
    Server::new(root)
        .serve(rmcp::transport::stdio())
        .await?
        .waiting()
        .await?;
    Ok(())
}
