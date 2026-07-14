//! cc@zang aka Gaurav-x111

use crate::app::*;
use crate::telemetry::{LogEntry, LogLevel};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use sysinfo::{Disks, Networks, System};

const PYTHON: &str = "python3";

/// Strip the common leading whitespace shared by every non-empty line of a
/// script. The `r#"..."#` collector scripts are written indented to match Rust
/// formatting; `python3 -c` rejects indented top-level code, so we dedent
/// before executing.
fn dedent(s: &str) -> String {
    let trimmed = s.strip_prefix('\n').unwrap_or(s);
    let min_indent = trimmed
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    trimmed
        .lines()
        .map(|l| {
            if l.trim().is_empty() {
                l.to_string()
            } else {
                l.get(min_indent..).unwrap_or(l).to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn workspace_root() -> PathBuf {
    // Try to find workspace root by walking up from CWD looking for src/ros2_fastfetch
    let mut dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    for _ in 0..10 {
        if dir.join("src").join("ros2_fastfetch").exists() {
            return dir;
        }
        if !dir.pop() {
            break;
        }
    }
    // Fallback: try relative to the binary
    if let Ok(exe) = std::env::current_exe() {
        let mut dir = exe.parent().unwrap_or(&dir).to_path_buf();
        for _ in 0..10 {
            if dir.join("src").join("ros2_fastfetch").exists() {
                return dir;
            }
            if !dir.pop() {
                break;
            }
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn run_python(script: &str) -> Result<String, String> {
    let ws = workspace_root();
    let script = dedent(script);
    let output = Command::new(PYTHON)
        .args(["-c", &script])
        .current_dir(&ws)
        .envs(std::env::vars())
        .output()
        .map_err(|e| format!("Failed to run Python: {e}"))?;
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if stdout.is_empty() {
            Err("Empty output from Python".to_string())
        } else {
            Ok(stdout)
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("Python error: {stderr}"))
    }
}

fn collect_ros2_script() -> &'static str {
    r#"
 import json, sys, os
 sys.path.insert(0, 'src/ros2_fastfetch')
 try:
     from fetch_info.collector.ros2 import collect_all
     d = collect_all(check_live=True, live_timeout=5, check_updates=False)
     topics = [(t.get('name', ''), t.get('type', '')) for t in d.get('topics', [])]
     # Prefer env var override, fall back to collector-detected value
     rmw = os.environ.get('RMW_IMPLEMENTATION', '') or d.get('rmw', '')
     sys.stdout.write(json.dumps({
         'distro': d.get('distro', ''),
         'domain_id': d.get('domain_id', ''),
         'dds': d.get('dds', ''),
         'rmw': rmw,
         'nodes': d.get('nodes', []),
         'topics': topics,
         'services': d.get('services', []),
         'actions': d.get('actions', []),
     }))
 except Exception as e:
     sys.stdout.write(json.dumps({'error': str(e), 'nodes': [], 'topics': [], 'services': [], 'actions': [], 'distro': '', 'domain_id': '', 'dds': '', 'rmw': ''}))
 "#
}

fn collect_workspace_script() -> &'static str {
    r#"
import json, sys
sys.path.insert(0, 'src/ros2_fastfetch')
try:
    from fetch_info.collector.workspace import collect_all
    d = collect_all()
    sys.stdout.write(json.dumps({
        'workspaces': d.get('workspaces', []),
        'packages': len(d.get('packages', [])),
        'built_packages': len(d.get('built_packages', [])),
        'modified_packages': d.get('modified_packages', []),
        'launch_count': d.get('launch_count', 0),
    }))
except Exception as e:
    sys.stdout.write(json.dumps({'error': str(e)}))
"#
}

fn collect_diagnostics_script() -> &'static str {
    r#"
 import json, sys
 sys.path.insert(0, 'src/ros2_fastfetch')
 try:
     from fetch_info.collector.diagnostics import run_diagnostics
     data = run_diagnostics()
     checks = data.get('checks', []) if isinstance(data, dict) else []
     result = []
     for c in checks:
         status = c.get('status', 'info')
         sev = 'error' if status == 'fail' else ('warn' if status == 'warn' else 'info')
         details = {'detail': c.get('detail', '')}
         if c.get('fix'):
             details['fix'] = c.get('fix')
         result.append({'severity': sev, 'message': c.get('name', ''), 'details': details})
     sys.stdout.write(json.dumps({'issues': result}))
 except Exception as e:
     sys.stdout.write(json.dumps({'error': str(e)}))
 "#
}

fn collect_trends_script() -> &'static str {
    r#"
import json, sys
sys.path.insert(0, 'src/ros2_fastfetch')
try:
    from fetch_info.collector.trends import get_summary
    d = get_summary()
    sys.stdout.write(json.dumps(d))
except Exception as e:
    sys.stdout.write(json.dumps({'error': str(e)}))
"#
}

/// Build a minimal `Fleet` view: the local machine as a single host, decorated
/// with the live ROS 2 node count and distro when available. This makes the
/// Fleet tab show real data instead of "No fleet data". Multi-host discovery
/// (parsing a fleet config / probing remote hosts over SSH) can extend this
/// later.
fn build_fleet(ros: Option<&Ros2Data>) -> FleetData {
    let hostname = std::env::var("HOSTNAME").unwrap_or_else(|_| "localhost".to_string());
    let host = FleetHost {
        hostname: hostname.clone(),
        ip: "127.0.0.1".to_string(),
        reachable: true,
        uptime: None,
        memory: None,
        disk: None,
        ros2_nodes: ros.map(|r| r.nodes.len()),
        ros_distro: ros.and_then(|r| {
            if r.distro.is_empty() {
                None
            } else {
                Some(r.distro.clone())
            }
        }),
    };
    FleetData { hosts: vec![host] }
}

fn collect_graph_script() -> &'static str {
    r#"
 import json, sys
 sys.path.insert(0, 'src/ros2_fastfetch')
try:
    from fetch_info.terminal import build_topic_graph
    g = build_topic_graph(timeout=5)
    sys.stdout.write(json.dumps(g))
except Exception as e:
    sys.stdout.write(json.dumps({'nodes': {}, 'topics': {}}))
"#
}

/// Persistent state for the system-stats collector. Held across poll ticks
/// so sysinfo sees two CPU samples (the first refresh always reads 0%) and
/// network counters become a rate instead of an ever-growing since-boot total.
struct SysState {
    sys: System,
    disks: Disks,
    networks: Networks,
    last_net_sent: u64,
    last_net_recv: u64,
    last_net_at: Instant,
}

impl SysState {
    fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_cpu_all();
        sys.refresh_memory();
        let disks = Disks::new_with_refreshed_list();
        let networks = Networks::new_with_refreshed_list();
        let (s, r) = total_net(&networks);
        Self {
            sys,
            disks,
            networks,
            last_net_sent: s,
            last_net_recv: r,
            last_net_at: Instant::now(),
        }
    }

    fn sample(&mut self) -> SystemData {
        self.sys.refresh_cpu_all();
        self.sys.refresh_memory();
        self.disks.refresh(true);
        self.networks.refresh(true);

        let cpu_percent = self.sys.global_cpu_usage();
        let mem_total = self.sys.total_memory();
        let mem_used = self.sys.used_memory();
        let mem_percent = if mem_total > 0 {
            (mem_used as f64 / mem_total as f64 * 100.0) as f32
        } else {
            0.0
        };

        let disk_percent = self
            .disks
            .iter()
            .find(|d| d.mount_point() == std::path::Path::new("/"))
            .map(|d| {
                let total = d.total_space();
                if total > 0 {
                    (total - d.available_space()) as f64 / total as f64 * 100.0
                } else {
                    0.0
                }
            })
            .unwrap_or(0.0);

        // ponytail: convert cumulative since-boot counters to a rate (MB/s)
        // by differencing against the previous sample. The old code divided
        // the raw total by 1e6, so the "rate" only ever went up.
        let (now_sent, now_recv) = total_net(&self.networks);
        let elapsed = self.last_net_at.elapsed().as_secs_f64().max(0.001);
        let net_sent_mb = (now_sent.saturating_sub(self.last_net_sent) as f64 / 1e6) / elapsed;
        let net_recv_mb = (now_recv.saturating_sub(self.last_net_recv) as f64 / 1e6) / elapsed;
        self.last_net_sent = now_sent;
        self.last_net_recv = now_recv;
        self.last_net_at = Instant::now();

        let uptime_secs = System::uptime();
        let cpu_cores = self.sys.cpus().len();
        let cpu_freq = self.sys.cpus().first().map(|c| c.frequency()).unwrap_or(0) as f64;
        let hostname = System::name().unwrap_or_default();
        let os_name = System::long_os_version().unwrap_or_default();
        let kernel = System::kernel_version().unwrap_or_default();

        SystemData {
            hostname,
            os_name,
            kernel,
            uptime_secs,
            cpu_percent,
            cpu_cores,
            cpu_freq,
            mem_percent,
            mem_used_gb: mem_used as f64 / 1e9,
            mem_total_gb: mem_total as f64 / 1e9,
            disk_percent,
            net_sent_mb,
            net_recv_mb,
            gpu_info: String::new(),
            battery_percent: None,
            temperatures: vec![],
        }
    }
}

fn total_net(networks: &Networks) -> (u64, u64) {
    let mut sent = 0u64;
    let mut recv = 0u64;
    for n in networks.iter() {
        sent += n.1.total_transmitted();
        recv += n.1.total_received();
    }
    (sent, recv)
}

fn parse_ros2(json_str: &str) -> Ros2Data {
    match serde_json::from_str::<serde_json::Value>(json_str) {
        Ok(v) => {
            if v.get("error").and_then(|e| e.as_str()).is_some() {
                return Ros2Data {
                    error: v["error"].as_str().map(String::from),
                    ..Default::default()
                };
            }
            Ros2Data {
                distro: v["distro"].as_str().unwrap_or("").to_string(),
                domain_id: v["domain_id"].as_str().unwrap_or("").to_string(),
                dds: v["dds"].as_str().unwrap_or("").to_string(),
                rmw: v["rmw"].as_str().unwrap_or("").to_string(),
                nodes: v["nodes"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                topics: v["topics"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|t| {
                                let name = t[0].as_str().unwrap_or("");
                                let r#type = t[1].as_str().unwrap_or("");
                                if name.is_empty() {
                                    None
                                } else {
                                    Some((name.to_string(), r#type.to_string()))
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                services: v["services"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                actions: v["actions"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|x| x.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                error: None,
            }
        }
        Err(e) => Ros2Data {
            error: Some(format!("JSON parse: {e}")),
            ..Default::default()
        },
    }
}

fn parse_workspace(json_str: &str) -> WorkspaceData {
    match serde_json::from_str::<serde_json::Value>(json_str) {
        Ok(v) => WorkspaceData {
            workspaces: v["workspaces"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            packages: v["packages"].as_u64().unwrap_or(0) as usize,
            built_packages: v["built_packages"].as_u64().unwrap_or(0) as usize,
            modified_packages: v["modified_packages"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            launch_count: v["launch_count"].as_u64().unwrap_or(0) as usize,
        },
        Err(_e) => WorkspaceData {
            ..Default::default()
        },
    }
}

fn parse_diagnostics(json_str: &str) -> DiagnosticsData {
    match serde_json::from_str::<serde_json::Value>(json_str) {
        Ok(v) => {
            let issues = v["issues"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|i| DiagnosticIssue {
                            severity: i["severity"].as_str().unwrap_or("info").to_string(),
                            message: i["message"].as_str().unwrap_or("").to_string(),
                            details: i["details"]
                                .as_object()
                                .map(|o| {
                                    o.iter()
                                        .map(|(k, v)| {
                                            (k.clone(), v.as_str().unwrap_or("").to_string())
                                        })
                                        .collect()
                                })
                                .unwrap_or_default(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            DiagnosticsData { issues }
        }
        Err(_e) => DiagnosticsData {
            ..Default::default()
        },
    }
}

fn parse_trends(json_str: &str) -> TrendsData {
    match serde_json::from_str::<serde_json::Value>(json_str) {
        Ok(v) => {
            let mut summary = HashMap::new();
            if let Some(obj) = v.as_object() {
                for (k, val) in obj {
                    if let Some(n) = val.as_f64() {
                        summary.insert(k.clone(), n);
                    }
                }
            }
            TrendsData { summary }
        }
        Err(_e) => TrendsData {
            ..Default::default()
        },
    }
}

fn parse_graph(json_str: &str) -> GraphData {
    match serde_json::from_str::<serde_json::Value>(json_str) {
        Ok(v) => {
            let mut nodes = HashMap::new();
            if let Some(obj) = v["nodes"].as_object() {
                for (name, nd) in obj {
                    nodes.insert(
                        name.clone(),
                        GraphNode {
                            pubs: nd["pubs"]
                                .as_array()
                                .map(|a| {
                                    a.iter()
                                        .filter_map(|x| x.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default(),
                            subs: nd["subs"]
                                .as_array()
                                .map(|a| {
                                    a.iter()
                                        .filter_map(|x| x.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default(),
                        },
                    );
                }
            }
            GraphData { nodes }
        }
        Err(_) => GraphData::default(),
    }
}

pub fn run_background_collection(tx: mpsc::Sender<DataEvent>) {
    thread::spawn(move || {
        let mut sys_state = SysState::new();
        let mut last_sys = Instant::now();
        let mut last_ros2 = Instant::now();
        let mut last_ws = Instant::now();
        let mut last_diag = Instant::now();
        let mut last_graph = Instant::now();
        let mut last_telem = Instant::now();
        let mut last_git = Instant::now();
        // Track which (file, byte-offset) we've already sent so the ring buffer
        // isn't refilled with the same lines every 3s.
        let mut log_cursor: HashMap<PathBuf, u64> = HashMap::new();
        // Ponytail: also cap the number of unseen lines per poll to avoid a
        // flood when a fresh run dir appears with many log files.
        const MAX_NEW_LINES: usize = 200;

        loop {
            let now = Instant::now();

            if now.duration_since(last_sys) >= Duration::from_secs(2) {
                let data = sys_state.sample();
                let _ = tx.send(DataEvent::System(data));
                last_sys = now;
            }

            if now.duration_since(last_ros2) >= Duration::from_secs(5) {
                match run_python(collect_ros2_script()) {
                    Ok(out) => {
                        let ros = parse_ros2(&out);
                        let _ = tx.send(DataEvent::Ros2(ros.clone()));
                        let _ = tx.send(DataEvent::Fleet(build_fleet(Some(&ros))));
                    }
                    Err(e) => {
                        let ros = Ros2Data {
                            error: Some(e.clone()),
                            ..Default::default()
                        };
                        let _ = tx.send(DataEvent::Ros2(ros.clone()));
                        let _ = tx.send(DataEvent::Fleet(build_fleet(None)));
                    }
                }
                last_ros2 = now;
            }

            if now.duration_since(last_ws) >= Duration::from_secs(10) {
                if let Ok(out) = run_python(collect_workspace_script()) {
                    let _ = tx.send(DataEvent::Workspace(parse_workspace(&out)));
                }
                if let Ok(out) = run_python(collect_trends_script()) {
                    let _ = tx.send(DataEvent::Trends(parse_trends(&out)));
                }
                last_ws = now;
            }

            if now.duration_since(last_diag) >= Duration::from_secs(15) {
                if let Ok(out) = run_python(collect_diagnostics_script()) {
                    let _ = tx.send(DataEvent::Diagnostics(parse_diagnostics(&out)));
                }
                last_diag = now;
            }

            if now.duration_since(last_graph) >= Duration::from_secs(10) {
                if let Ok(out) = run_python(collect_graph_script()) {
                    let _ = tx.send(DataEvent::Graph(parse_graph(&out)));
                }
                last_graph = now;
            }

            // Telemetry: scan ROS 2 log directory every 3 seconds
            if now.duration_since(last_telem) >= Duration::from_secs(3) {
                let entries = scan_ros2_logs(&mut log_cursor, MAX_NEW_LINES);
                if !entries.is_empty() {
                    let _ = tx.send(DataEvent::Telemetry(entries));
                }
                last_telem = now;
            }

            // Git status + GitHub issues/PRs. Done here (off the main thread)
            // so a slow `gh` network call can never freeze the UI.
            if now.duration_since(last_git) >= Duration::from_secs(5) {
                let mut g = crate::git::GitState::new();
                g.refresh();
                if let Some(repo) = crate::app::App::current_github_repo() {
                    g.refresh_github(&repo);
                }
                let _ = tx.send(DataEvent::Git(g));
                last_git = now;
            }

            thread::sleep(Duration::from_millis(500));
        }
    });
}

/// Scan the ROS 2 log directory for recent log entries, only emitting lines
/// we haven't already sent. `cursor` maps each log file to the byte offset we
/// last read up to; on a new run dir we drop stale entries so it starts fresh.
pub fn scan_ros2_logs(cursor: &mut HashMap<PathBuf, u64>, max_new: usize) -> Vec<LogEntry> {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let log_dir = PathBuf::from(&home).join(".ros").join("log");

    if !log_dir.exists() {
        return Vec::new();
    }

    // Find the most recent run directory
    let Ok(entries) = fs::read_dir(&log_dir) else {
        return Vec::new();
    };

    let mut run_dirs: Vec<_> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();

    run_dirs.sort_by(|a, b| {
        b.metadata()
            .and_then(|m| m.modified())
            .ok()
            .cmp(&a.metadata().and_then(|m| m.modified()).ok())
    });

    if run_dirs.is_empty() {
        return Vec::new();
    }

    let run_path = run_dirs[0].path();

    // If the active run dir changed, prune cursor entries that aren't under
    // the new run dir — the old files are no longer being tailed.
    cursor.retain(|p, _| p.starts_with(&run_path));

    let Ok(log_entries) = fs::read_dir(&run_path) else {
        return Vec::new();
    };

    let mut log_files: Vec<_> = log_entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "log"))
        .collect();

    log_files.sort_by(|a, b| {
        b.metadata()
            .and_then(|m| m.modified())
            .ok()
            .cmp(&a.metadata().and_then(|m| m.modified()).ok())
    });

    let mut results = Vec::new();

    for log_file in log_files.iter().take(5) {
        if results.len() >= max_new {
            break;
        }
        let node_name = log_file.file_name().to_string_lossy().to_string();
        let path = log_file.path();

        // Read only the bytes past our last cursor offset.
        let Ok(bytes) = fs::read(&path) else { continue };
        let prev = *cursor.get(&path).unwrap_or(&0);
        let start = if (prev as usize) <= bytes.len() {
            prev as usize
        } else {
            0
        };
        let chunk = &bytes[start..];
        if chunk.is_empty() {
            cursor.insert(path, bytes.len() as u64);
            continue;
        }
        let text = String::from_utf8_lossy(chunk);
        // ponytail: take the last 20 lines of the new chunk so a burst from
        // one file doesn't drown out the others; the cursor advance still
        // marks everything seen.
        for line in text.lines().rev().take(20) {
            if let Some(entry) = parse_log_line(line, &node_name) {
                results.push(entry);
            }
        }
        cursor.insert(path, bytes.len() as u64);
    }

    // Newest line first in the chunk, but the UI ring buffer adds in order;
    // reverse the per-file-reversed collection so older new lines come first.
    results.reverse();
    results
}

/// Parse a single log line into a LogEntry
fn parse_log_line(line: &str, default_node: &str) -> Option<LogEntry> {
    if line.is_empty() {
        return None;
    }

    // ROS 2 log format: [severity] [time] [node]: message
    // Or fallback: severity timestamp [node] message
    let (level, rest) = if line.contains("[ERROR]") || line.contains("ERROR") {
        (LogLevel::Error, line)
    } else if line.contains("[WARN]") || line.contains("WARN") {
        (LogLevel::Warn, line)
    } else if line.contains("DEBUG") {
        (LogLevel::Debug, line)
    } else if line.contains("FATAL") {
        (LogLevel::Fatal, line)
    } else {
        (LogLevel::Info, line)
    };

    Some(LogEntry {
        timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
        node: default_node.to_string(),
        level,
        message: rest.to_string(),
    })
}
