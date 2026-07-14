use axum::{
    body::Body,
    extract::Path,
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::State,
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use sysinfo::{Disks, Networks, System};
use tokio::sync::Mutex;
use tokio::time::interval;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tracing::{error, info};

const PYTHON_COLLECTOR: &str = "python3";

#[derive(Clone)]
struct AppState {
    sys: Arc<Mutex<System>>,
    disks: Arc<Mutex<Disks>>,
    networks: Arc<Mutex<Networks>>,
}

#[derive(Serialize)]
struct SystemStats {
    timestamp: f64,
    cpu_percent: f32,
    mem_percent: f32,
    mem_used_gb: f64,
    mem_total_gb: f64,
    disk_percent: f64,
    net_sent_mb: f64,
    net_recv_mb: f64,
    uptime_seconds: u64,
}

#[derive(Serialize, Deserialize, Default, PartialEq)]
struct Ros2Info {
    distro: String,
    domain_id: String,
    dds: String,
    nodes: Vec<String>,
    topics: Vec<TopicEntry>,
    services: Vec<String>,
    actions: Vec<String>,
    #[serde(default)]
    node_count: usize,
    #[serde(default)]
    topic_count: usize,
    #[serde(default)]
    service_count: usize,
    #[serde(default)]
    action_count: usize,
    error: Option<String>,
}

#[derive(Serialize)]
struct FullInfo {
    system: serde_json::Value,
    ros2: Ros2Info,
    workspace: serde_json::Value,
}

#[derive(Serialize, Deserialize, Default, PartialEq, Clone)]
struct TopicEntry {
    name: String,
    #[serde(rename = "type")]
    r#type: String,
}

#[derive(Serialize, Deserialize)]
struct GraphData {
    nodes: HashMap<String, GraphNode>,
    topics: HashMap<String, TopicConns>,
}

#[derive(Serialize, Deserialize)]
struct GraphNode {
    pubs: Vec<String>,
    subs: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct TopicConns {
    publishers: Vec<String>,
    subscribers: Vec<String>,
}

#[derive(Deserialize)]
struct ExecRequest {
    cmd: String,
}

#[derive(Serialize)]
struct ExecResponse {
    output: String,
}

#[derive(Serialize)]
struct WsMessage {
    r#type: String,
    data: serde_json::Value,
}

fn collect_system_stats(sys: &mut System, disks: &mut Disks, networks: &mut Networks) -> SystemStats {
    sys.refresh_cpu_all();
    sys.refresh_memory();
    disks.refresh(true);
    networks.refresh(true);

    let cpu_percent = sys.global_cpu_usage();
    let mem_percent: f32 = if sys.total_memory() > 0 {
        (sys.used_memory() as f64 / sys.total_memory() as f64 * 100.0) as f32
    } else {
        0.0
    };
    let mem_used_gb = sys.used_memory() as f64 / 1e9;
    let mem_total_gb = sys.total_memory() as f64 / 1e9;

    let disk_percent = disks
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

    let (net_sent_mb, net_recv_mb) = {
        let mut sent = 0u64;
        let mut recv = 0u64;
        for n in networks.iter() {
            sent += n.1.total_transmitted();
            recv += n.1.total_received();
        }
        (sent as f64 / 1e6, recv as f64 / 1e6)
    };

    let uptime_seconds = System::uptime();

    SystemStats {
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64(),
        cpu_percent,
        mem_percent: (mem_percent * 10.0).round() / 10.0,
        mem_used_gb: (mem_used_gb * 10.0).round() / 10.0,
        mem_total_gb: (mem_total_gb * 10.0).round() / 10.0,
        disk_percent: (disk_percent * 10.0).round() / 10.0,
        net_sent_mb: (net_sent_mb * 10.0).round() / 10.0,
        net_recv_mb: (net_recv_mb * 10.0).round() / 10.0,
        uptime_seconds,
    }
}

fn find_project_root() -> String {
    let candidates = vec![
        "src/ros2_fastfetch".to_string(),
        "../src/ros2_fastfetch".to_string(),
        "../../src/ros2_fastfetch".to_string(),
        "src/backend/../../src/ros2_fastfetch".to_string(),
    ];
    for c in &candidates {
        if std::path::Path::new(c).join("fetch_info").exists() {
            // Return the absolute path of the src/ros2_fastfetch dir
            return std::fs::canonicalize(c)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| c.clone());
        }
    }
    // Fallback: assume relative to the binary
    "src/ros2_fastfetch".to_string()
}

fn run_python_script(script: &str) -> Result<String, String> {
    let project_root = find_project_root();
    let wrapped_script = format!(
        "import sys\nsys.path.insert(0, '{}')\n{}",
        project_root.replace('\\', "\\\\"),
        script
    );
    let output = std::process::Command::new(PYTHON_COLLECTOR)
        .args(["-c", &wrapped_script])
        .envs(std::env::vars())
        .output()
        .map_err(|e| format!("Failed to run Python: {e}"))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("Python error: {stderr}"))
    }
}

fn collect_ros2_info() -> Ros2Info {
    let script = r#"
import json, sys
try:
    from fetch_info.collector import ros2
    d = ros2.collect_all(check_live=True, live_timeout=5, check_updates=False)
    sys.stdout.write(json.dumps({
        'distro': d.get('distro', 'unknown'),
        'domain_id': d.get('domain_id', '?'),
        'dds': d.get('dds', '?'),
        'nodes': d.get('nodes', []),
        'topics': d.get('topics', []),
        'services': d.get('services', []),
        'actions': d.get('actions', []),
    }))
except Exception as e:
    sys.stdout.write(json.dumps({'error': str(e), 'nodes': [], 'topics': [], 'services': [], 'actions': [], 'distro': '', 'domain_id': '', 'dds': ''}))
"#;

    match run_python_script(script) {
        Ok(json_str) => {
            match serde_json::from_str::<Ros2Info>(&json_str) {
                Ok(mut info) => {
                    info.node_count = info.nodes.len();
                    info.topic_count = info.topics.len();
                    info.service_count = info.services.len();
                    info.action_count = info.actions.len();
                    info
                }
                Err(e) => {
                    error!("Failed to parse ROS2 JSON: {e} — raw: {json_str:.200}");
                    Ros2Info {
                        error: Some(format!("Parse error: {e}")),
                        ..Default::default()
                    }
                }
            }
        }
        Err(e) => {
            error!("Failed to collect ROS2 info: {e}");
            Ros2Info {
                error: Some(e),
                ..Default::default()
            }
        }
    }
}

fn collect_full_info() -> FullInfo {
    let script = r#"
import json, sys
try:
    from fetch_info.collector import system, ros2, workspace
    data = {
        'system': system.collect_all(),
        'ros2_raw': ros2.collect_all(check_live=True, live_timeout=5, check_updates=False),
        'workspace': workspace.collect_all(),
    }
    sys.stdout.write(json.dumps(data))
except Exception as e:
    sys.stdout.write(json.dumps({'system': {}, 'ros2_raw': {'error': str(e), 'nodes': [], 'topics': [], 'services': [], 'actions': [], 'distro': '', 'domain_id': '', 'dds': ''}, 'workspace': {'workspaces': [], 'count': 0}}))
"#;

    match run_python_script(script) {
        Ok(json_str) => {
            match serde_json::from_str::<serde_json::Value>(&json_str) {
                Ok(val) => {
                    let system_data = val.get("system").cloned().unwrap_or(serde_json::json!({}));
                    let workspace_data = val.get("workspace").cloned().unwrap_or(serde_json::json!({"workspaces": [], "count": 0}));
                    let ros2_raw = val.get("ros2_raw").cloned().unwrap_or(serde_json::json!({}));

                    let ros2_info: Ros2Info = serde_json::from_value(serde_json::json!({
                        "distro": ros2_raw.get("distro").and_then(|v| v.as_str()).unwrap_or(""),
                        "domain_id": ros2_raw.get("domain_id").and_then(|v| v.as_str()).unwrap_or(""),
                        "dds": ros2_raw.get("dds").and_then(|v| v.as_str()).unwrap_or(""),
                        "nodes": ros2_raw.get("nodes").cloned().unwrap_or(serde_json::json!([])),
                        "topics": ros2_raw.get("topics").cloned().unwrap_or(serde_json::json!([])),
                        "services": ros2_raw.get("services").cloned().unwrap_or(serde_json::json!([])),
                        "actions": ros2_raw.get("actions").cloned().unwrap_or(serde_json::json!([])),
                    })).unwrap_or_default();

                    let mut ros2_final = ros2_info;
                    ros2_final.node_count = ros2_final.nodes.len();
                    ros2_final.topic_count = ros2_final.topics.len();
                    ros2_final.service_count = ros2_final.services.len();
                    ros2_final.action_count = ros2_final.actions.len();

                    FullInfo {
                        system: system_data,
                        ros2: ros2_final,
                        workspace: workspace_data,
                    }
                }
                Err(e) => {
                    error!("Failed to parse full info JSON: {e}");
                    FullInfo {
                        system: serde_json::json!({}),
                        ros2: Ros2Info {
                            error: Some(format!("Parse error: {e}")),
                            ..Default::default()
                        },
                        workspace: serde_json::json!({"workspaces": [], "count": 0}),
                    }
                }
            }
        }
        Err(e) => {
            error!("Failed to collect full info: {e}");
            FullInfo {
                system: serde_json::json!({}),
                ros2: Ros2Info {
                    error: Some(e),
                    ..Default::default()
                },
                workspace: serde_json::json!({"workspaces": [], "count": 0}),
            }
        }
    }
}

fn collect_graph_data() -> GraphData {
    let script = r#"
import json, sys
try:
    from fetch_info.terminal import build_topic_graph
    g = build_topic_graph(timeout=5)
    sys.stdout.write(json.dumps(g))
except Exception as e:
    sys.stdout.write(json.dumps({'nodes': {}, 'topics': {}}))
"#;

    match run_python_script(script) {
        Ok(json_str) => serde_json::from_str(&json_str).unwrap_or(GraphData {
            nodes: HashMap::new(),
            topics: HashMap::new(),
        }),
        Err(e) => {
            error!("Graph collection failed: {e}");
            GraphData {
                nodes: HashMap::new(),
                topics: HashMap::new(),
            }
        }
    }
}

fn find_template_dir() -> String {
    let candidates = vec![
        "src/ros2_fastfetch/fetch_info/templates/".to_string(),
        "../fetch_info/templates/".to_string(),
        "templates/".to_string(),
    ];
    for c in &candidates {
        if std::path::Path::new(c).join("index.html").exists() {
            return c.clone();
        }
    }
    "templates/".to_string()
}

async fn handle_ws(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let mut tick = interval(Duration::from_secs(1));
    let mut ros2_tick = interval(Duration::from_secs(5));

    loop {
        tokio::select! {
            _ = tick.tick() => {
                let mut sys = state.sys.lock().await;
                let mut disks = state.disks.lock().await;
                let mut nets = state.networks.lock().await;
                let stats = collect_system_stats(&mut sys, &mut disks, &mut nets);
                let msg = WsMessage {
                    r#type: "system".to_string(),
                    data: serde_json::to_value(&stats).unwrap_or_default(),
                };
                if let Ok(text) = serde_json::to_string(&msg) {
                    if socket.send(Message::Text(text)).await.is_err() {
                        break;
                    }
                }
            }
            _ = ros2_tick.tick() => {
                let ros2 = tokio::task::spawn_blocking(collect_ros2_info).await.unwrap_or_default();
                let msg = WsMessage {
                    r#type: "ros2".to_string(),
                    data: serde_json::to_value(&ros2).unwrap_or_default(),
                };
                if let Ok(text) = serde_json::to_string(&msg) {
                    if socket.send(Message::Text(text)).await.is_err() {
                        break;
                    }
                }
            }
            _ = socket.recv() => {
                break;
            }
        }
    }
}

async fn api_status(State(state): State<AppState>) -> Json<SystemStats> {
    let mut sys = state.sys.lock().await;
    let mut disks = state.disks.lock().await;
    let mut nets = state.networks.lock().await;
    Json(collect_system_stats(&mut sys, &mut disks, &mut nets))
}

async fn api_info() -> Json<FullInfo> {
    Json(tokio::task::spawn_blocking(collect_full_info)
        .await
        .unwrap_or(FullInfo {
            system: serde_json::json!({}),
            ros2: Ros2Info::default(),
            workspace: serde_json::json!({"workspaces": [], "count": 0}),
        }))
}

async fn api_graph() -> Json<GraphData> {
    Json(tokio::task::spawn_blocking(collect_graph_data)
        .await
        .unwrap_or(GraphData {
            nodes: HashMap::new(),
            topics: HashMap::new(),
        }))
}

async fn api_exec(Json(req): Json<ExecRequest>) -> Json<ExecResponse> {
    let cmd = req.cmd.trim().to_lowercase();

    let ros_cmd: Option<Vec<&str>> = match cmd.as_str() {
        "nodes" => Some(vec!["ros2", "node", "list"]),
        "topics" => Some(vec!["ros2", "topic", "list", "-t"]),
        "services" => Some(vec!["ros2", "service", "list"]),
        "actions" => Some(vec!["ros2", "action", "list"]),
        "env" => {
            return Json(ExecResponse {
                output: std::env::vars()
                    .filter(|(k, _)| {
                        k.starts_with("ROS")
                            || k.starts_with("AMENT")
                            || k.starts_with("COLCON")
                            || k.starts_with("RMW")
                    })
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
            });
        }
        _ if cmd.starts_with("node info ") => {
            Some(vec!["ros2", "node", "info", cmd.trim_start_matches("node info ").trim()])
        }
        _ if cmd.starts_with("param list") => Some(vec!["ros2", "param", "list"]),
        _ if cmd.starts_with("bag info ") => {
            Some(vec!["ros2", "bag", "info", cmd.trim_start_matches("bag info ").trim()])
        }
        _ if cmd.starts_with("interface show ") => {
            Some(vec!["ros2", "interface", "show", cmd.trim_start_matches("interface show ").trim()])
        }
        _ => {
            return Json(ExecResponse {
                output: format!(
                    "Command '{cmd}' not supported in web terminal.\n\
                     Supported: nodes, topics, services, actions, env, node info <n>, param list, bag info <f>, interface show <t>"
                ),
            });
        }
    };

    match ros_cmd {
        Some(args) => {
            let output = std::process::Command::new(args[0])
                .args(&args[1..])
                .envs(std::env::vars())
                .output();

            match output {
                Ok(out) => {
                    let text = if !out.stdout.is_empty() {
                        String::from_utf8_lossy(&out.stdout).trim().to_string()
                    } else if !out.stderr.is_empty() {
                        String::from_utf8_lossy(&out.stderr).trim().to_string()
                    } else {
                        "(no output)".to_string()
                    };
                    Json(ExecResponse { output: text })
                }
                Err(e) => Json(ExecResponse {
                    output: format!("Error: {e}"),
                }),
            }
        }
        None => Json(ExecResponse {
            output: "No command to execute".to_string(),
        }),
    }
}

fn collect_python(python_code: &str) -> Result<String, String> {
    let script = format!(
        r#"
import json, sys
try:
    {}
    sys.stdout.write(json.dumps(result))
except Exception as e:
    sys.stdout.write(json.dumps({{'error': str(e)}}))
"#,
        python_code
    );
    run_python_script(&script)
}

async fn api_trend() -> Json<serde_json::Value> {
    let code = r#"
from fetch_info.collector.trends import record_snapshot, get_summary, get_trend
from fetch_info.collector import system, ros2
sys_d = system.collect_all()
ros2_d = ros2.collect_all(check_live=True, live_timeout=3, check_updates=False)
mem = sys_d.get('memory', {})
bat = sys_d.get('battery', {})
record_snapshot(
    cpu_percent=sys_d.get('cpu', {}).get('freq_mhz', 0) or 0,
    memory_percent=mem.get('percent', 0),
    disk_percent=sys_d.get('disk', {}).get('percent', 0),
    battery_percent=bat.get('percent'),
    node_count=len(ros2_d.get('nodes', [])),
    topic_count=len(ros2_d.get('topics', [])),
    service_count=len(ros2_d.get('services', [])),
)
result = get_summary()
result['snapshot'] = 'recorded'
"#;
    match collect_python(code) {
        Ok(json_str) => Json(serde_json::from_str(&json_str).unwrap_or(serde_json::json!({"error": "parse error"}))),
        Err(e) => Json(serde_json::json!({"error": e})),
    }
}

async fn api_launch_verify(Json(req): Json<serde_json::Value>) -> Json<serde_json::Value> {
    let path = req.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let code = format!(
        r#"
from fetch_info.collector.launch_verify import verify_launch_file, verify_workspace_launch_files
import os
if os.path.isdir('{path}'):
    result = verify_workspace_launch_files('{path}')
else:
    result = verify_launch_file('{path}')
"#
    );
    match collect_python(&code) {
        Ok(json_str) => Json(serde_json::from_str(&json_str).unwrap_or(serde_json::json!({"error": "parse error"}))),
        Err(e) => Json(serde_json::json!({"error": e})),
    }
}

async fn api_bag_analyze(Json(req): Json<serde_json::Value>) -> Json<serde_json::Value> {
    let path = req.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let code = format!(
        r#"
from fetch_info.collector.bag_forensics import analyze_bag, check_bag_health, get_topic_timeline
result = {{
    'info': analyze_bag('{path}'),
    'health': check_bag_health('{path}'),
    'timeline': get_topic_timeline('{path}'),
}}
"#
    );
    match collect_python(&code) {
        Ok(json_str) => Json(serde_json::from_str(&json_str).unwrap_or(serde_json::json!({"error": "parse error"}))),
        Err(e) => Json(serde_json::json!({"error": e})),
    }
}

async fn api_fleet(Json(req): Json<serde_json::Value>) -> Json<serde_json::Value> {
    let hosts: Vec<String> = req.get("hosts")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let hosts_str = hosts.iter().map(|h| format!("'{}'", h)).collect::<Vec<_>>().join(", ");
    let code = format!(
        r#"
from fetch_info.collector.fleet import FleetHost, collect_fleet
hosts_list = [FleetHost(hostname=h, ip=h, username='root') for h in [{hosts_str}]]
raw = collect_fleet(hosts_list)
result = []
for r in raw:
    result.append({{
        'hostname': r.get('hostname'),
        'ip': r.get('ip'),
        'reachable': r.get('reachable', False),
        'uptime': r.get('uptime'),
        'memory': r.get('memory'),
        'disk': r.get('disk'),
        'ros2_nodes': r.get('ros2_nodes'),
        'ros_distro': r.get('ros_distro'),
    }})
"#
    );
    match collect_python(&code) {
        Ok(json_str) => Json(serde_json::from_str(&json_str).unwrap_or(serde_json::json!({"error": "parse error"}))),
        Err(e) => Json(serde_json::json!({"error": e})),
    }
}

async fn auth_middleware(req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    let username = std::env::var("ROS2_INFO_USERNAME").unwrap_or_default();
    let password = std::env::var("ROS2_INFO_PASSWORD").unwrap_or_default();
    if username.is_empty() && password.is_empty() {
        return Ok(next.run(req).await);
    }
    let auth_header = req.headers().get("authorization").and_then(|v| v.to_str().ok());
    match auth_header {
        Some(val) if val.starts_with("Basic ") => {
            let encoded = val.trim_start_matches("Basic ");
            let decoded_bytes = base64_decode(encoded);
            let decoded = String::from_utf8_lossy(&decoded_bytes);
            let parts: Vec<&str> = decoded.splitn(2, ':').collect();
            if parts.len() == 2 && parts[0] == username && parts[1] == password {
                return Ok(next.run(req).await);
            }
        }
        _ => {}
    }
    Err(StatusCode::UNAUTHORIZED)
}

fn base64_decode(input: &str) -> Vec<u8> {
    use base64::Engine as _;
    let cleaned: String = input.chars().filter(|c| *c != '\n' && *c != '\r').collect();
    base64::engine::general_purpose::STANDARD.decode(&cleaned).unwrap_or_default()
}

async fn api_logo(Path(distro): Path<String>) -> impl IntoResponse {
    let valid_distros = [
        "jazzy", "humble", "iron", "rolling", "kilted",
        "foxy", "galactic", "eloquent", "crystal", "bouncy", "ardent",
        "lunar", "melodic", "noetic", "generic",
    ];
    let name = if valid_distros.contains(&distro.as_str()) {
        format!("{distro}_cropped.png")
    } else {
        "ros_generic_cropped.png".to_string()
    };

    let project_root = find_project_root();
    let path = std::path::PathBuf::from(&project_root)
        .join("fetch_info/display/assets")
        .join(&name);

    match tokio::fs::read(&path).await {
        Ok(bytes) => (
            StatusCode::OK,
            [("content-type", "image/png")],
            bytes,
        ).into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let args: Vec<String> = std::env::args().collect();
    let port: u16 = args
        .iter()
        .position(|a| a == "--port")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(8100);

    let template_dir = args
        .iter()
        .position(|a| a == "--templates")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(find_template_dir);

    info!("Starting ROS2 Info real-time backend on port {port}");
    info!("Template directory: {template_dir}");

    let state = AppState {
        sys: Arc::new(Mutex::new(System::new_all())),
        disks: Arc::new(Mutex::new(Disks::new_with_refreshed_list())),
        networks: Arc::new(Mutex::new(Networks::new_with_refreshed_list())),
    };

    let app = Router::new()
        .route("/api/status", get(api_status))
        .route("/api/info", get(api_info))
        .route("/api/graph", get(api_graph))
        .route("/api/exec", post(api_exec))
        .route("/api/trend", get(api_trend))
        .route("/api/launch-verify", post(api_launch_verify))
        .route("/api/bag-analyze", post(api_bag_analyze))
        .route("/api/fleet", post(api_fleet))
        .route("/api/logo/:distro", get(api_logo))
        .route("/ws", get(handle_ws))
        .nest_service("/static", ServeDir::new(&template_dir))
        .fallback_service(ServeDir::new(&template_dir).append_index_html_on_directories(true))
        .layer(CorsLayer::permissive())
        .layer(middleware::from_fn(auth_middleware))
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    info!("Listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
