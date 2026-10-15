//! Only explicitly journaled OnTrack handoffs are managed here. Beads remain independent.
//!
//! The advisory lock serializes OnTrack processes, not arbitrary external Git clients.
//! Every mutation rechecks Git and uses ff-only; no reset, force, or branch deletion.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, Output},
};

use serde_json::{Value, json};

use crate::herdr::{self, Executor, RepositoryIdentity, SystemExecutor, envelope};

#[derive(Debug, Clone)]
pub struct ManagedWorktree {
    pub id: String,
    pub bead_id: String,
    /// Herdr accepted the initial prompt and the active phase was persisted.
    pub handoff_accepted: bool,
    pub bead_title: Option<String>,
    pub bead_description: Option<String>,
    pub branch: String,
    pub target_branch: String,
    pub workspace_id: String,
    pub status: WorktreeStatus,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorktreeStatus {
    Unmerged,
    MergeRequested,
    NeedsResolution,
    Integrated,
    WorkspaceClosed,
    CheckoutRemoved,
    NeedsAttention,
}

#[derive(Debug, Clone, Copy)]
pub enum Action {
    RequestMerge,
    CloseWorkspace,
    RemoveCheckout,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CleanupPolicy {
    pub auto_merge: bool,
    pub auto_close_workspace: bool,
    pub auto_remove_checkout: bool,
}

fn error(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}

fn git_output(path: &Path, args: &[&str]) -> io::Result<Output> {
    Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
}

fn git(path: &Path, args: &[&str]) -> io::Result<String> {
    let output = git_output(path, args)?;
    if !output.status.success() {
        return Err(error(format!(
            "Git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    String::from_utf8(output.stdout)
        .map(|s| s.strip_suffix('\n').unwrap_or(&s).to_owned())
        .map_err(|_| error("Git output must be UTF-8"))
}

fn branch(path: &Path) -> io::Result<String> {
    git(path, &["symbolic-ref", "--quiet", "--short", "HEAD"])
}

fn tip(path: &Path, reference: &str) -> io::Result<String> {
    git(
        path,
        &["rev-parse", "--verify", &format!("{reference}^{{commit}}")],
    )
}

fn ancestor(path: &Path, source: &str, target: &str) -> io::Result<bool> {
    let output = git_output(path, &["merge-base", "--is-ancestor", source, target])?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(error(format!(
            "Cannot verify ancestry: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))),
    }
}

fn clean(path: &Path) -> io::Result<()> {
    if !git(path, &["status", "--porcelain=v1", "--untracked-files=all"])?.is_empty() {
        return Err(error(format!(
            "Dirty checkout {}; commit/resolve changes first",
            path.display()
        )));
    }
    // Some in-progress operations can have a temporarily clean index/worktree.
    for name in [
        "MERGE_HEAD",
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "rebase-merge",
        "rebase-apply",
        "sequencer",
    ] {
        let metadata = git(
            path,
            &["rev-parse", "--path-format=absolute", "--git-path", name],
        )?;
        if Path::new(&metadata).exists() {
            return Err(error(format!(
                "Unfinished Git operation {name} in {}",
                path.display()
            )));
        }
    }
    Ok(())
}

/// Remote symbolic HEADs must agree. Without one, only the verified primary
/// checkout's actual branch is evidence; init.defaultBranch is never consulted.
fn default_branch(repo: &RepositoryIdentity) -> io::Result<String> {
    let refs = git(
        &repo.main_root,
        &[
            "for-each-ref",
            "--format=%(refname) %(symref)",
            "refs/remotes/",
        ],
    )?;
    let mut defaults = std::collections::BTreeSet::new();
    // for-each-ref omits dangling symbolic HEADs: do not silently fall back
    // when a configured remote has a broken default ref.
    for remote in git(&repo.main_root, &["remote"])?.lines() {
        let reference = format!("refs/remotes/{remote}/HEAD");
        let output = git_output(&repo.main_root, &["symbolic-ref", "--quiet", &reference])?;
        if output.status.success() {
            let target =
                String::from_utf8(output.stdout).map_err(|_| error("Non UTF-8 remote HEAD"))?;
            let target = target.trim_end_matches('\n');
            let prefix = format!("refs/remotes/{remote}/");
            let candidate = target
                .strip_prefix(&prefix)
                .ok_or_else(|| error("Remote HEAD points outside its remote"))?;
            tip(&repo.main_root, target)?;
            defaults.insert(candidate.to_owned());
        } else if output.status.code() != Some(1) {
            return Err(error(format!(
                "Cannot inspect remote symbolic HEAD {reference}"
            )));
        }
    }
    for line in refs.lines() {
        let Some((name, target)) = line.split_once(' ') else {
            continue;
        };
        if name.ends_with("/HEAD") && !target.is_empty() {
            let prefix = name
                .strip_suffix("HEAD")
                .ok_or_else(|| error("Invalid remote HEAD"))?;
            let candidate = target
                .strip_prefix(prefix)
                .ok_or_else(|| error("Remote HEAD points outside its remote"))?;
            tip(&repo.main_root, target)?;
            defaults.insert(candidate.to_owned());
        }
    }
    if defaults.len() > 1 {
        return Err(error(
            "Remote symbolic HEADs disagree; default branch is ambiguous",
        ));
    }
    let target = match defaults.into_iter().next() {
        Some(target) => target,
        None => branch(&repo.main_root).map_err(|_| error("No remote symbolic HEAD and primary checkout is detached; cannot determine default branch"))?,
    };
    if target.starts_with("ontrack/") || target.is_empty() {
        return Err(error(
            "Primary checkout is a task branch; cannot establish a safe default branch",
        ));
    }
    tip(&repo.main_root, &format!("refs/heads/{target}"))?;
    if branch(&repo.main_root)? != target {
        return Err(error(format!(
            "Verified primary checkout must be on default branch {target}"
        )));
    }
    Ok(target)
}

/// Registry entries deliberately retain raw responses, including malformed or
/// mismatched create results, so partial launches can be recovered by a human.
struct Registry {
    repo: RepositoryIdentity,
    directory: PathBuf,
    _lock: File,
    entries: Vec<Value>,
}

impl Registry {
    fn open(cwd: &Path) -> io::Result<Self> {
        let root = git(cwd, &["rev-parse", "--show-toplevel"])?;
        let repo = SystemExecutor.git_identity(Path::new(&root))?;
        let directory = repo.common_dir.join("ontrack-worktrees");
        match fs::create_dir(&directory) {
            Ok(()) => {
                File::open(&repo.common_dir)?.sync_all()?;
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
        if !fs::symlink_metadata(&directory)?.file_type().is_dir() {
            return Err(error("OnTrack registry directory must not be a symlink"));
        }
        let lock_path = directory.join("registry.lock");
        reject_symlink(&lock_path)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)?;
        // std::fs::File::try_lock requires Rust 1.89; keep the project's 1.88 MSRV.
        fs2::FileExt::try_lock_exclusive(&lock)
            .map_err(|e| error(format!("OnTrack worktree registry is busy: {e}")))?;
        let path = directory.join("registry.json");
        reject_symlink(&path)?;
        let entries = match fs::read(&path) {
            Ok(bytes) => {
                let value: Value = serde_json::from_slice(&bytes).map_err(|e| {
                    error(format!("Invalid OnTrack registry (not overwritten): {e}"))
                })?;
                if value["version"] != 1 || value["common_dir"].as_str() != repo.common_dir.to_str()
                {
                    return Err(error(
                        "Unsupported/mismatched OnTrack registry; refusing changes",
                    ));
                }
                let entries = value["entries"]
                    .as_array()
                    .ok_or_else(|| error("Invalid registry entries"))?
                    .clone();
                let mut ids = std::collections::HashSet::new();
                for entry in &entries {
                    for key in [
                        "id",
                        "bead_id",
                        "branch",
                        "target_branch",
                        "common_dir",
                        "main_root",
                        "phase",
                        "base",
                        "agent",
                        "source_workspace",
                    ] {
                        field(entry, key)?;
                    }
                    if !ids.insert(field(entry, "id")?.to_owned())
                        || entry["common_dir"].as_str() != repo.common_dir.to_str()
                        || entry["main_root"].as_str() != repo.main_root.to_str()
                    {
                        return Err(error("Invalid/duplicate registry identity"));
                    }
                }
                entries
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(e),
        };
        Ok(Self {
            repo,
            directory,
            _lock: lock,
            entries,
        })
    }

    fn save(&self) -> io::Result<()> {
        let path = self.directory.join("registry.json");
        reject_symlink(&path)?;
        // A stable temporary name is safe under the separate, never-renamed lock.
        let temporary = self.directory.join("registry.json.tmp");
        reject_symlink(&temporary)?;
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&temporary)?;
        serde_json::to_writer_pretty(
            &mut file,
            &json!({"version":1, "common_dir":self.repo.common_dir, "entries":self.entries}),
        )?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        fs::rename(temporary, path)?;
        File::open(&self.directory)?.sync_all()
    }

    fn index(&self, id: &str) -> io::Result<usize> {
        self.entries
            .iter()
            .position(|e| e["id"] == id)
            .ok_or_else(|| {
                error("Unknown managed handoff; existing worktrees are never adopted automatically")
            })
    }
}

fn reject_symlink(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => Err(error(format!(
            "Refusing registry symlink {}",
            path.display()
        ))),
        Ok(_) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

fn field<'a>(entry: &'a Value, key: &str) -> io::Result<&'a str> {
    entry[key]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            error(format!(
                "Managed handoff has missing {key}; inspect registry before recovery"
            ))
        })
}

pub(crate) struct LaunchJournal {
    registry: Registry,
    index: usize,
}

impl LaunchJournal {
    pub(crate) fn begin(
        checkout: &Path,
        bead: &Value,
        branch: &str,
        agent: &str,
        name: &str,
        workspace: &str,
    ) -> io::Result<Self> {
        let bead_id = field(bead, "id")?;
        let mut registry = Registry::open(checkout)?;
        let target = default_branch(&registry.repo)?;
        clean(&registry.repo.main_root)?;
        if registry
            .entries
            .iter()
            .any(|e| e["id"] == name || e["branch"] == branch)
        {
            return Err(error(
                "Launch intent already recorded; inspect it instead of retrying creation",
            ));
        }
        // Also catch branches without an open Herdr workspace.
        if git_output(
            &registry.repo.main_root,
            &["show-ref", "--verify", &format!("refs/heads/{branch}")],
        )?
        .status
        .success()
        {
            return Err(error("Launch branch already exists; refusing reuse"));
        }
        let base = tip(&registry.repo.main_root, "HEAD")?;
        let index = registry.entries.len();
        registry.entries.push(json!({
            "id":name, "bead_id":bead_id,
            "bead_title":bead["title"].as_str(),
            "bead_description":bead["description"].as_str(),
            "branch":branch, "target_branch":target,
            "common_dir":registry.repo.common_dir, "main_root":registry.repo.main_root,
            "source_workspace":workspace, "agent":agent, "base":base,
            "phase":"create_possibly_sent", "responses":{},
            "workspace_closed":false, "checkout_removed":false,
            "detail":"Launch intent persisted before Herdr creation. If interrupted, inspect the raw response and Herdr/Git before recovery; never blindly relaunch."
        }));
        registry.save()?;
        Ok(Self { registry, index })
    }

    pub(crate) fn capture(&mut self, kind: &str, bytes: &[u8]) -> io::Result<()> {
        let entry = &mut self.registry.entries[self.index];
        entry["responses"][kind] = json!(String::from_utf8_lossy(bytes));
        if let Ok(result) = envelope(bytes)
            && result["agent"]["agent_session"].is_object()
        {
            entry["agent_session"] = result["agent"]["agent_session"].clone();
            if kind == "ready" {
                entry["initial_lifecycle_seq"] = result["agent"]["state_change_seq"].clone();
            }
        }
        self.registry.save()
    }

    pub(crate) fn created(&mut self, result: &Value) -> io::Result<()> {
        // Persist received handles BEFORE filesystem identity checks or agent start.
        let entry = &mut self.registry.entries[self.index];
        entry["workspace_id"] = result["workspace"]["workspace_id"].clone();
        entry["pane_id"] = result["root_pane"]["pane_id"].clone();
        entry["path"] = result["worktree"]["path"].clone();
        entry["phase"] = json!("created");
        self.registry.save()?;
        let entry = &self.registry.entries[self.index];
        let source = Path::new(field(entry, "path")?).canonicalize()?;
        if source == self.registry.repo.main_root || source.to_str() != entry["path"].as_str() {
            return Err(error(
                "Created checkout path is not a distinct canonical path",
            ));
        }
        let identity = SystemExecutor.git_identity(&source)?;
        if identity != self.registry.repo
            || branch(&source)? != field(entry, "branch")?
            || tip(&source, "HEAD")? != field(entry, "base")?
        {
            return Err(error(
                "Created checkout identity, branch, or launch base does not match intent; no agent started",
            ));
        }
        clean(&source)?;
        self.phase("created_verified")
    }

    pub(crate) fn phase(&mut self, phase: &str) -> io::Result<()> {
        let entry = &mut self.registry.entries[self.index];
        entry["phase"] = json!(phase);
        if phase == "active" {
            entry["detail"] = Value::Null;
        }
        self.registry.save()
    }

    pub(crate) fn failed(&mut self, message: &str) -> io::Result<()> {
        self.registry.entries[self.index]["detail"] = json!(message);
        self.registry.save()
    }
}

struct Verified {
    source: PathBuf,
    source_tip: String,
    target_tip: String,
}

const NO_TASK_COMMITS: &str = "No task commits beyond the recorded launch base. No-op completion is not supported: inspect the task and keep the checkout; closing a Bead alone never authorizes integration or cleanup.";
const CLOSED_REMOVAL_UNAVAILABLE: &str = "Checkout removal unavailable after workspace closure: installed Herdr help documents only --workspace ID; safe removal using a closed ID is unverified. Do not reuse the closed ID. Inspect the recorded checkout and current branch integration, then use Git's non-force worktree removal for that exact path, retaining the branch; OnTrack will not automatically adopt a replacement workspace.";

fn task_commits(repo: &RepositoryIdentity, entry: &Value, source_tip: &str) -> io::Result<()> {
    let base = field(entry, "base")?;
    if source_tip == base {
        return Err(error(NO_TASK_COMMITS));
    }
    if !ancestor(&repo.main_root, base, source_tip)? {
        return Err(error(
            "Source no longer contains its recorded launch base; inspect rewritten history before integration or cleanup",
        ));
    }
    Ok(())
}

fn verify(repo: &RepositoryIdentity, entry: &Value) -> io::Result<Verified> {
    let actual = SystemExecutor.git_identity(&repo.main_root)?;
    if actual != *repo
        || entry["common_dir"].as_str() != repo.common_dir.to_str()
        || entry["main_root"].as_str() != repo.main_root.to_str()
    {
        return Err(error("Repository identity changed"));
    }
    let target = default_branch(repo)?;
    if target != field(entry, "target_branch")? {
        return Err(error(
            "Repository default branch changed; explicit registry recovery required",
        ));
    }
    clean(&repo.main_root)?;
    let source = Path::new(field(entry, "path")?).canonicalize()?;
    if source.to_str() != entry["path"].as_str()
        || source == repo.main_root
        || SystemExecutor.git_identity(&source)? != *repo
    {
        return Err(error(
            "Source checkout canonical/repository identity changed",
        ));
    }
    if branch(&source)? != field(entry, "branch")? {
        return Err(error("Source checkout branch changed"));
    }
    clean(&source)?;
    Ok(Verified {
        source_tip: tip(&source, "HEAD")?,
        target_tip: tip(&repo.main_root, "HEAD")?,
        source,
    })
}

fn view(entry: &Value, status: WorktreeStatus, detail: Option<String>) -> ManagedWorktree {
    let string = |key: &str| entry[key].as_str().unwrap_or_default().to_owned();
    ManagedWorktree {
        id: string("id"),
        bead_id: string("bead_id"),
        handoff_accepted: entry["phase"] == "active",
        bead_title: entry["bead_title"].as_str().map(str::to_owned),
        bead_description: entry["bead_description"].as_str().map(str::to_owned),
        branch: string("branch"),
        target_branch: string("target_branch"),
        workspace_id: string("workspace_id"),
        status,
        detail,
    }
}

fn inspect(repo: &RepositoryIdentity, entry: &Value) -> ManagedWorktree {
    use WorktreeStatus::*;
    if entry["restart_pending"] == true {
        return view(entry, NeedsAttention, Some(entry["detail"].as_str().unwrap_or(
            "Restart removal may have succeeded; inspect Git and Herdr before recovery."
        ).to_owned()));
    }
    if entry["restarted"] == true {
        if entry["restart_launch_pending"] == true || entry["restart_launch_error"].is_string() {
            return view(entry, NeedsAttention, Some(entry["restart_launch_error"].as_str().unwrap_or(
                "Old checkout removed; fresh launch may not have completed. Inspect tracked handoffs before recovery."
            ).to_owned()));
        }
        if entry["path"]
            .as_str()
            .is_some_and(|path| Path::new(path).exists())
        {
            return view(
                entry,
                NeedsAttention,
                Some("Restarted checkout path exists again; inspect identity".into()),
            );
        }
        return view(entry, CheckoutRemoved, None);
    }
    if entry["phase"] != "active" {
        return view(
            entry,
            NeedsAttention,
            Some(
                entry["detail"]
                    .as_str()
                    .unwrap_or(
                        "Incomplete launch; inspect recorded response/handles. No automatic retry.",
                    )
                    .to_owned(),
            ),
        );
    }
    if entry["checkout_removed"] == true {
        let check = (|| {
            let target = default_branch(repo)?;
            if target != field(entry, "target_branch")? {
                return Err(error("Default branch changed after removal"));
            }
            clean(&repo.main_root)?;
            let source = tip(
                &repo.main_root,
                &format!("refs/heads/{}", field(entry, "branch")?),
            )?;
            if Path::new(field(entry, "path")?).exists() {
                return Err(error(
                    "Removed checkout path exists again; identity requires inspection",
                ));
            }
            if !ancestor(&repo.main_root, &source, &tip(&repo.main_root, "HEAD")?)? {
                return Err(error(
                    "Preserved source branch advanced after checkout removal; new work is outstanding",
                ));
            }
            Ok(())
        })();
        return match check {
            Ok(()) => view(
                entry,
                CheckoutRemoved,
                entry["detail"].as_str().map(str::to_owned),
            ),
            Err(e) => view(entry, NeedsAttention, Some(e.to_string())),
        };
    }
    if entry["cleanup_pending"].is_string() {
        return view(entry, NeedsAttention, Some(entry["detail"].as_str().unwrap_or("Cleanup possibly sent; reconciling read-only evidence, never blindly retrying.").to_owned()));
    }
    let verified = match verify(repo, entry) {
        Ok(verified) => verified,
        Err(e) => {
            let conflicts = entry["merge_request"].is_object()
                && entry["path"].as_str().is_some_and(|path| {
                    git(Path::new(path), &["ls-files", "--unmerged"])
                        .is_ok_and(|files| !files.is_empty())
                });
            return view(
                entry,
                if conflicts {
                    NeedsResolution
                } else {
                    NeedsAttention
                },
                Some(e.to_string()),
            );
        }
    };
    if let Err(e) = task_commits(repo, entry, &verified.source_tip) {
        return view(
            entry,
            if verified.source_tip == entry["base"] {
                Unmerged
            } else {
                NeedsAttention
            },
            Some(e.to_string()),
        );
    }
    match ancestor(&repo.main_root, &verified.source_tip, &verified.target_tip) {
        Ok(true) => {
            return view(
                entry,
                if entry["workspace_closed"] == true {
                    WorkspaceClosed
                } else {
                    Integrated
                },
                entry["detail"].as_str().map(str::to_owned).or_else(|| {
                    (entry["workspace_closed"] == true)
                        .then(|| CLOSED_REMOVAL_UNAVAILABLE.to_owned())
                }),
            );
        }
        Err(e) => return view(entry, NeedsAttention, Some(e.to_string())),
        Ok(false) => {}
    }
    if entry["workspace_closed"] == true {
        return view(entry, NeedsAttention, Some("Source advanced after workspace closure; integration is outstanding, reopen the workspace explicitly".into()));
    }
    if entry["merge_request"].is_object() {
        if entry["merge_request"]["target_tip"] != verified.target_tip {
            return view(entry, NeedsResolution, Some("Target advanced since the request; explicitly request another agent resolution. Never automatically resend.".into()));
        }
        return view(entry, MergeRequested, Some(entry["detail"].as_str().unwrap_or("Merge prompt possibly sent; waiting for verified current Git ancestry. Native idle/sequence changes are not proof that this particular prompt completed. Never blindly resend.").to_owned()));
    }
    view(entry, Unmerged, entry["detail"].as_str().map(str::to_owned))
}

/// A confirmed restart abandons the old checkout and launches the live Bead
/// from the current main HEAD. The old branch is deliberately retained.
pub fn restart(id: &str) -> io::Result<String> {
    restart_with(&std::env::current_dir()?, id, &mut SystemExecutor)
}

fn restart_with(cwd: &Path, id: &str, executor: &mut impl Executor) -> io::Result<String> {
    let mut registry = Registry::open(cwd)?;
    let index = registry.index(id)?;
    let entry = &registry.entries[index];
    if entry["phase"] != "active"
        || entry["checkout_removed"] == true
        || entry["restart_pending"] == true
        || entry["restarted"] == true
        || entry["cleanup_pending"].is_string()
    {
        return Err(error(
            "This handoff cannot be restarted or may already have been removed",
        ));
    }
    let main = registry.repo.main_root.clone();
    if SystemExecutor.git_identity(&main)? != registry.repo
        || default_branch(&registry.repo)? != field(entry, "target_branch")?
        || branch(&main)? != field(entry, "target_branch")?
    {
        return Err(error(
            "Main checkout identity or branch changed; restart cancelled",
        ));
    }
    clean(&main)?;
    let path = Path::new(field(entry, "path")?).canonicalize()?;
    if path == main
        || path.to_str() != entry["path"].as_str()
        || SystemExecutor.git_identity(&path)? != registry.repo
        || branch(&path)? != field(entry, "branch")?
    {
        return Err(error(
            "Recorded checkout identity or branch changed; restart cancelled",
        ));
    }
    verify_open_workspace(&registry.repo, entry, executor)?;
    let workspace = field(entry, "workspace_id")?.to_owned();
    let recorded_path = field(entry, "path")?.to_owned();
    let source_workspace = field(entry, "source_workspace")?.to_owned();
    let agent = field(entry, "agent")?.to_owned();
    let bead_id = field(entry, "bead_id")?.to_owned();
    let bead = executor.show_bead(&bead_id)?;
    let title = if bead["id"] == bead_id {
        field(&bead, "title")?.to_owned()
    } else {
        return Err(error("Live Bead identity changed; restart cancelled"));
    };
    herdr::validate_restart_source(executor, &main, &source_workspace, &agent)?;
    // Recheck ownership immediately before the destructive command.
    verify_open_workspace(&registry.repo, entry, executor)?;
    if Path::new(field(entry, "path")?).canonicalize()? != path
        || branch(&path)? != field(entry, "branch")?
    {
        return Err(error("Checkout changed during restart validation"));
    }
    registry.entries[index]["restart_pending"] = json!(true);
    registry.entries[index]["detail"] = json!(
        "Restart removal may have been sent. Inspect Herdr and Git before recovery; never retry blindly."
    );
    registry.save()?;
    let bytes = executor.run(&["worktree", "remove", "--workspace", &workspace, "--force"])?;
    registry.entries[index]["restart_remove_response"] = json!(String::from_utf8_lossy(&bytes));
    registry.save()?;
    envelope(&bytes)?;
    let inventory = worktree_inventory(&registry.repo, &registry.entries[index], executor)?;
    let rows = inventory["worktrees"]
        .as_array()
        .ok_or_else(|| error("Missing worktree rows"))?;
    if path.exists()
        || rows
            .iter()
            .any(|row| row["open_workspace_id"] == workspace || row["path"] == recorded_path)
        || git(&main, &["worktree", "list", "--porcelain", "-z"])?
            .split('\0')
            .any(|line| line.strip_prefix("worktree ") == path.to_str())
    {
        return Err(error(
            "Restart removal is unconfirmed; inspect Herdr and Git",
        ));
    }
    registry.entries[index]["restart_pending"] = Value::Null;
    registry.entries[index]["restarted"] = json!(true);
    registry.entries[index]["checkout_removed"] = json!(true);
    registry.entries[index]["workspace_closed"] = json!(true);
    registry.entries[index]["restart_launch_pending"] = json!(true);
    registry.entries[index]["detail"] = Value::Null;
    registry.save()?;
    drop(registry);
    match herdr::relaunch_from(executor, &main, &source_workspace, &agent, &bead_id, &title) {
        Ok(outcome) => {
            let mut registry = Registry::open(&main)?;
            let index = registry.index(id)?;
            registry.entries[index]["restart_launch_pending"] = Value::Null;
            registry.save()?;
            Ok(outcome.message)
        }
        Err(cause) => {
            let mut registry = Registry::open(&main)?;
            let index = registry.index(id)?;
            registry.entries[index]["restart_launch_pending"] = Value::Null;
            registry.entries[index]["restart_launch_error"] = json!(format!(
                "Old checkout removed; fresh launch failed: {cause}. Inspect any new handoff before recovery."
            ));
            registry.save()?;
            Err(cause)
        }
    }
}

/// A conflict can be a transient part of the requested merge. Only a live,
/// matching native agent observation changes its display state; Git safety
/// decisions continue to use `inspect` and `verify`.
fn inspect_for_display(
    repo: &RepositoryIdentity,
    entry: &Value,
    executor: &mut impl Executor,
) -> ManagedWorktree {
    let mut item = inspect(repo, entry);
    if item.status != WorktreeStatus::NeedsResolution || !entry["merge_request"].is_object() {
        return item;
    }
    let working = (|| -> io::Result<bool> {
        let result = envelope(&executor.run(&["agent", "get", field(entry, "pane_id")?])?)?;
        let agent = &result["agent"];
        if result["type"] != "agent_info"
            || agent["pane_id"] != entry["pane_id"]
            || agent["workspace_id"] != entry["workspace_id"]
            || agent["agent"] != entry["agent"]
        {
            return Ok(false);
        }
        let session = if entry["merge_request"]["agent_session"].is_object() {
            &entry["merge_request"]["agent_session"]
        } else {
            &entry["agent_session"]
        };
        if session.is_object() && agent["agent_session"] != *session {
            return Ok(false);
        }
        Ok(agent["agent_status"] == "working")
    })()
    .unwrap_or(false);
    if working {
        item.status = WorktreeStatus::MergeRequested;
        item.detail = Some(format!(
            "{} The recorded agent is still working; recheck Git when it settles.",
            item.detail
                .as_deref()
                .unwrap_or("Merge request is pending.")
        ));
    }
    item
}

/// Read-only with respect to Git/Herdr; only this repository's durable handoffs.
pub fn list() -> io::Result<Vec<ManagedWorktree>> {
    list_with(&std::env::current_dir()?, &mut SystemExecutor)
}

fn list_with(cwd: &Path, executor: &mut impl Executor) -> io::Result<Vec<ManagedWorktree>> {
    let registry = Registry::open(cwd)?;
    Ok(registry
        .entries
        .iter()
        .map(|entry| inspect_for_display(&registry.repo, entry, executor))
        .collect())
}

/// Native lifecycle evidence is mandatory for automatic operations. It is not a
/// completion claim: Git ancestry/cleanliness and a fresh closed Bead are separate gates.
fn settled(
    executor: &mut impl Executor,
    entry: &Value,
    require_session: bool,
) -> io::Result<Value> {
    let result = envelope(&executor.run(&["agent", "get", field(entry, "pane_id")?])?)?;
    let agent = &result["agent"];
    if result["type"] != "agent_info"
        || agent["pane_id"] != entry["pane_id"]
        || agent["workspace_id"] != entry["workspace_id"]
        || agent["agent"] != entry["agent"]
    {
        return Err(error(
            "Herdr agent identity does not match recorded pane/workspace/kind",
        ));
    }
    if !matches!(agent["agent_status"].as_str(), Some("idle" | "done")) {
        return Err(error(
            "Agent is not natively idle/done; automation deferred (unknown/working/blocked is not completion)",
        ));
    }
    if entry["agent_session"].is_object() {
        if (require_session || agent["agent_session"].is_object())
            && agent["agent_session"] != entry["agent_session"]
        {
            return Err(error("Agent session changed; automation/merge deferred"));
        }
    } else if require_session {
        return Err(error(
            "Automatic action deferred: launch response did not record a native Herdr agent_session; idle alone cannot prove the original agent completed",
        ));
    }
    if require_session {
        let before = entry["initial_lifecycle_seq"].as_u64().ok_or_else(|| {
            error("Automatic action deferred: no recorded native pre-prompt lifecycle sequence")
        })?;
        if agent["state_change_seq"]
            .as_u64()
            .is_none_or(|now| now <= before)
        {
            return Err(error(
                "Automatic action deferred: no observed lifecycle activity after the initial prompt; dispatch/idle alone is not completion",
            ));
        }
    }
    for key in ["cwd", "foreground_cwd"] {
        // Herdr 0.9.1 permits either path to be omitted or null. Explicit
        // manual authorization rests on the separately verified Git checkout;
        // validate every path actually reported, without inventing a required
        // optional field. Automation deliberately requires both observations.
        let reported = match agent.get(key) {
            None | Some(Value::Null) if !require_session => continue,
            None | Some(Value::Null) => {
                return Err(error(format!(
                    "Automatic action deferred: native agent {key} is unavailable"
                )));
            }
            Some(Value::String(path)) if !path.is_empty() => path,
            Some(_) => {
                return Err(error(format!(
                    "Agent {key} is invalid: expected a nonempty absolute path"
                )));
            }
        };
        let path = Path::new(reported);
        if !path.is_absolute() {
            return Err(error(format!(
                "Agent {key} is invalid: expected an absolute path"
            )));
        }
        let path = path
            .canonicalize()
            .map_err(|e| error(format!("Agent {key} cannot be resolved: {e}")))?;
        if path.to_str() != entry["path"].as_str() {
            return Err(error(format!(
                "Agent {key} does not match recorded checkout"
            )));
        }
    }
    Ok(agent.clone())
}

fn request_merge(
    registry: &mut Registry,
    index: usize,
    executor: &mut impl Executor,
    automatic: bool,
) -> io::Result<()> {
    let entry = &registry.entries[index];
    if entry["phase"] != "active"
        || entry["workspace_closed"] == true
        || entry["checkout_removed"] == true
    {
        return Err(error("Cannot prompt an incomplete/closed handoff"));
    }
    let verified = verify(&registry.repo, entry)?;
    task_commits(&registry.repo, entry, &verified.source_tip)?;
    if ancestor(
        &registry.repo.main_root,
        &verified.source_tip,
        &verified.target_tip,
    )? {
        return Ok(());
    }
    if entry["merge_request"]["target_tip"] == verified.target_tip {
        return Err(error(
            "Merge request already possibly sent for this target tip; inspect the agent, do not blindly resend",
        ));
    }
    let ready = settled(executor, entry, automatic)?;
    let fresh = verify(&registry.repo, entry)?;
    if fresh.source_tip != verified.source_tip || fresh.target_tip != verified.target_tip {
        return Err(error(
            "Git tips changed before merge dispatch; inspect again before requesting",
        ));
    }
    let pane = field(entry, "pane_id")?.to_owned();
    let prompt = format!(
        "OnTrack integration request. Work ONLY in the source checkout {} on branch {}. Requested source commit: {}. Verified default branch: {} (refs/heads/{}), target commit: {}. Read project instructions. Merge the latest LOCAL target branch INTO THIS SOURCE BRANCH, resolve conflicts here and commit the resolution; run relevant checks and report blockers. Do not change/check out/merge in the primary/default checkout {}, do not reset, force, delete branches/worktrees, or close workspaces. Do not change Beads status for this integration request. Leave the source clean and containing both requested commits. OnTrack independently verifies ancestry and performs ff-only integration. If the target advances again, stop and report rather than altering the default checkout.",
        verified.source.display(),
        field(entry, "branch")?,
        verified.source_tip,
        field(entry, "target_branch")?,
        field(entry, "target_branch")?,
        verified.target_tip,
        registry.repo.main_root.display()
    );
    registry.entries[index]["merge_request"] = json!({"source_tip":verified.source_tip, "target_tip":verified.target_tip, "possibly_sent":true, "provenance":if automatic {"automatic"} else {"manual"}, "lifecycle_seq":ready["state_change_seq"], "agent_session":ready["agent_session"]});
    registry.entries[index]["detail"] =
        json!("Merge prompt possibly sent; inspect agent before recovery, never blindly resend.");
    registry.save()?;
    let bytes = executor.run(&["agent", "prompt", &pane, &prompt])?;
    registry.entries[index]["merge_response"] = json!(String::from_utf8_lossy(&bytes));
    registry.save()?;
    let result = envelope(&bytes)?;
    if result["type"] != "agent_prompted"
        || result["agent"]["pane_id"] != registry.entries[index]["pane_id"]
        || result["agent"]["workspace_id"] != registry.entries[index]["workspace_id"]
    {
        return Err(error(
            "Merge prompt response identity mismatch; request may have been sent, not retried",
        ));
    }
    registry.entries[index]["detail"] = Value::Null;
    registry.save()
}

fn finalize(registry: &mut Registry, index: usize, executor: &mut impl Executor) -> io::Result<()> {
    let entry = &registry.entries[index];
    let verified = verify(&registry.repo, entry)?;
    task_commits(&registry.repo, entry, &verified.source_tip)?;
    if ancestor(
        &registry.repo.main_root,
        &verified.source_tip,
        &verified.target_tip,
    )? {
        return Ok(());
    }
    let request = &entry["merge_request"];
    if !request.is_object() {
        return Ok(());
    }
    if request["target_tip"] != verified.target_tip {
        return Err(error(
            "Target advanced; another explicit resolution request is required",
        ));
    }
    if !ancestor(
        &registry.repo.main_root,
        field(request, "source_tip")?,
        &verified.source_tip,
    )? || !ancestor(
        &registry.repo.main_root,
        &verified.target_tip,
        &verified.source_tip,
    )? {
        return Err(error(
            "Source does not yet contain requested source and current target commits; resolve and commit on source",
        ));
    }
    // Manual confirmation authorizes ff-only from verified CURRENT Git history;
    // optional native session/sequence metadata must not disable that workflow.
    // Unknown/legacy provenance remains conservative, like automatic requests.
    let automatic = request["provenance"] != "manual";
    let completed = settled(executor, entry, automatic)?;
    if request["agent_session"].is_object()
        && completed["agent_session"].is_object()
        && completed["agent_session"] != request["agent_session"]
    {
        return Err(error(
            "Merge request agent session changed; integration deferred",
        ));
    }
    if automatic {
        let before = request["lifecycle_seq"].as_u64().ok_or_else(|| {
            error("Automatic integration deferred: no recorded native merge-request lifecycle sequence")
        })?;
        if completed["state_change_seq"]
            .as_u64()
            .is_none_or(|now| now <= before)
        {
            return Err(error(
                "Automatic integration deferred: no native lifecycle change after merge dispatch. Idle/sequence changes are safety gates, not proof that this particular prompt completed",
            ));
        }
    }
    // Recheck immediately after external inspection. ff-only also refuses a
    // diverging concurrent target update instead of ever overwriting it.
    let fresh = verify(&registry.repo, entry)?;
    if fresh.source_tip != verified.source_tip || fresh.target_tip != verified.target_tip {
        return Err(error(
            "Git tips changed during finalization; retry inspection, not the prompt",
        ));
    }
    registry.entries[index]["detail"] =
        json!("ff-only integration possibly in progress; ancestry will be reverified on restart");
    registry.save()?;
    git(
        &registry.repo.main_root,
        &["merge", "--ff-only", "--no-edit", &verified.source_tip],
    )?;
    let after = verify(&registry.repo, &registry.entries[index])?;
    if !ancestor(
        &registry.repo.main_root,
        &after.source_tip,
        &after.target_tip,
    )? {
        return Err(error(
            "Source advanced during integration; current source is still outstanding",
        ));
    }
    registry.entries[index]["detail"] = Value::Null;
    registry.save()
}

fn cleanup(
    registry: &mut Registry,
    index: usize,
    action: Action,
    executor: &mut impl Executor,
) -> io::Result<()> {
    let entry = &registry.entries[index];
    if entry["phase"] != "active" {
        return Err(error("Incomplete launch cannot be cleaned automatically"));
    }
    if entry["checkout_removed"] == true {
        return Ok(());
    }
    if matches!(action, Action::CloseWorkspace) && entry["workspace_closed"] == true {
        return Ok(());
    }
    if matches!(action, Action::RemoveCheckout) && entry["workspace_closed"] == true {
        return Err(error(CLOSED_REMOVAL_UNAVAILABLE));
    }
    if entry["cleanup_pending"].is_string() {
        return Err(error(
            "Previous cleanup may have succeeded; inspect Herdr and recorded response before explicit recovery. Not automatically retried.",
        ));
    }
    let verified = verify(&registry.repo, entry)?;
    task_commits(&registry.repo, entry, &verified.source_tip)?;
    if !ancestor(
        &registry.repo.main_root,
        &verified.source_tip,
        &verified.target_tip,
    )? {
        return Err(error("Current source is not integrated; cleanup refused"));
    }
    verify_open_workspace(&registry.repo, entry, executor)?;
    // External inspection is not a lock on Git: revalidate immediately before
    // persisting/dispatching the mutation, including CURRENT source ancestry.
    let fresh = verify(&registry.repo, entry)?;
    if fresh.source_tip != verified.source_tip || fresh.target_tip != verified.target_tip {
        return Err(error(
            "Git tips changed during cleanup ownership validation; nothing closed or removed",
        ));
    }
    let workspace = field(entry, "workspace_id")?.to_owned();
    let (args, operation) = match action {
        Action::CloseWorkspace => (vec!["workspace", "close", workspace.as_str()], "close"),
        Action::RemoveCheckout => (
            vec!["worktree", "remove", "--workspace", workspace.as_str()],
            "remove",
        ),
        Action::RequestMerge => return Err(error("Not a cleanup action")),
    };
    registry.entries[index]["cleanup_pending"] = json!(operation);
    registry.entries[index]["cleanup_source_tip"] = json!(verified.source_tip);
    registry.save()?;
    let bytes = executor.run(&args)?;
    registry.entries[index]["cleanup_response"] = json!(String::from_utf8_lossy(&bytes));
    registry.save()?;
    // Even a successful transport is not proof: independently verify topology.
    envelope(&bytes)?;
    confirm_cleanup(registry, index, executor)
}

fn worktree_inventory(
    repo: &RepositoryIdentity,
    entry: &Value,
    executor: &mut impl Executor,
) -> io::Result<Value> {
    let result = envelope(
        &executor.run(&[
            "worktree",
            "list",
            "--cwd",
            repo.main_root
                .to_str()
                .ok_or_else(|| error("Non UTF-8 main checkout"))?,
        ])?,
    )?;
    if result["type"] != "worktree_list" {
        return Err(error("Unexpected Herdr worktree listing; cleanup refused"));
    }
    let reported_root = Path::new(field(&result["source"], "source_checkout_path")?);
    if !reported_root.is_absolute()
        || reported_root.canonicalize()? != repo.main_root
        || result["source"]["source_workspace_id"] != entry["source_workspace"]
    {
        return Err(error(
            "Herdr source checkout/workspace relationship changed; cleanup refused",
        ));
    }
    let rows = result["worktrees"]
        .as_array()
        .ok_or_else(|| error("Missing worktree verification rows"))?;
    let main_rows: Vec<_> = rows
        .iter()
        .filter(|row| row["open_workspace_id"] == entry["source_workspace"])
        .collect();
    if main_rows.len() != 1
        || main_rows[0]["branch"] != entry["target_branch"]
        || Path::new(field(main_rows[0], "path")?).canonicalize()? != repo.main_root
    {
        return Err(error(
            "Herdr primary checkout/source workspace ownership changed; cleanup refused",
        ));
    }
    Ok(result)
}

fn verify_open_workspace(
    repo: &RepositoryIdentity,
    entry: &Value,
    executor: &mut impl Executor,
) -> io::Result<()> {
    let result = worktree_inventory(repo, entry, executor)?;
    let rows = result["worktrees"]
        .as_array()
        .ok_or_else(|| error("Missing worktree verification rows"))?;
    let source = Path::new(field(entry, "path")?);
    let ownership: Vec<_> = rows
        .iter()
        .filter(|row| {
            row["open_workspace_id"] == entry["workspace_id"]
                || row["path"].as_str().is_some_and(|path| {
                    Path::new(path)
                        .canonicalize()
                        .is_ok_and(|path| path == source)
                })
        })
        .collect();
    if entry["workspace_id"] == entry["source_workspace"] || ownership.len() != 1 {
        return Err(error(
            "Recorded workspace no longer uniquely owns the managed checkout; nothing closed or removed",
        ));
    }
    let row = ownership[0];
    let path = Path::new(field(row, "path")?);
    if row["open_workspace_id"] != entry["workspace_id"]
        || row["branch"] != entry["branch"]
        || !path.is_absolute()
        || path.canonicalize()? != source
    {
        return Err(error(
            "Recorded workspace ID/path/branch ownership changed; nothing closed or removed",
        ));
    }
    Ok(())
}

fn confirm_cleanup(
    registry: &mut Registry,
    index: usize,
    executor: &mut impl Executor,
) -> io::Result<()> {
    let entry = &registry.entries[index];
    let operation = field(entry, "cleanup_pending")?.to_owned();
    let result = worktree_inventory(&registry.repo, entry, executor)?;
    let rows = result["worktrees"]
        .as_array()
        .ok_or_else(|| error("Missing worktree verification rows"))?;
    if rows
        .iter()
        .any(|row| row["open_workspace_id"] == entry["workspace_id"])
    {
        return Err(error(
            "Workspace still open; cleanup unconfirmed, inspect before recovery",
        ));
    }
    let source_ref = format!("refs/heads/{}", field(entry, "branch")?);
    let current_source = tip(&registry.repo.main_root, &source_ref)?;
    task_commits(&registry.repo, entry, &current_source)?;
    if default_branch(&registry.repo)? != field(entry, "target_branch")? {
        return Err(error("Default changed during cleanup"));
    }
    clean(&registry.repo.main_root)?;
    if !ancestor(
        &registry.repo.main_root,
        &current_source,
        &tip(&registry.repo.main_root, "HEAD")?,
    )? {
        return Err(error(
            "Source advanced during cleanup; new work is outstanding",
        ));
    }
    if operation == "remove" {
        let path = Path::new(field(entry, "path")?);
        if path.exists() || rows.iter().any(|row| row["path"] == entry["path"]) {
            return Err(error("Checkout removal unconfirmed; checkout still exists"));
        }
        let inventory = git(
            &registry.repo.main_root,
            &["worktree", "list", "--porcelain", "-z"],
        )?;
        if inventory
            .split('\0')
            .any(|line| line.strip_prefix("worktree ") == path.to_str())
        {
            return Err(error(
                "Checkout remains registered in Git; removal unconfirmed",
            ));
        }
        registry.entries[index]["checkout_removed"] = json!(true);
    } else {
        verify(&registry.repo, entry)?;
    }
    registry.entries[index]["workspace_closed"] = json!(true);
    registry.entries[index]["cleanup_pending"] = Value::Null;
    registry.entries[index]["detail"] = Value::Null;
    registry.save()
}

pub fn act(id: &str, action: Action) -> io::Result<ManagedWorktree> {
    act_with(&std::env::current_dir()?, id, action, &mut SystemExecutor)
}

fn act_with(
    cwd: &Path,
    id: &str,
    action: Action,
    executor: &mut impl Executor,
) -> io::Result<ManagedWorktree> {
    let mut registry = Registry::open(cwd)?;
    let index = registry.index(id)?;
    let result = match action {
        Action::RequestMerge => request_merge(&mut registry, index, executor, false),
        _ => cleanup(&mut registry, index, action, executor),
    };
    if let Err(e) = result {
        registry.entries[index]["detail"] = json!(e.to_string());
        registry.save()?;
        return Err(e);
    }
    Ok(inspect(&registry.repo, &registry.entries[index]))
}

pub fn reconcile(policy: CleanupPolicy) -> io::Result<Vec<ManagedWorktree>> {
    reconcile_with(&std::env::current_dir()?, policy, &mut SystemExecutor)
}

fn reconcile_with(
    cwd: &Path,
    policy: CleanupPolicy,
    executor: &mut impl Executor,
) -> io::Result<Vec<ManagedWorktree>> {
    let mut registry = Registry::open(cwd)?;
    for index in 0..registry.entries.len() {
        let result = (|| {
            if registry.entries[index]["phase"] != "active"
                || registry.entries[index]["checkout_removed"] == true
            {
                return Ok(());
            }
            // Read-only confirmation recovers successful-but-timed-out cleanup;
            // it NEVER retries the close/remove command.
            if registry.entries[index]["cleanup_pending"].is_string() {
                return confirm_cleanup(&mut registry, index, executor);
            }
            let status = inspect(&registry.repo, &registry.entries[index]).status;
            if matches!(status, WorktreeStatus::MergeRequested) {
                finalize(&mut registry, index, executor)?;
            }
            let status = inspect(&registry.repo, &registry.entries[index]).status;
            if !(policy.auto_merge || policy.auto_close_workspace || policy.auto_remove_checkout) {
                return Ok(());
            }
            if !matches!(
                status,
                WorktreeStatus::Unmerged
                    | WorktreeStatus::Integrated
                    | WorktreeStatus::WorkspaceClosed
            ) {
                return Ok(());
            }
            if (status == WorktreeStatus::Unmerged && !policy.auto_merge)
                || (status == WorktreeStatus::Integrated
                    && !(policy.auto_close_workspace || policy.auto_remove_checkout))
                || (status == WorktreeStatus::WorkspaceClosed && !policy.auto_remove_checkout)
            {
                return Ok(());
            }
            let entry = &registry.entries[index];
            if status == WorktreeStatus::WorkspaceClosed {
                return Err(error(CLOSED_REMOVAL_UNAVAILABLE));
            }
            let verified = verify(&registry.repo, entry)?;
            task_commits(&registry.repo, entry, &verified.source_tip)?;
            let bead = executor.show_bead(field(entry, "bead_id")?)?;
            if bead["id"] != entry["bead_id"] || bead["status"] != "closed" {
                return Ok(());
            }
            // Only open workspaces reach this gate. A closed workspace has no
            // documented Herdr removal path and must not query a departed agent.
            settled(executor, entry, true)?;
            if status == WorktreeStatus::Unmerged {
                if policy.auto_merge && entry["merge_request"].is_null() {
                    request_merge(&mut registry, index, executor, true)?;
                }
            } else if policy.auto_remove_checkout {
                // Removal closes the workspace too. Never close first when both enabled.
                cleanup(&mut registry, index, Action::RemoveCheckout, executor)?;
            } else if policy.auto_close_workspace && status == WorktreeStatus::Integrated {
                cleanup(&mut registry, index, Action::CloseWorkspace, executor)?;
            }
            Ok(())
        })();
        if let Err(e) = result {
            registry.entries[index]["detail"] = json!(e.to_string());
            registry.save()?;
        }
    }
    Ok(registry
        .entries
        .iter()
        .map(|entry| inspect_for_display(&registry.repo, entry, executor))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::VecDeque,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    const ID: &str = "ot-test";
    const BRANCH: &str = "ontrack/ot-test";

    fn test_bead() -> Value {
        json!({
            "id": "Bead-1",
            "title": "Fix the selected handoff",
            "description": "First line\nSecond line"
        })
    }

    struct Fixture {
        directory: PathBuf,
        main: PathBuf,
        source: PathBuf,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.directory);
        }
    }

    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let directory = loop {
                let nanos = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos();
                let path = std::env::temp_dir().join(format!(
                    "ontrack-worktrees-{}-{nanos}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                match fs::create_dir(&path) {
                    Ok(()) => break path,
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("Cannot create worktree test fixture: {error}"),
                }
            };
            let main = directory.join("main");
            fs::create_dir(&main).unwrap();
            git(&main, &["init", "--initial-branch=trunk"]).unwrap();
            for (key, value) in [
                ("user.name", "OnTrack Test"),
                ("user.email", "test@example.invalid"),
                ("commit.gpgsign", "false"),
                ("core.hooksPath", "/dev/null"),
                ("init.defaultBranch", "wrong-guess"),
            ] {
                git(&main, &["config", "--local", key, value]).unwrap();
            }
            Self::commit_at(&main, "file", "base\n");
            let source = directory.join("source");
            Self {
                directory,
                main,
                source,
            }
        }

        fn commit_at(path: &Path, name: &str, contents: &str) {
            fs::write(path.join(name), contents).unwrap();
            git(path, &["add", "--", name]).unwrap();
            git(path, &["commit", "-m", "test change"]).unwrap();
        }

        fn managed() -> Self {
            let fixture = Self::new();
            let mut journal =
                LaunchJournal::begin(&fixture.main, &test_bead(), BRANCH, "opencode", ID, "wMain")
                    .unwrap();
            git(
                &fixture.main,
                &[
                    "worktree",
                    "add",
                    "-b",
                    BRANCH,
                    fixture.source.to_str().unwrap(),
                ],
            )
            .unwrap();
            let created = json!({"workspace":{"workspace_id":"wTask"}, "root_pane":{"pane_id":"wTask:p1"}, "worktree":{"path":fixture.source, "branch":BRANCH}});
            journal
                .capture(
                    "create",
                    &json!({"result":created}).to_string().into_bytes(),
                )
                .unwrap();
            journal.created(&created).unwrap();
            journal
                .capture("ready", &fixture.agent(10, "idle").to_string().into_bytes())
                .unwrap();
            journal.phase("active").unwrap();
            drop(journal);
            fixture
        }

        fn agent(&self, seq: u64, state: &str) -> Value {
            json!({"result":{"type":"agent_info", "agent":{
                "pane_id":"wTask:p1", "workspace_id":"wTask", "agent":"opencode",
                "agent_status":state, "cwd":self.source, "foreground_cwd":self.source,
                "agent_session":{"agent":"opencode", "kind":"id", "source":"herdr:opencode", "value":"session-1"},
                "state_change_seq":seq
            }}})
        }

        fn prompt_result(&self) -> Value {
            json!({"result":{"type":"agent_prompted", "agent":{"pane_id":"wTask:p1", "workspace_id":"wTask"}}})
        }

        fn inventory(&self, open: bool, checkout: bool) -> Value {
            let mut rows =
                vec![json!({"path":self.main, "branch":"trunk", "open_workspace_id":"wMain"})];
            if checkout {
                rows.push(json!({"path":self.source, "branch":BRANCH, "open_workspace_id":if open {json!("wTask")} else {Value::Null}}));
            }
            json!({"result":{"type":"worktree_list", "source":{"source_checkout_path":self.main, "source_workspace_id":"wMain"}, "worktrees":rows}})
        }

        fn status(&self) -> ManagedWorktree {
            let registry = Registry::open(&self.main).unwrap();
            inspect(&registry.repo, &registry.entries[0])
        }

        fn request(&self) {
            let mut executor = Fake::default();
            executor.json(self.agent(12, "idle"));
            executor.json(self.prompt_result());
            assert_eq!(
                act_with(&self.main, ID, Action::RequestMerge, &mut executor)
                    .unwrap()
                    .status,
                WorktreeStatus::MergeRequested
            );
            assert_eq!(executor.calls[1][..3], ["agent", "prompt", "wTask:p1"]);
            assert!(executor.calls[1][3].contains("INTO THIS SOURCE BRANCH"));
        }

        fn integrated(&self) {
            Self::commit_at(&self.source, "task", "implemented\n");
            git(&self.main, &["merge", "--ff-only", BRANCH]).unwrap();
        }
    }

    type Hook = Box<dyn FnMut(&[&str])>;

    #[derive(Default)]
    struct Fake {
        outputs: VecDeque<io::Result<Vec<u8>>>,
        calls: Vec<Vec<String>>,
        bead: Option<Value>,
        hook: Option<Hook>,
    }

    impl Fake {
        fn json(&mut self, value: Value) {
            self.outputs.push_back(Ok(value.to_string().into_bytes()));
        }
    }

    impl Executor for Fake {
        fn run(&mut self, args: &[&str]) -> io::Result<Vec<u8>> {
            self.calls
                .push(args.iter().map(|s| (*s).to_owned()).collect());
            if let Some(hook) = self.hook.as_mut() {
                hook(args);
            }
            self.outputs
                .pop_front()
                .unwrap_or_else(|| panic!("Unexpected Herdr call: {args:?}"))
        }
        fn show_bead(&mut self, id: &str) -> io::Result<Value> {
            self.calls.push(vec!["show_bead".into(), id.into()]);
            Ok(self
                .bead
                .clone()
                .unwrap_or_else(|| json!({"id":id, "status":"closed"})))
        }
        fn git_identity(&mut self, path: &Path) -> io::Result<RepositoryIdentity> {
            SystemExecutor.git_identity(path)
        }
    }

    #[test]
    fn intent_is_durable_before_creation_and_exclusively_locked() {
        let fixture = Fixture::new();
        let journal =
            LaunchJournal::begin(&fixture.main, &test_bead(), BRANCH, "opencode", ID, "wMain")
                .unwrap();
        let persisted: Value = serde_json::from_slice(
            &fs::read(journal.registry.directory.join("registry.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(persisted["version"], 1);
        assert_eq!(persisted["entries"][0]["phase"], "create_possibly_sent");
        assert_eq!(
            persisted["entries"][0]["bead_title"],
            "Fix the selected handoff"
        );
        assert_eq!(
            persisted["entries"][0]["bead_description"],
            "First line\nSecond line"
        );
        assert!(!fixture.source.exists());
        assert!(
            Registry::open(&fixture.main)
                .err()
                .unwrap()
                .to_string()
                .contains("busy")
        );
        let child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "worktrees::tests::registry_lock_child"])
            .env("ONTRACK_TEST_LOCK_REPOSITORY", &fixture.main)
            .output()
            .unwrap();
        assert!(
            child.status.success(),
            "{}",
            String::from_utf8_lossy(&child.stdout)
        );
        drop(journal);
        assert_eq!(fixture.status().status, WorktreeStatus::NeedsAttention);
        assert!(!fixture.status().handoff_accepted);
        assert_eq!(
            fixture.status().bead_title.as_deref(),
            Some("Fix the selected handoff")
        );
        assert_eq!(
            fixture.status().bead_description.as_deref(),
            Some("First line\nSecond line")
        );
        assert!(
            LaunchJournal::begin(&fixture.main, &test_bead(), BRANCH, "opencode", ID, "wMain")
                .is_err()
        );
    }

    #[test]
    fn older_registry_entries_without_bead_context_remain_viewable() {
        let fixture = Fixture::managed();
        let mut registry = Registry::open(&fixture.main).unwrap();
        registry.entries[0]
            .as_object_mut()
            .unwrap()
            .remove("bead_title");
        registry.entries[0]
            .as_object_mut()
            .unwrap()
            .remove("bead_description");
        registry.save().unwrap();
        drop(registry);
        let item = fixture.status();
        assert_eq!(item.bead_id, "Bead-1");
        assert!(item.bead_title.is_none());
        assert!(item.bead_description.is_none());
    }

    #[test]
    fn accepted_handoff_is_derived_from_durable_active_phase() {
        let pending = json!({"id":"wt", "bead_id":"bead", "phase":"initial_prompt_possibly_sent"});
        let active = json!({"id":"wt", "bead_id":"bead", "phase":"active"});
        assert!(!view(&pending, WorktreeStatus::NeedsAttention, None).handoff_accepted);
        assert!(view(&active, WorktreeStatus::Unmerged, None).handoff_accepted);
    }

    #[test]
    fn registry_lock_child() {
        if let Some(repository) = std::env::var_os("ONTRACK_TEST_LOCK_REPOSITORY") {
            assert!(
                Registry::open(Path::new(&repository))
                    .err()
                    .unwrap()
                    .to_string()
                    .contains("busy")
            );
        }
    }

    #[test]
    fn partial_launch_response_and_failure_survive_restart() {
        let fixture = Fixture::new();
        let mut journal =
            LaunchJournal::begin(&fixture.main, &test_bead(), BRANCH, "opencode", ID, "wMain")
                .unwrap();
        journal
            .capture(
                "create",
                b"malformed but recoverable response with wTask:p1",
            )
            .unwrap();
        journal
            .failed("creation timed out; do not blindly retry")
            .unwrap();
        drop(journal);
        let registry = Registry::open(&fixture.main).unwrap();
        assert!(
            registry.entries[0]["responses"]["create"]
                .as_str()
                .unwrap()
                .contains("wTask:p1")
        );
        assert!(
            inspect(&registry.repo, &registry.entries[0])
                .detail
                .unwrap()
                .contains("timed out")
        );
    }

    #[test]
    fn corrupt_or_unknown_registry_is_not_overwritten() {
        let fixture = Fixture::managed();
        let directory = fixture.main.join(".git/ontrack-worktrees");
        for bytes in [b"not JSON".as_slice(), b"{\"version\":999}"] {
            fs::write(directory.join("registry.json"), bytes).unwrap();
            assert!(Registry::open(&fixture.main).is_err());
            assert_eq!(fs::read(directory.join("registry.json")).unwrap(), bytes);
        }
    }

    #[test]
    fn default_is_actual_primary_branch_not_init_guess_and_detached_refuses() {
        let fixture = Fixture::new();
        let repo = SystemExecutor.git_identity(&fixture.main).unwrap();
        assert_eq!(default_branch(&repo).unwrap(), "trunk");
        git(&fixture.main, &["checkout", "--detach"]).unwrap();
        assert!(
            default_branch(&repo)
                .unwrap_err()
                .to_string()
                .contains("detached")
        );
    }

    #[test]
    fn remote_heads_must_agree_and_match_primary_checkout() {
        let fixture = Fixture::new();
        let repo = SystemExecutor.git_identity(&fixture.main).unwrap();
        git(
            &fixture.main,
            &["update-ref", "refs/remotes/origin/trunk", "HEAD"],
        )
        .unwrap();
        git(
            &fixture.main,
            &[
                "symbolic-ref",
                "refs/remotes/origin/HEAD",
                "refs/remotes/origin/trunk",
            ],
        )
        .unwrap();
        assert_eq!(default_branch(&repo).unwrap(), "trunk");
        git(
            &fixture.main,
            &["update-ref", "refs/remotes/other/different", "HEAD"],
        )
        .unwrap();
        git(
            &fixture.main,
            &[
                "symbolic-ref",
                "refs/remotes/other/HEAD",
                "refs/remotes/other/different",
            ],
        )
        .unwrap();
        assert!(
            default_branch(&repo)
                .unwrap_err()
                .to_string()
                .contains("ambiguous")
        );
        git(
            &fixture.main,
            &["symbolic-ref", "--delete", "refs/remotes/other/HEAD"],
        )
        .unwrap();
        git(&fixture.main, &["checkout", "-b", "not-default"]).unwrap();
        assert!(
            default_branch(&repo)
                .unwrap_err()
                .to_string()
                .contains("must be on default branch trunk")
        );
    }

    #[test]
    fn merge_requires_source_resolution_then_verified_ff_only() {
        let fixture = Fixture::managed();
        Fixture::commit_at(&fixture.source, "task", "implemented\n");
        Fixture::commit_at(&fixture.main, "target", "target advanced\n");
        let before = tip(&fixture.main, "HEAD").unwrap();
        fixture.request();
        let result = reconcile_with(
            &fixture.main,
            CleanupPolicy::default(),
            &mut Fake::default(),
        )
        .unwrap();
        assert_eq!(result[0].status, WorktreeStatus::MergeRequested);
        assert!(
            result[0]
                .detail
                .as_ref()
                .unwrap()
                .contains("does not yet contain")
        );
        assert_eq!(tip(&fixture.main, "HEAD").unwrap(), before);
        git(&fixture.source, &["merge", "--no-edit", "trunk"]).unwrap();
        let source_tip = tip(&fixture.source, "HEAD").unwrap();
        let mut executor = Fake::default();
        executor.json(fixture.agent(14, "done"));
        let result =
            reconcile_with(&fixture.main, CleanupPolicy::default(), &mut executor).unwrap();
        assert_eq!(result[0].status, WorktreeStatus::Integrated);
        assert_eq!(tip(&fixture.main, "HEAD").unwrap(), source_tip);
        assert_eq!(branch(&fixture.main).unwrap(), "trunk");
        assert_eq!(branch(&fixture.source).unwrap(), BRANCH);
        assert_eq!(executor.calls.len(), 1);
    }

    #[test]
    fn automatic_dispatch_and_stale_idle_are_not_completion() {
        let fixture = Fixture::managed();
        Fixture::commit_at(&fixture.source, "task", "implemented\n");
        let before = tip(&fixture.main, "HEAD").unwrap();
        let mut request = Fake::default();
        request.json(fixture.agent(12, "idle"));
        request.json(fixture.prompt_result());
        {
            let mut registry = Registry::open(&fixture.main).unwrap();
            request_merge(&mut registry, 0, &mut request, true).unwrap();
        }
        let mut executor = Fake::default();
        executor.json(fixture.agent(12, "idle"));
        let rows = reconcile_with(&fixture.main, CleanupPolicy::default(), &mut executor).unwrap();
        assert!(
            rows[0]
                .detail
                .as_ref()
                .unwrap()
                .contains("not proof that this particular prompt completed")
        );
        assert_eq!(tip(&fixture.main, "HEAD").unwrap(), before);
    }

    #[test]
    fn timed_out_prompt_is_persisted_and_never_resent() {
        let fixture = Fixture::managed();
        Fixture::commit_at(&fixture.source, "task", "implemented\n");
        let mut executor = Fake::default();
        executor.json(fixture.agent(12, "idle"));
        executor
            .outputs
            .push_back(Err(error("timeout after possible delivery")));
        assert!(act_with(&fixture.main, ID, Action::RequestMerge, &mut executor).is_err());
        let mut retry = Fake::default();
        let e = act_with(&fixture.main, ID, Action::RequestMerge, &mut retry).unwrap_err();
        assert!(e.to_string().contains("already possibly sent"));
        assert!(retry.calls.is_empty());
        let registry = Registry::open(&fixture.main).unwrap();
        assert_eq!(registry.entries[0]["merge_request"]["possibly_sent"], true);
    }

    #[test]
    fn target_advancement_requires_explicit_new_resolution_request() {
        let fixture = Fixture::managed();
        Fixture::commit_at(&fixture.source, "task", "implemented\n");
        fixture.request();
        Fixture::commit_at(&fixture.main, "target", "advanced after prompt\n");
        let before = tip(&fixture.main, "HEAD").unwrap();
        let mut executor = Fake::default();
        executor.json(fixture.agent(13, "idle"));
        let rows = reconcile_with(&fixture.main, CleanupPolicy::default(), &mut executor).unwrap();
        assert_eq!(rows[0].status, WorktreeStatus::NeedsResolution);
        assert_eq!(executor.calls, vec![vec!["agent", "get", "wTask:p1"]]);
        assert_eq!(tip(&fixture.main, "HEAD").unwrap(), before);
        fixture.request();
    }

    #[test]
    fn working_agent_keeps_unresolved_merge_pending_in_worktree_list() {
        let fixture = Fixture::managed();
        Fixture::commit_at(&fixture.source, "file", "source change\n");
        Fixture::commit_at(&fixture.main, "file", "target change\n");
        fixture.request();
        assert!(git(&fixture.source, &["merge", "--no-edit", "trunk"]).is_err());
        assert_eq!(fixture.status().status, WorktreeStatus::NeedsResolution);

        let mut working = Fake::default();
        working.json(fixture.agent(13, "working"));
        let rows = list_with(&fixture.main, &mut working).unwrap();
        assert_eq!(rows[0].status, WorktreeStatus::MergeRequested);
        assert!(rows[0].detail.as_deref().unwrap().contains("still working"));
        assert_eq!(working.calls, vec![vec!["agent", "get", "wTask:p1"]]);

        for state in ["idle", "done", "blocked"] {
            let mut executor = Fake::default();
            executor.json(fixture.agent(14, state));
            assert_eq!(
                list_with(&fixture.main, &mut executor).unwrap()[0].status,
                WorktreeStatus::NeedsResolution,
                "{state}"
            );
        }
        let mut mismatched = fixture.agent(14, "working");
        mismatched["result"]["agent"]["agent_session"]["value"] = json!("replacement");
        let mut executor = Fake::default();
        executor.json(mismatched);
        assert_eq!(
            list_with(&fixture.main, &mut executor).unwrap()[0].status,
            WorktreeStatus::NeedsResolution
        );
        let mut unavailable = Fake::default();
        unavailable
            .outputs
            .push_back(Err(error("Herdr unavailable")));
        assert_eq!(
            list_with(&fixture.main, &mut unavailable).unwrap()[0].status,
            WorktreeStatus::NeedsResolution
        );
    }

    #[test]
    fn working_agent_keeps_advanced_target_pending() {
        let fixture = Fixture::managed();
        Fixture::commit_at(&fixture.source, "task", "implemented\n");
        fixture.request();
        Fixture::commit_at(&fixture.main, "target", "advanced after prompt\n");
        let mut executor = Fake::default();
        executor.json(fixture.agent(13, "working"));
        let rows = list_with(&fixture.main, &mut executor).unwrap();
        assert_eq!(rows[0].status, WorktreeStatus::MergeRequested);
        assert!(
            rows[0]
                .detail
                .as_deref()
                .unwrap()
                .contains("Target advanced")
        );
        assert_eq!(fixture.status().status, WorktreeStatus::NeedsResolution);

        let before = tip(&fixture.main, "HEAD").unwrap();
        let mut reconcile = Fake::default();
        reconcile.json(fixture.agent(13, "working"));
        let rows = reconcile_with(&fixture.main, CleanupPolicy::default(), &mut reconcile).unwrap();
        assert_eq!(rows[0].status, WorktreeStatus::MergeRequested);
        assert_eq!(tip(&fixture.main, "HEAD").unwrap(), before);
        assert_eq!(reconcile.calls, vec![vec!["agent", "get", "wTask:p1"]]);
    }

    #[test]
    fn conflicts_stay_in_source_and_are_visible_until_resolved() {
        let fixture = Fixture::managed();
        Fixture::commit_at(&fixture.source, "file", "source change\n");
        Fixture::commit_at(&fixture.main, "file", "target change\n");
        fixture.request();
        let before = tip(&fixture.main, "HEAD").unwrap();
        assert!(git(&fixture.source, &["merge", "--no-edit", "trunk"]).is_err());
        assert_eq!(fixture.status().status, WorktreeStatus::NeedsResolution);
        assert!(
            act_with(
                &fixture.main,
                ID,
                Action::RemoveCheckout,
                &mut Fake::default()
            )
            .is_err()
        );
        assert_eq!(tip(&fixture.main, "HEAD").unwrap(), before);
        Fixture::commit_at(&fixture.source, "file", "resolved both\n");
        let mut executor = Fake::default();
        executor.json(fixture.agent(14, "done"));
        assert_eq!(
            reconcile_with(&fixture.main, CleanupPolicy::default(), &mut executor).unwrap()[0]
                .status,
            WorktreeStatus::Integrated
        );
    }

    #[test]
    fn dirty_target_dirty_source_wrong_branch_and_missing_source_block_mutations() {
        for mode in ["target", "source", "branch", "missing"] {
            let fixture = Fixture::managed();
            fixture.integrated();
            match mode {
                "target" => fs::write(fixture.main.join("dirty"), "untracked").unwrap(),
                "source" => fs::write(fixture.source.join("dirty"), "untracked").unwrap(),
                "branch" => {
                    git(&fixture.source, &["checkout", "-b", "other"]).unwrap();
                }
                _ => {
                    fs::rename(&fixture.source, fixture.directory.join("moved")).unwrap();
                }
            }
            assert_eq!(
                fixture.status().status,
                WorktreeStatus::NeedsAttention,
                "{mode}"
            );
            let mut executor = Fake::default();
            assert!(
                act_with(&fixture.main, ID, Action::RemoveCheckout, &mut executor).is_err(),
                "{mode}"
            );
            assert!(executor.calls.is_empty());
        }
    }

    #[test]
    fn external_integration_is_recognized_and_source_advancement_reopens() {
        let fixture = Fixture::managed();
        fixture.integrated();
        assert_eq!(fixture.status().status, WorktreeStatus::Integrated);
        Fixture::commit_at(&fixture.source, "new-task", "more work\n");
        assert_eq!(fixture.status().status, WorktreeStatus::Unmerged);
        assert!(
            act_with(
                &fixture.main,
                ID,
                Action::CloseWorkspace,
                &mut Fake::default()
            )
            .is_err()
        );
    }

    #[test]
    fn finalization_rechecks_tips_after_native_agent_observation() {
        let fixture = Fixture::managed();
        Fixture::commit_at(&fixture.source, "task", "implemented\n");
        fixture.request();
        let main = fixture.main.clone();
        let mut advanced = false;
        let mut executor = Fake {
            hook: Some(Box::new(move |_| {
                if !advanced {
                    Fixture::commit_at(&main, "race", "advanced concurrently\n");
                    advanced = true;
                }
            })),
            ..Fake::default()
        };
        executor.json(fixture.agent(14, "done"));
        executor.json(fixture.agent(14, "done"));
        let rows = reconcile_with(&fixture.main, CleanupPolicy::default(), &mut executor).unwrap();
        assert_eq!(rows[0].status, WorktreeStatus::NeedsResolution);
        assert!(
            !ancestor(
                &fixture.main,
                &tip(&fixture.source, "HEAD").unwrap(),
                &tip(&fixture.main, "HEAD").unwrap()
            )
            .unwrap()
        );
    }

    #[test]
    fn cleanup_close_uses_exact_command_and_confirms_workspace_topology() {
        let fixture = Fixture::managed();
        fixture.integrated();
        let mut executor = Fake::default();
        executor.json(fixture.inventory(true, true));
        executor.json(json!({"result":{"type":"workspace_closed"}}));
        executor.json(fixture.inventory(false, true));
        let outcome = act_with(&fixture.main, ID, Action::CloseWorkspace, &mut executor).unwrap();
        assert_eq!(outcome.status, WorktreeStatus::WorkspaceClosed);
        assert_eq!(executor.calls[1], ["workspace", "close", "wTask"]);
        assert!(fixture.source.exists());
        assert!(tip(&fixture.main, &format!("refs/heads/{BRANCH}")).is_ok());
    }

    #[test]
    fn restart_checks_ownership_before_any_mutation() {
        let fixture = Fixture::managed();
        let mut wrong = fixture.inventory(true, true);
        wrong["result"]["worktrees"][1]["branch"] = json!("other-branch");
        let mut executor = Fake::default();
        executor.json(wrong);
        assert!(restart_with(&fixture.main, ID, &mut executor).is_err());
        assert_eq!(executor.calls.len(), 1);
        assert_eq!(executor.calls[0][..2], ["worktree", "list"]);
        assert!(fixture.source.exists());
        assert!(Registry::open(&fixture.main).unwrap().entries[0]["restart_pending"].is_null());
    }

    #[test]
    fn restart_force_removal_is_journaled_and_never_blindly_retried() {
        let fixture = Fixture::managed();
        fs::write(
            fixture.source.join("uncommitted"),
            "discard on confirmed restart",
        )
        .unwrap();
        let mut executor = Fake {
            bead: Some(test_bead()),
            ..Default::default()
        };
        executor.json(fixture.inventory(true, true));
        executor.json(fixture.inventory(true, true));
        executor
            .outputs
            .push_back(Ok(b"--kind <KIND> [possible values: opencode]".to_vec()));
        executor
            .outputs
            .push_back(Ok(b"opencode: current (v1) (/tmp/opencode)".to_vec()));
        executor.json(fixture.inventory(true, true));
        executor
            .outputs
            .push_back(Err(error("Herdr timed out after removal may have started")));
        assert!(restart_with(&fixture.main, ID, &mut executor).is_err());
        assert_eq!(
            executor.calls.last().unwrap(),
            &["worktree", "remove", "--workspace", "wTask", "--force"]
        );
        assert_eq!(fixture.status().status, WorktreeStatus::NeedsAttention);
        let registry = Registry::open(&fixture.main).unwrap();
        assert_eq!(registry.entries[0]["restart_pending"], true);
        drop(registry);
        let mut retry = Fake::default();
        assert!(restart_with(&fixture.main, ID, &mut retry).is_err());
        assert!(retry.calls.is_empty());
    }

    #[test]
    fn restart_verifies_forced_removal_and_exposes_failed_relaunch() {
        let fixture = Fixture::managed();
        fs::write(fixture.source.join("uncommitted"), "discard me").unwrap();
        let main = fixture.main.clone();
        let source = fixture.source.clone();
        let mut executor = Fake {
            bead: Some(test_bead()),
            hook: Some(Box::new(move |args| {
                if args.starts_with(&["worktree", "remove"]) {
                    git(
                        &main,
                        &["worktree", "remove", "--force", source.to_str().unwrap()],
                    )
                    .unwrap();
                }
            })),
            ..Default::default()
        };
        executor.json(fixture.inventory(true, true));
        executor.json(fixture.inventory(true, true));
        executor
            .outputs
            .push_back(Ok(b"[possible values: opencode]".to_vec()));
        executor
            .outputs
            .push_back(Ok(b"opencode: current (v1) (/tmp/opencode)".to_vec()));
        executor.json(fixture.inventory(true, true));
        executor.json(json!({"result":{"type":"worktree_removed"}}));
        executor.json(fixture.inventory(false, false));
        executor
            .outputs
            .push_back(Err(error("discovery unavailable")));
        assert!(restart_with(&fixture.main, ID, &mut executor).is_err());
        assert!(!fixture.source.exists());
        assert!(tip(&fixture.main, &format!("refs/heads/{BRANCH}")).is_ok());
        assert_eq!(fixture.status().status, WorktreeStatus::NeedsAttention);
        let registry = Registry::open(&fixture.main).unwrap();
        assert_eq!(registry.entries[0]["checkout_removed"], true);
        assert_eq!(registry.entries[0]["restarted"], true);
        assert!(registry.entries[0]["restart_launch_error"].is_string());
        assert!(registry.entries[0]["restart_launch_pending"].is_null());
    }

    #[test]
    fn failed_cleanup_is_not_retried_and_can_recover_from_read_only_confirmation() {
        let fixture = Fixture::managed();
        fixture.integrated();
        let mut executor = Fake::default();
        executor.json(fixture.inventory(true, true));
        executor
            .outputs
            .push_back(Err(error("close timed out; may have succeeded")));
        assert!(act_with(&fixture.main, ID, Action::CloseWorkspace, &mut executor).is_err());
        assert_eq!(fixture.status().status, WorktreeStatus::NeedsAttention);
        let mut confirm = Fake::default();
        confirm.json(fixture.inventory(true, true));
        let rows = reconcile_with(&fixture.main, CleanupPolicy::default(), &mut confirm).unwrap();
        assert_eq!(rows[0].status, WorktreeStatus::NeedsAttention);
        assert_eq!(confirm.calls[0][..2], ["worktree", "list"]);
        let mut confirm = Fake::default();
        confirm.json(fixture.inventory(false, true));
        let rows = reconcile_with(&fixture.main, CleanupPolicy::default(), &mut confirm).unwrap();
        assert_eq!(rows[0].status, WorktreeStatus::WorkspaceClosed);
        assert_eq!(confirm.calls.len(), 1);
    }

    #[test]
    fn auto_remove_precedes_close_and_preserves_branch() {
        let fixture = Fixture::managed();
        fixture.integrated();
        let main = fixture.main.clone();
        let source = fixture.source.clone();
        let mut executor = Fake {
            hook: Some(Box::new(move |args| {
                if args.starts_with(&["worktree", "remove"]) {
                    git(&main, &["worktree", "remove", source.to_str().unwrap()]).unwrap();
                }
            })),
            ..Fake::default()
        };
        executor.json(fixture.agent(12, "done"));
        executor.json(fixture.inventory(true, true));
        executor.json(json!({"result":{"type":"worktree_removed"}}));
        executor.json(fixture.inventory(false, false));
        let policy = CleanupPolicy {
            auto_merge: true,
            auto_close_workspace: true,
            auto_remove_checkout: true,
        };
        let rows = reconcile_with(&fixture.main, policy, &mut executor).unwrap();
        assert_eq!(rows[0].status, WorktreeStatus::CheckoutRemoved);
        assert_eq!(
            executor.calls[3],
            ["worktree", "remove", "--workspace", "wTask"]
        );
        assert!(
            !executor
                .calls
                .iter()
                .flatten()
                .any(|s| s == "--force" || s == "--group" || s == "close")
        );
        assert!(tip(&fixture.main, &format!("refs/heads/{BRANCH}")).is_ok());
    }

    #[test]
    fn automatic_actions_require_closed_bead_original_session_and_post_prompt_activity() {
        for mode in [
            "open",
            "unknown",
            "working",
            "blocked",
            "stale",
            "changed_session",
            "missing_session",
        ] {
            let fixture = Fixture::managed();
            fixture.integrated();
            let mut executor = Fake::default();
            let mut agent = fixture.agent(
                if mode == "stale" { 10 } else { 12 },
                if matches!(mode, "unknown" | "working" | "blocked") {
                    mode
                } else {
                    "done"
                },
            );
            if mode == "open" {
                executor.bead = Some(json!({"id":"Bead-1", "status":"open"}));
            }
            if mode == "changed_session" {
                agent["result"]["agent"]["agent_session"]["value"] = json!("replaced");
            }
            if mode == "missing_session" {
                let mut registry = Registry::open(&fixture.main).unwrap();
                registry.entries[0]["agent_session"] = Value::Null;
                registry.save().unwrap();
            }
            executor.json(agent);
            let rows = reconcile_with(
                &fixture.main,
                CleanupPolicy {
                    auto_merge: true,
                    auto_close_workspace: true,
                    auto_remove_checkout: true,
                },
                &mut executor,
            )
            .unwrap();
            assert_eq!(rows[0].status, WorktreeStatus::Integrated, "{mode}");
            assert!(
                !executor.calls.iter().any(|args| args
                    .get(1)
                    .is_some_and(|s| s == "remove" || s == "close" || s == "prompt")),
                "{mode}"
            );
            if mode != "open" {
                assert!(rows[0].detail.is_some(), "{mode}");
            }
        }
    }

    #[test]
    fn default_policy_neither_reads_beads_nor_closes_or_removes() {
        let fixture = Fixture::managed();
        fixture.integrated();
        let mut executor = Fake::default();
        assert_eq!(
            reconcile_with(&fixture.main, CleanupPolicy::default(), &mut executor).unwrap()[0]
                .status,
            WorktreeStatus::Integrated
        );
        assert!(executor.calls.is_empty());
    }

    #[test]
    fn closed_bead_with_native_completed_session_can_request_auto_merge_once() {
        let fixture = Fixture::managed();
        Fixture::commit_at(&fixture.source, "task", "implemented\n");
        let before = tip(&fixture.main, "HEAD").unwrap();
        let mut executor = Fake::default();
        executor.json(fixture.agent(12, "done"));
        executor.json(fixture.agent(12, "done"));
        executor.json(fixture.prompt_result());
        let policy = CleanupPolicy {
            auto_merge: true,
            ..CleanupPolicy::default()
        };
        let rows = reconcile_with(&fixture.main, policy, &mut executor).unwrap();
        assert_eq!(rows[0].status, WorktreeStatus::MergeRequested);
        assert_eq!(tip(&fixture.main, "HEAD").unwrap(), before);
        assert_eq!(
            executor
                .calls
                .iter()
                .filter(|args| args.get(1).is_some_and(|s| s == "prompt"))
                .count(),
            1
        );
        let mut next = Fake::default();
        next.json(fixture.agent(14, "done"));
        next.bead = Some(json!({"id":"Bead-1", "status":"closed"}));
        next.json(fixture.agent(14, "done"));
        assert_eq!(
            reconcile_with(&fixture.main, policy, &mut next).unwrap()[0].status,
            WorktreeStatus::Integrated
        );
        assert!(
            !next
                .calls
                .iter()
                .any(|args| args.get(1).is_some_and(|s| s == "prompt"))
        );
    }

    #[test]
    fn timed_out_removal_recovers_without_reissuing_and_branch_advancement_is_visible() {
        let fixture = Fixture::managed();
        fixture.integrated();
        let main = fixture.main.clone();
        let source = fixture.source.clone();
        let mut executor = Fake {
            hook: Some(Box::new(move |args| {
                if args.starts_with(&["worktree", "remove"]) {
                    git(&main, &["worktree", "remove", source.to_str().unwrap()]).unwrap();
                }
            })),
            ..Fake::default()
        };
        executor.json(fixture.inventory(true, true));
        executor
            .outputs
            .push_back(Err(error("remove timed out after possible success")));
        assert!(act_with(&fixture.main, ID, Action::RemoveCheckout, &mut executor).is_err());
        let mut confirm = Fake::default();
        confirm.json(fixture.inventory(false, false));
        let rows = reconcile_with(&fixture.main, CleanupPolicy::default(), &mut confirm).unwrap();
        assert_eq!(rows[0].status, WorktreeStatus::CheckoutRemoved);
        assert_eq!(confirm.calls.len(), 1);
        assert_eq!(confirm.calls[0][..2], ["worktree", "list"]);
        git(
            &fixture.main,
            &[
                "worktree",
                "add",
                fixture.directory.join("new-checkout").to_str().unwrap(),
                BRANCH,
            ],
        )
        .unwrap();
        Fixture::commit_at(
            &fixture.directory.join("new-checkout"),
            "new-work",
            "not integrated\n",
        );
        assert_eq!(fixture.status().status, WorktreeStatus::NeedsAttention);
        assert!(
            fixture
                .status()
                .detail
                .unwrap()
                .contains("advanced after checkout removal")
        );
    }

    #[test]
    fn dangling_remote_default_ref_is_not_treated_as_absent() {
        let fixture = Fixture::new();
        let repo = SystemExecutor.git_identity(&fixture.main).unwrap();
        git(
            &fixture.main,
            &["remote", "add", "origin", "/nonexistent-test-remote"],
        )
        .unwrap();
        git(
            &fixture.main,
            &[
                "symbolic-ref",
                "refs/remotes/origin/HEAD",
                "refs/remotes/origin/missing",
            ],
        )
        .unwrap();
        assert!(default_branch(&repo).is_err());
    }

    #[test]
    fn cleanup_rejects_reassigned_workspace_ids_and_changed_checkout_relationships() {
        for action in [Action::CloseWorkspace, Action::RemoveCheckout] {
            for mode in [
                "reassigned",
                "wrong_path",
                "wrong_branch",
                "wrong_source",
                "wrong_primary_owner",
            ] {
                let fixture = Fixture::managed();
                fixture.integrated();
                let mut inventory = fixture.inventory(true, true);
                match mode {
                    "reassigned" => {
                        inventory["result"]["worktrees"][1]["open_workspace_id"] = json!("wOther");
                        inventory["result"]["worktrees"].as_array_mut().unwrap().push(json!({"path":fixture.main, "branch":"trunk", "open_workspace_id":"wTask"}));
                    }
                    "wrong_path" => {
                        inventory["result"]["worktrees"][1]["path"] = json!(fixture.main)
                    }
                    "wrong_branch" => {
                        inventory["result"]["worktrees"][1]["branch"] = json!("unrelated-branch")
                    }
                    "wrong_source" => {
                        inventory["result"]["source"]["source_workspace_id"] = json!("wReassigned")
                    }
                    _ => {
                        inventory["result"]["worktrees"][0]["open_workspace_id"] =
                            json!("wReassigned")
                    }
                }
                let mut executor = Fake::default();
                executor.json(inventory);
                assert!(
                    act_with(&fixture.main, ID, action, &mut executor).is_err(),
                    "{action:?} {mode}"
                );
                assert_eq!(executor.calls.len(), 1, "{action:?} {mode}");
                assert_eq!(executor.calls[0][..2], ["worktree", "list"]);
                assert!(fixture.source.exists());
                let registry = Registry::open(&fixture.main).unwrap();
                assert!(registry.entries[0]["cleanup_pending"].is_null());
            }
        }
    }

    #[test]
    fn immediately_closed_bead_does_not_complete_or_clean_untouched_checkout() {
        let fixture = Fixture::managed();
        let before = tip(&fixture.main, "HEAD").unwrap();
        let mut executor = Fake::default();
        executor.json(fixture.agent(12, "done"));
        let rows = reconcile_with(
            &fixture.main,
            CleanupPolicy {
                auto_merge: true,
                auto_close_workspace: true,
                auto_remove_checkout: true,
            },
            &mut executor,
        )
        .unwrap();
        assert_eq!(rows[0].status, WorktreeStatus::Unmerged);
        assert!(rows[0].detail.as_ref().unwrap().contains("No task commits"));
        assert!(executor.calls.is_empty());
        assert!(fixture.source.exists());
        assert_eq!(tip(&fixture.main, "HEAD").unwrap(), before);
        for action in [
            Action::RequestMerge,
            Action::CloseWorkspace,
            Action::RemoveCheckout,
        ] {
            assert!(
                act_with(&fixture.main, ID, action, &mut executor)
                    .unwrap_err()
                    .to_string()
                    .contains("No-op completion is not supported")
            );
        }
        assert!(executor.calls.is_empty());
    }

    #[test]
    fn close_now_remove_later_is_explicitly_unavailable_without_querying_departed_agent() {
        let fixture = Fixture::managed();
        fixture.integrated();
        let mut close = Fake::default();
        close.json(fixture.inventory(true, true));
        close.json(json!({"result":{"type":"workspace_closed"}}));
        close.json(fixture.inventory(false, true));
        let result = act_with(&fixture.main, ID, Action::CloseWorkspace, &mut close).unwrap();
        assert_eq!(result.status, WorktreeStatus::WorkspaceClosed);
        assert!(
            result
                .detail
                .unwrap()
                .contains("removal unavailable after workspace closure")
        );
        let mut later = Fake::default();
        let rows = reconcile_with(
            &fixture.main,
            CleanupPolicy {
                auto_merge: true,
                auto_close_workspace: true,
                auto_remove_checkout: true,
            },
            &mut later,
        )
        .unwrap();
        assert_eq!(rows[0].status, WorktreeStatus::WorkspaceClosed);
        assert!(
            rows[0]
                .detail
                .as_ref()
                .unwrap()
                .contains("non-force worktree removal")
        );
        assert!(later.calls.is_empty());
        assert!(
            act_with(&fixture.main, ID, Action::RemoveCheckout, &mut later)
                .unwrap_err()
                .to_string()
                .contains("Do not reuse the closed ID")
        );
        assert!(later.calls.is_empty());
        assert!(fixture.source.exists());
    }

    #[test]
    fn manual_merge_without_optional_session_or_sequence_metadata_uses_current_git_history() {
        for missing_at_launch in [false, true] {
            let fixture = Fixture::managed();
            if missing_at_launch {
                let mut registry = Registry::open(&fixture.main).unwrap();
                registry.entries[0]["agent_session"] = Value::Null;
                registry.entries[0]["initial_lifecycle_seq"] = Value::Null;
                registry.save().unwrap();
            }
            Fixture::commit_at(&fixture.source, "task", "implemented\n");
            Fixture::commit_at(&fixture.main, "latest-target", "target changed\n");
            let mut native = fixture.agent(12, "idle");
            native["result"]["agent"]
                .as_object_mut()
                .unwrap()
                .remove("agent_session");
            native["result"]["agent"]
                .as_object_mut()
                .unwrap()
                .remove("state_change_seq");
            let mut request = Fake::default();
            request.json(native.clone());
            request.json(fixture.prompt_result());
            assert_eq!(
                act_with(&fixture.main, ID, Action::RequestMerge, &mut request)
                    .unwrap()
                    .status,
                WorktreeStatus::MergeRequested
            );
            {
                let registry = Registry::open(&fixture.main).unwrap();
                assert_eq!(registry.entries[0]["merge_request"]["provenance"], "manual");
                assert!(registry.entries[0]["merge_request"]["agent_session"].is_null());
                assert!(registry.entries[0]["merge_request"]["lifecycle_seq"].is_null());
            }
            git(&fixture.source, &["merge", "--no-edit", "trunk"]).unwrap();
            let mut complete = Fake::default();
            complete.json(native.clone());
            let rows =
                reconcile_with(&fixture.main, CleanupPolicy::default(), &mut complete).unwrap();
            assert_eq!(rows[0].status, WorktreeStatus::Integrated);
            assert_eq!(
                tip(&fixture.main, "HEAD").unwrap(),
                tip(&fixture.source, "HEAD").unwrap()
            );
            let mut automatic_cleanup = Fake::default();
            automatic_cleanup.json(native);
            let rows = reconcile_with(
                &fixture.main,
                CleanupPolicy {
                    auto_merge: true,
                    auto_close_workspace: true,
                    auto_remove_checkout: true,
                },
                &mut automatic_cleanup,
            )
            .unwrap();
            assert_eq!(rows[0].status, WorktreeStatus::Integrated);
            assert!(rows[0].detail.is_some());
            assert!(!automatic_cleanup.calls.iter().any(|args| {
                args.get(1)
                    .is_some_and(|s| s == "remove" || s == "close" || s == "prompt")
            }));
            assert!(fixture.source.exists());
        }
    }

    #[test]
    fn manual_dispatch_and_finalization_allow_null_or_omitted_optional_checkout_paths() {
        for keys in [
            vec!["foreground_cwd"],
            vec!["cwd"],
            vec!["cwd", "foreground_cwd"],
        ] {
            for omitted in [false, true] {
                let fixture = Fixture::managed();
                Fixture::commit_at(&fixture.source, "task", "implemented\n");
                Fixture::commit_at(&fixture.main, "latest-target", "target changed\n");
                let mut native = fixture.agent(12, "idle");
                for key in &keys {
                    if omitted {
                        native["result"]["agent"]
                            .as_object_mut()
                            .unwrap()
                            .remove(*key);
                    } else {
                        native["result"]["agent"][*key] = Value::Null;
                    }
                }
                let mut request = Fake::default();
                request.json(native.clone());
                request.json(fixture.prompt_result());
                assert_eq!(
                    act_with(&fixture.main, ID, Action::RequestMerge, &mut request)
                        .unwrap()
                        .status,
                    WorktreeStatus::MergeRequested,
                    "{keys:?}, omitted={omitted}"
                );
                assert_eq!(request.calls[1][..3], ["agent", "prompt", "wTask:p1"]);
                git(&fixture.source, &["merge", "--no-edit", "trunk"]).unwrap();
                let mut complete = Fake::default();
                complete.json(native);
                let rows =
                    reconcile_with(&fixture.main, CleanupPolicy::default(), &mut complete).unwrap();
                assert_eq!(
                    rows[0].status,
                    WorktreeStatus::Integrated,
                    "{keys:?}, omitted={omitted}"
                );
                assert_eq!(
                    tip(&fixture.main, "HEAD").unwrap(),
                    tip(&fixture.source, "HEAD").unwrap()
                );
            }
        }
    }

    #[test]
    fn manual_dispatch_and_finalization_refuse_invalid_or_mismatched_reported_paths() {
        for key in ["cwd", "foreground_cwd"] {
            for mode in [
                "wrong_checkout",
                "invalid_type",
                "empty",
                "relative",
                "unresolvable",
            ] {
                let fixture = Fixture::managed();
                Fixture::commit_at(&fixture.source, "task", "implemented\n");
                Fixture::commit_at(&fixture.main, "latest-target", "target changed\n");
                let mut native = fixture.agent(12, "idle");
                let (value, expected) = match mode {
                    "wrong_checkout" => (json!(fixture.main), "does not match recorded checkout"),
                    "invalid_type" => (json!(42), "is invalid"),
                    "empty" => (json!(""), "is invalid"),
                    "relative" => (json!("relative/path"), "is invalid"),
                    _ => (
                        json!(fixture.directory.join("missing-checkout")),
                        "cannot be resolved",
                    ),
                };
                native["result"]["agent"][key] = value;
                let mut invalid_dispatch = Fake::default();
                invalid_dispatch.json(native.clone());
                let message = act_with(
                    &fixture.main,
                    ID,
                    Action::RequestMerge,
                    &mut invalid_dispatch,
                )
                .unwrap_err()
                .to_string();
                assert!(
                    message.contains(key) && message.contains(expected),
                    "{mode}: {message}"
                );
                assert_eq!(invalid_dispatch.calls.len(), 1);
                {
                    let registry = Registry::open(&fixture.main).unwrap();
                    assert!(registry.entries[0]["merge_request"].is_null());
                }
                fixture.request();
                git(&fixture.source, &["merge", "--no-edit", "trunk"]).unwrap();
                let before = tip(&fixture.main, "HEAD").unwrap();
                let mut invalid_completion = Fake::default();
                invalid_completion.json(native);
                let rows = reconcile_with(
                    &fixture.main,
                    CleanupPolicy::default(),
                    &mut invalid_completion,
                )
                .unwrap();
                assert_eq!(rows[0].status, WorktreeStatus::MergeRequested);
                let message = rows[0].detail.as_ref().unwrap();
                assert!(
                    message.contains(key) && message.contains(expected),
                    "{mode}: {message}"
                );
                assert_eq!(tip(&fixture.main, "HEAD").unwrap(), before);
            }
        }
    }

    #[test]
    fn automatic_cleanup_defers_when_optional_checkout_path_evidence_is_unavailable() {
        for key in ["cwd", "foreground_cwd"] {
            for omitted in [false, true] {
                let fixture = Fixture::managed();
                fixture.integrated();
                let mut native = fixture.agent(12, "done");
                if omitted {
                    native["result"]["agent"]
                        .as_object_mut()
                        .unwrap()
                        .remove(key);
                } else {
                    native["result"]["agent"][key] = Value::Null;
                }
                let mut executor = Fake::default();
                executor.json(native);
                let rows = reconcile_with(
                    &fixture.main,
                    CleanupPolicy {
                        auto_merge: true,
                        auto_close_workspace: true,
                        auto_remove_checkout: true,
                    },
                    &mut executor,
                )
                .unwrap();
                assert_eq!(rows[0].status, WorktreeStatus::Integrated);
                assert!(
                    rows[0]
                        .detail
                        .as_ref()
                        .unwrap()
                        .contains(&format!("native agent {key} is unavailable"))
                );
                assert_eq!(executor.calls.len(), 2);
                assert_eq!(executor.calls[1][..2], ["agent", "get"]);
                assert!(fixture.source.exists());
            }
        }
    }
}
