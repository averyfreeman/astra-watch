use anyhow::{Result, ensure};
use astra_watch::{files, store::Store};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// Absolute project root; the connected working directory is the default.
    #[arg(long, global = true)]
    root: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create an idempotent profile and native instruction bridge.
    Bootstrap {
        /// Do not create optional trusted-project Codex configuration/roles.
        #[arg(long)]
        no_native_config: bool,
    },
    /// Verify the durable event chain and report the current profile.
    State,
    /// Serve the ten stable tools over MCP stdio.
    Serve,
    /// Incrementally refresh local project retrieval storage.
    Index,
    Context {
        query: String,
        #[arg(long)]
        limit: Option<usize>,
    },
    Graph {
        #[arg(long)]
        query: Option<String>,
        #[arg(long, default_value_t = 40)]
        limit: usize,
    },
    /// A typed JSON task request; see the delivery skill for examples.
    Task {
        #[arg(long)]
        request: String,
    },
    Verify {
        #[arg(long)]
        task: String,
        #[arg(long)]
        criterion: String,
    },
    Decision {
        #[arg(long)]
        title: String,
        #[arg(long)]
        decision: String,
        #[arg(long)]
        alternative: Vec<String>,
        #[arg(long)]
        supersedes: Option<String>,
    },
    Git {
        #[arg(long)]
        task: Option<String>,
    },
    Docs,
    Telemetry {
        #[arg(long)]
        import: Option<PathBuf>,
        #[arg(long)]
        export: Option<String>,
        #[arg(long)]
        allow_remote: bool,
        #[arg(long, default_value_t = 0)]
        after: i64,
        #[arg(long)]
        otlp: bool,
    },
    Dashboard {
        #[arg(long, default_value_t = 0)]
        port: u16,
    },
    Artifact {
        id: String,
    },
    Events {
        #[arg(long, default_value_t = 0)]
        after: i64,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Print the typed project-profile JSON schema.
    Schema,
    Cadence {
        #[command(subcommand)]
        command: CadenceCommand,
    },
}
#[derive(Subcommand)]
enum CadenceCommand {
    Create {
        #[arg(long)]
        id: String,
        #[arg(long)]
        max_runs: u32,
        #[arg(long)]
        expires_unix: u64,
        #[arg(long)]
        isolated_checkout: bool,
    },
    Tick {
        #[arg(long)]
        id: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = files::project_root(cli.root.as_deref())?;
    let value = match cli.command {
        Command::Bootstrap { no_native_config } => {
            astra_watch::bootstrap::bootstrap(cli.root.as_deref(), !no_native_config)?
        }
        Command::Serve => {
            astra_watch::mcp::serve(root).await?;
            return Ok(());
        }
        Command::Dashboard { port } => {
            astra_watch::dashboard::run(root, port).await?;
            return Ok(());
        }
        Command::State => astra_watch::integration::state(&Store::open(&root)?)?,
        Command::Index => astra_watch::knowledge::refresh(&mut Store::open(&root)?)?,
        Command::Context { query, limit } => {
            astra_watch::knowledge::context(&mut Store::open(&root)?, &query, limit)?
        }
        Command::Graph { query, limit } => {
            astra_watch::knowledge::graph(&mut Store::open(&root)?, query.as_deref(), limit)?
        }
        Command::Task { request } => {
            ensure!(request.len() <= 32000, "task request exceeds 32KB");
            astra_watch::tasks::mutate(&mut Store::open(&root)?, serde_json::from_str(&request)?)?
        }
        Command::Verify { task, criterion } => astra_watch::verify::run(&root, &task, &criterion)?,
        Command::Decision {
            title,
            decision,
            alternative,
            supersedes,
        } => astra_watch::tasks::decision(
            &mut Store::open(&root)?,
            &title,
            &decision,
            &alternative,
            supersedes.as_deref(),
        )?,
        Command::Git { task } => {
            astra_watch::integration::git_plan(&Store::open(&root)?, task.as_deref())?
        }
        Command::Docs => astra_watch::integration::docs_plan(&root)?,
        Command::Telemetry {
            import,
            export,
            allow_remote,
            after,
            otlp,
        } => {
            let mut store = Store::open(&root)?;
            ensure!(
                usize::from(import.is_some()) + usize::from(export.is_some()) + usize::from(otlp)
                    <= 1,
                "select one telemetry operation"
            );
            if let Some(path) = import {
                astra_watch::telemetry::from_file(&mut store, &path)?
            } else if let Some(url) = export {
                astra_watch::telemetry::export(&store, &url, allow_remote, after)?
            } else if otlp {
                astra_watch::telemetry::otlp(&store, after)?
            } else {
                astra_watch::telemetry::summary(&store)?
            }
        }
        Command::Artifact { id } => Store::open(&root)?.read_artifact(&id)?,
        Command::Events { after, limit } => {
            serde_json::json!({"events":Store::open(&root)?.events(after,limit)?})
        }
        Command::Schema => {
            serde_json::to_value(schemars::schema_for!(astra_watch::config::Settings))?
        }
        Command::Cadence { command } => {
            let mut store = Store::open(&root)?;
            match command {
                CadenceCommand::Create {
                    id,
                    max_runs,
                    expires_unix,
                    isolated_checkout,
                } => astra_watch::cadence::create(
                    &mut store,
                    &id,
                    max_runs,
                    expires_unix,
                    isolated_checkout,
                )?,
                CadenceCommand::Tick { id } => astra_watch::cadence::tick(&mut store, &id)?,
            }
        }
    };
    println!("{}", serde_json::to_string_pretty(&value)?);
    ensure!(
        value.get("passed").is_none_or(|v| v != false),
        "verification did not pass; evidence was recorded"
    );
    Ok(())
}
