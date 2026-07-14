#![allow(dead_code)]

use std::process::Command as StdCommand;

pub fn run_git(args: &[&str]) -> (String, String, bool) {
    match StdCommand::new("git").args(args).output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let success = output.status.success();
            (stdout, stderr, success)
        }
        Err(e) => (String::new(), format!("Failed to run git: {}", e), false),
    }
}

#[derive(Debug, Clone)]
pub struct GitFile {
    pub path: String,
    pub status: char,
    pub index_status: char,
    pub worktree_status: char,
}

#[derive(Debug, Clone)]
pub struct GitStatus {
    pub branch: String,
    pub ahead: usize,
    pub behind: usize,
    pub staged: Vec<GitFile>,
    pub unstaged: Vec<GitFile>,
    pub untracked: Vec<GitFile>,
}

pub fn git_status() -> GitStatus {
    let (stdout, _stderr, _success) = run_git(&["status", "--porcelain", "-b"]);
    let mut branch = String::new();
    let mut ahead: usize = 0;
    let mut behind: usize = 0;
    let mut staged = Vec::new();
    let mut unstaged = Vec::new();
    let mut untracked = Vec::new();

    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix("## ") {
            if let Some((br, extra)) = rest.split_once("...") {
                branch = br.to_string();
                if extra.contains("ahead") {
                    if let Some(n) = extra
                        .split("ahead ")
                        .nth(1)
                        .and_then(|s| s.split(']').next())
                    {
                        ahead = n.trim().parse().unwrap_or(0);
                    }
                }
                if extra.contains("behind") {
                    if let Some(n) = extra
                        .split("behind ")
                        .nth(1)
                        .and_then(|s| s.split(']').next())
                    {
                        behind = n.trim().parse().unwrap_or(0);
                    }
                }
            } else {
                branch = rest.to_string();
            }
            continue;
        }
        if line.is_empty() {
            continue;
        }

        let (index_raw, rest) = line.split_at(1);
        let (worktree_raw, path_part) = rest.split_at(1);
        let path = path_part.trim().to_string();

        let index_status = index_raw.chars().next().unwrap_or(' ');
        let worktree_status = worktree_raw.chars().next().unwrap_or(' ');

        let status_char = if index_status != ' ' {
            index_status
        } else {
            worktree_status
        };

        let file = GitFile {
            path,
            status: status_char,
            index_status,
            worktree_status,
        };

        if index_status != ' ' && index_status != '?' {
            staged.push(file.clone());
        }
        if worktree_status != ' ' && worktree_status != '?' {
            unstaged.push(file);
        }
        if index_status == '?' && worktree_status == '?' {
            untracked.push(GitFile {
                path: line[3..].to_string(),
                status: '?',
                index_status: '?',
                worktree_status: '?',
            });
        }
    }

    GitStatus {
        branch,
        ahead,
        behind,
        staged,
        unstaged,
        untracked,
    }
}

pub fn git_diff(path: &str) -> String {
    let (stdout, _stderr, _success) = run_git(&["diff", "--", path]);
    stdout
}

pub fn git_diff_staged() -> String {
    let (stdout, _stderr, _success) = run_git(&["diff", "--cached"]);
    stdout
}

pub fn git_commit(message: &str) -> Result<String, String> {
    let (stdout, stderr, success) = run_git(&["commit", "-m", message]);
    if success {
        Ok(stdout.trim().to_string())
    } else {
        Err(stderr.trim().to_string())
    }
}

#[derive(Debug, Clone)]
pub struct GitCommit {
    pub hash: String,
    pub author: String,
    pub date: String,
    pub message: String,
}

pub fn git_log(n: usize) -> Vec<GitCommit> {
    let n_str = n.to_string();
    let (stdout, _stderr, _success) =
        run_git(&["log", &format!("-{}", n_str), "--format=%H|%an|%ai|%s"]);
    let mut commits = Vec::new();
    for line in stdout.lines() {
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.splitn(4, '|').collect();
        if parts.len() >= 4 {
            commits.push(GitCommit {
                hash: parts[0].to_string(),
                author: parts[1].to_string(),
                date: parts[2].to_string(),
                message: parts[3].to_string(),
            });
        }
    }
    commits
}

pub fn git_branches() -> Vec<String> {
    let (stdout, _stderr, _success) = run_git(&["branch"]);
    stdout.lines().map(|l| l.trim().to_string()).collect()
}

pub fn git_checkout(branch: &str) -> Result<(), String> {
    let (stdout, stderr, success) = run_git(&["checkout", branch]);
    if success {
        Ok(())
    } else {
        let err = if stderr.is_empty() { stdout } else { stderr };
        Err(err.trim().to_string())
    }
}

fn run_gh(args: &[&str]) -> (String, String, bool) {
    match StdCommand::new("gh").args(args).output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let success = output.status.success();
            (stdout, stderr, success)
        }
        Err(e) => (String::new(), format!("Failed to run gh: {}", e), false),
    }
}

#[derive(Debug, Clone)]
pub struct GitHubIssue {
    pub number: u32,
    pub title: String,
    pub state: String,
    pub author: String,
    pub labels: Vec<String>,
    pub created_at: String,
}

pub fn github_list_issues(repo: &str) -> Vec<GitHubIssue> {
    let (stdout, _stderr, _success) = run_gh(&[
        "issue",
        "list",
        "--repo",
        repo,
        "--json",
        "number,title,state,author,labels,createdAt",
        "--limit",
        "50",
    ]);
    let mut issues = Vec::new();
    if let Ok(values) = serde_json::from_str::<Vec<serde_json::Value>>(&stdout) {
        for v in values {
            let labels: Vec<String> = v["labels"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|l| l["name"].as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            issues.push(GitHubIssue {
                number: v["number"].as_u64().unwrap_or(0) as u32,
                title: v["title"].as_str().unwrap_or("").to_string(),
                state: v["state"].as_str().unwrap_or("").to_string(),
                author: v["author"]["login"].as_str().unwrap_or("").to_string(),
                labels,
                created_at: v["createdAt"].as_str().unwrap_or("").to_string(),
            });
        }
    }
    issues
}

#[derive(Debug, Clone)]
pub struct GitHubPr {
    pub number: u32,
    pub title: String,
    pub state: String,
    pub author: String,
    pub branch: String,
    pub mergeable: bool,
}

pub fn github_list_prs(repo: &str) -> Vec<GitHubPr> {
    let (stdout, _stderr, _success) = run_gh(&[
        "pr",
        "list",
        "--repo",
        repo,
        "--json",
        "number,title,state,author,headRefName,mergeable",
        "--limit",
        "50",
    ]);
    let mut prs = Vec::new();
    if let Ok(values) = serde_json::from_str::<Vec<serde_json::Value>>(&stdout) {
        for v in values {
            prs.push(GitHubPr {
                number: v["number"].as_u64().unwrap_or(0) as u32,
                title: v["title"].as_str().unwrap_or("").to_string(),
                state: v["state"].as_str().unwrap_or("").to_string(),
                author: v["author"]["login"].as_str().unwrap_or("").to_string(),
                branch: v["headRefName"].as_str().unwrap_or("").to_string(),
                mergeable: v["mergeable"].as_bool().unwrap_or(false),
            });
        }
    }
    prs
}

pub fn github_create_issue(repo: &str, title: &str, body: &str) -> Result<String, String> {
    let (stdout, stderr, success) = run_gh(&[
        "issue", "create", "--repo", repo, "--title", title, "--body", body,
    ]);
    if success {
        Ok(stdout.trim().to_string())
    } else {
        Err(stderr.trim().to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitTab {
    Status,
    Diff,
    Log,
    Branches,
    Issues,
    PullRequests,
}

pub struct GitState {
    pub status: Option<GitStatus>,
    pub log: Vec<GitCommit>,
    pub issues: Vec<GitHubIssue>,
    pub prs: Vec<GitHubPr>,
    pub diff: String,
    pub selected_file: Option<usize>,
    pub tab: GitTab,
}

impl GitState {
    pub fn new() -> Self {
        GitState {
            status: None,
            log: Vec::new(),
            issues: Vec::new(),
            prs: Vec::new(),
            diff: String::new(),
            selected_file: None,
            tab: GitTab::Status,
        }
    }

    pub fn refresh(&mut self) {
        self.status = Some(git_status());
        self.log = git_log(20);
    }

    pub fn refresh_github(&mut self, repo: &str) {
        self.issues = github_list_issues(repo);
        self.prs = github_list_prs(repo);
    }
}
