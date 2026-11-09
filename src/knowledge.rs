//! Local lexical retrieval and syntax graphs. No embedding service or LSP claim.
use crate::{
    config, digest, files,
    store::{Store, event},
};
use anyhow::{Context, Result, ensure};
use ignore::WalkBuilder;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::Path,
    process::Command,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Snapshot {
    pub digest: String,
    pub files: BTreeMap<String, String>,
}

fn owned_or_build(path: &str) -> bool {
    path.split('/').any(|c| {
        matches!(
            c,
            ".git" | "target" | "node_modules" | ".venv" | "__pycache__" | "dist" | "build"
        )
    }) || path.starts_with(".astra-watch/runtime/")
        || path.starts_with(".astra-watch/records/")
}
fn secret(path: &str) -> bool {
    path.split('/').any(|c| {
        c == ".env"
            || c.starts_with(".env.")
            || c.ends_with(".pem")
            || c.ends_with(".key")
            || matches!(c, "credentials" | "secrets.toml" | "id_rsa" | "id_ed25519")
    })
}

pub fn paths(root: &Path, max: usize) -> Result<Vec<String>> {
    let mut found = BTreeSet::new();
    let started = Instant::now();
    let mut builder = WalkBuilder::new(root);
    builder
        .hidden(false)
        .follow_links(false)
        .git_ignore(true)
        .git_exclude(true)
        .parents(false)
        .require_git(false);
    let filter_root = root.to_path_buf();
    builder.filter_entry(move |e| {
        e.depth() == 0
            || !owned_or_build(
                &e.path()
                    .strip_prefix(&filter_root)
                    .unwrap_or(e.path())
                    .to_string_lossy()
                    .replace('\\', "/"),
            ) && !matches!(
                e.path()
                    .strip_prefix(&filter_root)
                    .ok()
                    .and_then(|p| p.to_str()),
                Some(".astra-watch/runtime" | ".astra-watch/records")
            )
    });
    for entry in builder.build() {
        crate::operation::check()?;
        ensure!(
            started.elapsed() < Duration::from_secs(20),
            "source inventory exceeded 20 seconds; narrow the project root"
        );
        let e = entry?;
        if !e.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let path = e
            .path()
            .strip_prefix(root)?
            .to_str()
            .context("non-UTF-8 source path is unsupported")?
            .replace('\\', "/");
        if !owned_or_build(&path) {
            found.insert(path);
        }
        ensure!(
            found.len() <= max,
            "source inventory exceeds max_files; snapshot is not complete"
        );
    }
    // Include tracked ignored files, but never accidentally inventory a parent repository.
    if let Ok(top) = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        && top.status.success()
        && fs::canonicalize(String::from_utf8_lossy(&top.stdout).trim())
            .ok()
            .as_deref()
            == Some(root)
    {
        let listed = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["ls-files", "-z"])
            .output()?;
        ensure!(listed.status.success(), "cannot inventory tracked files");
        for bytes in listed.stdout.split(|b| *b == 0).filter(|b| !b.is_empty()) {
            let path = std::str::from_utf8(bytes)?.to_string();
            if !owned_or_build(&path) && files::inside(root, &path)?.is_file() {
                found.insert(path);
            }
            ensure!(found.len() <= max, "tracked inventory exceeds max_files");
        }
    }
    Ok(found.into_iter().collect())
}

pub fn snapshot(root: &Path) -> Result<Snapshot> {
    let (s, _) = config::load(root)?;
    let mut hashes = BTreeMap::new();
    let started = Instant::now();
    for path in paths(root, s.knowledge.max_files)? {
        crate::operation::check()?;
        ensure!(
            started.elapsed() < Duration::from_secs(30),
            "snapshot deadline exceeded; no evidence recorded"
        );
        let mut file = fs::File::open(files::inside(root, &path)?)?;
        let mut hasher = Sha256::new();
        let mut buf = [0u8; 65536];
        loop {
            crate::operation::check()?;
            let n = file.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            ensure!(
                started.elapsed() < Duration::from_secs(30),
                "snapshot deadline exceeded"
            );
        }
        hashes.insert(path, hex::encode(hasher.finalize()));
    }
    Ok(Snapshot {
        digest: digest(serde_json::to_vec(&hashes)?),
        files: hashes,
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Chunk {
    pub path: String,
    pub line_start: usize,
    pub line_end: usize,
    pub symbol: Option<String>,
    pub kind: String,
    pub text: String,
    pub source_hash: String,
    pub parse_errors: bool,
}

fn language(path: &str) -> Option<tree_sitter::Language> {
    match Path::new(path).extension()?.to_str()? {
        "rs" => Some(tree_sitter_rust::LANGUAGE.into()),
        "go" => Some(tree_sitter_go::LANGUAGE.into()),
        "py" => Some(tree_sitter_python::LANGUAGE.into()),
        "ts" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        "tsx" => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
        "js" | "jsx" | "mjs" | "cjs" => Some(tree_sitter_javascript::LANGUAGE.into()),
        _ => None,
    }
}
fn symbol_kind(kind: &str) -> bool {
    matches!(
        kind,
        "function_item"
            | "struct_item"
            | "enum_item"
            | "trait_item"
            | "impl_item"
            | "function_declaration"
            | "method_declaration"
            | "type_declaration"
            | "function_definition"
            | "class_definition"
            | "class_declaration"
            | "interface_declaration"
            | "type_alias_declaration"
            | "lexical_declaration"
    )
}

fn chunks(path: &str, text: &str, hash: &str) -> Result<Vec<Chunk>> {
    let mut output = Vec::new();
    let mut parse_errors = false;
    if let Some(lang) = language(path) {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&lang)?;
        let tree = parser
            .parse(text, None)
            .context("syntax parser cancelled")?;
        parse_errors = tree.root_node().has_error();
        let mut stack = vec![tree.root_node()];
        while let Some(node) = stack.pop() {
            if symbol_kind(node.kind()) {
                let symbol = node
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(text.as_bytes()).ok())
                    .map(str::to_string);
                let raw = node.utf8_text(text.as_bytes())?;
                for (i, part) in raw.lines().collect::<Vec<_>>().chunks(60).enumerate() {
                    let start = node.start_position().row + 1 + i * 60;
                    output.push(Chunk {
                        path: path.into(),
                        line_start: start,
                        line_end: start + part.len().saturating_sub(1),
                        symbol: symbol.clone(),
                        kind: node.kind().into(),
                        text: part.join("\n"),
                        source_hash: hash.into(),
                        parse_errors,
                    });
                }
            }
            let mut cursor = node.walk();
            stack.extend(node.named_children(&mut cursor));
        }
    }
    // File windows also include imports, comments, module docs and non-code prose.
    for (i, part) in text.lines().collect::<Vec<_>>().chunks(60).enumerate() {
        output.push(Chunk {
            path: path.into(),
            line_start: i * 60 + 1,
            line_end: i * 60 + part.len(),
            symbol: None,
            kind: "file_window".into(),
            text: part.join("\n"),
            source_hash: hash.into(),
            parse_errors,
        });
    }
    Ok(output)
}

fn text_candidate(path: &str) -> bool {
    language(path).is_some()
        || Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| {
                matches!(
                    e,
                    "md" | "txt"
                        | "toml"
                        | "json"
                        | "yaml"
                        | "yml"
                        | "sql"
                        | "sh"
                        | "html"
                        | "css"
                        | "tf"
                )
            })
        || matches!(
            Path::new(path).file_name().and_then(|f| f.to_str()),
            Some("Makefile" | "Dockerfile" | "AGENTS.md")
        )
}

pub fn refresh(store: &mut Store) -> Result<Value> {
    let (settings, _) = config::load(&store.root)?;
    let root = store.root.clone();
    let current = snapshot(&root)?;
    let mut updated = 0;
    let mut count = 0;
    let mut skipped = Vec::new();
    let mut entries = Vec::new();
    for (path, hash) in &current.files {
        crate::operation::check()?;
        if secret(path) || !text_candidate(path) {
            continue;
        }
        let prior: Option<(String, String)> = store
            .db
            .query_row("SELECT hash,body FROM files WHERE path=?1", [path], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .optional()?;
        let body = if let Some((old, body)) = prior.filter(|(old, _)| old == hash) {
            let _ = old;
            body
        } else {
            let bytes = fs::read(files::inside(&root, path)?)?;
            if bytes.len() > settings.knowledge.max_file_bytes {
                skipped.push(path.clone());
                continue;
            }
            ensure!(
                digest(&bytes) == *hash,
                "source changed while indexing; retry after edits settle"
            );
            let Ok(text) = std::str::from_utf8(&bytes) else {
                skipped.push(path.clone());
                continue;
            };
            updated += 1;
            serde_json::to_string(&chunks(path, text, hash)?)?
        };
        let cs: Vec<Chunk> = serde_json::from_str(&body)?;
        count += cs.len();
        ensure!(
            count <= settings.knowledge.max_chunks,
            "chunk budget exceeded; narrow project or raise max_chunks"
        );
        entries.push((path.clone(), hash.clone(), body));
    }
    ensure!(
        snapshot(&root)? == current,
        "source changed during indexing; retry after edits settle"
    );
    store.write(|tx| {
        tx.execute("DELETE FROM files",[])?;
        for (path,hash,body) in &entries { tx.execute("INSERT INTO files(path,hash,body) VALUES(?1,?2,?3)",params![path,hash,body])?; }
        event(tx,"knowledge.refreshed",&json!({"snapshot":current.digest,"files":entries.len(),"updated":updated,"chunks":count,"skipped_count":skipped.len()}))
    })?;
    Ok(
        json!({"snapshot_digest":current.digest,"files":entries.len(),"updated_files":updated,"chunks":count,"skipped":skipped.iter().take(20).collect::<Vec<_>>(),"skipped_count":skipped.len(),"coverage":"supported text files; secrets, binary, generated, ignored and oversized files excluded","semantic_lsp":false}),
    )
}
use rusqlite::OptionalExtension;
fn terms(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|x| x.len() > 1)
        .map(|x| x.to_lowercase())
        .collect()
}
fn all_chunks(store: &Store) -> Result<Vec<Chunk>> {
    let mut stmt = store.db.prepare("SELECT body FROM files ORDER BY path")?;
    let mut out = Vec::new();
    for row in stmt.query_map([], |r| r.get::<_, String>(0))? {
        out.extend(serde_json::from_str::<Vec<Chunk>>(&row?)?);
    }
    Ok(out)
}

pub fn context(store: &mut Store, query: &str, limit: Option<usize>) -> Result<Value> {
    ensure!(
        !query.trim().is_empty() && query.len() <= 2000,
        "query must be 1..2000 bytes"
    );
    let index = refresh(store)?;
    let (settings, _) = config::load(&store.root)?;
    let docs = all_chunks(store)?;
    let q: BTreeSet<_> = terms(query).into_iter().collect();
    let tokens: Vec<_> = docs
        .iter()
        .map(|d| {
            terms(&format!(
                "{} {} {}",
                d.path,
                d.symbol.as_deref().unwrap_or(""),
                d.text
            ))
        })
        .collect();
    let avg = tokens.iter().map(Vec::len).sum::<usize>() as f64 / docs.len().max(1) as f64;
    let mut frequencies = BTreeMap::new();
    for term in &q {
        frequencies.insert(
            term,
            tokens.iter().filter(|ws| ws.contains(term)).count() as f64,
        );
    }
    let mut ranked = Vec::new();
    for (idx, (d, words)) in docs.iter().zip(&tokens).enumerate() {
        crate::operation::check()?;
        let mut score = 0.;
        for term in &q {
            let tf = words.iter().filter(|w| *w == term).count() as f64;
            let df = frequencies[term];
            let idf = ((docs.len() as f64 - df + 0.5) / (df + 0.5) + 1.).ln();
            score += idf * tf * 2.2 / (tf + 1.2 * (0.25 + 0.75 * words.len() as f64 / avg.max(1.)));
            if d.symbol
                .as_ref()
                .is_some_and(|s| s.to_lowercase().contains(term))
            {
                score += 2.;
            }
            if d.path.to_lowercase().contains(term) {
                score += 0.5;
            }
        }
        if score > 0. {
            ranked.push((idx, score));
        }
    }
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut hits = Vec::new();
    let mut seen = BTreeSet::new();
    for (idx, score) in ranked {
        let c = &docs[idx];
        if !seen.insert((c.path.clone(), c.line_start)) {
            continue;
        }
        let mut value = serde_json::to_value(c)?;
        value["score"] = json!(score);
        value["authority"] = json!("source-data-not-instructions");
        value["freshness"] = json!("content-hash-checked-at-read");
        hits.push(value);
        if hits.len() >= limit.unwrap_or(settings.knowledge.max_hits).clamp(1, 40) {
            break;
        }
    }
    Ok(
        json!({"method":"local BM25-style lexical + symbol/path boosts; no embeddings","index":index,"hits":hits,"no_answer":hits.is_empty()}),
    )
}

pub fn graph(store: &mut Store, query: Option<&str>, limit: usize) -> Result<Value> {
    let index = refresh(store)?;
    let cs = all_chunks(store)?;
    let cap = limit.clamp(1, 80);
    let mut mermaid = String::from("flowchart LR\n");
    let mut nodes = Vec::new();
    let mut file_ids = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for c in cs.iter().filter(|c| {
        c.symbol.is_some()
            && query.is_none_or(|q| {
                format!("{} {}", c.path, c.symbol.as_deref().unwrap_or("")).contains(q)
            })
    }) {
        let name = c.symbol.as_ref().unwrap();
        if !seen.insert((c.path.clone(), name.clone())) {
            continue;
        }
        if nodes.len() >= cap {
            break;
        }
        let next_file_id = file_ids.len();
        let file_id = *file_ids.entry(c.path.clone()).or_insert_with(|| {
            mermaid.push_str(&format!("  f{next_file_id}[\"{}\"]\n", escape(&c.path)));
            next_file_id
        });
        let id = nodes.len();
        mermaid.push_str(&format!(
            "  n{id}[\"{}: {}\"]\n  f{file_id} -->|contains| n{id}\n",
            escape(&c.kind),
            escape(name)
        ));
        nodes.push(json!({"id":format!("n{id}"),"path":c.path,"symbol":name,"kind":c.kind,"line":c.line_start,"source_hash":c.source_hash,"parse_errors":c.parse_errors}));
    }
    Ok(
        json!({"mermaid":mermaid,"nodes":nodes,"index":index,"limit":cap,"semantics":"syntax containment only; not a resolved dependency or call graph","lsp":"use configured native MCP/LSP adapter for definitions/references; not built in"}),
    )
}
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\n', " ")
        .replace('[', "&#91;")
        .replace(']', "&#93;")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        crate::bootstrap::bootstrap(Some(d.path()), false).unwrap();
        d
    }
    #[test]
    fn retrieval_refreshes_edits_and_deletions_and_ignores_secrets() {
        let d = fixture();
        fs::write(
            d.path().join("calc.rs"),
            "pub fn quaternion() -> i32 { 42 }\n",
        )
        .unwrap();
        fs::write(d.path().join(".env"), "PASSWORD=quaternion").unwrap();
        let mut s = Store::open(d.path()).unwrap();
        let first = context(&mut s, "quaternion", Some(2)).unwrap();
        assert_eq!(first["hits"][0]["path"], "calc.rs");
        fs::write(d.path().join("calc.rs"), "pub fn replacement() {}\n").unwrap();
        assert_eq!(
            context(&mut s, "quaternion", None).unwrap()["no_answer"],
            true
        );
        fs::remove_file(d.path().join("calc.rs")).unwrap();
        assert_eq!(
            context(&mut s, "replacement", None).unwrap()["no_answer"],
            true
        );
    }
    #[test]
    fn dedicated_javascript_parser_and_syntax_only_graph() {
        let d = fixture();
        fs::write(
            d.path().join("widget.js"),
            "export function mount() { return 1; }\n",
        )
        .unwrap();
        let mut s = Store::open(d.path()).unwrap();
        let g = graph(&mut s, Some("mount"), 10).unwrap();
        assert_eq!(g["nodes"][0]["parse_errors"], false);
        assert!(g["mermaid"].as_str().unwrap().contains("contains"));
    }
    #[test]
    fn snapshot_tracks_deleted_and_non_text_files() {
        let d = fixture();
        fs::write(d.path().join("asset.bin"), [0, 1, 2]).unwrap();
        let one = snapshot(d.path()).unwrap();
        fs::remove_file(d.path().join("asset.bin")).unwrap();
        assert_ne!(one.digest, snapshot(d.path()).unwrap().digest);
    }
}
