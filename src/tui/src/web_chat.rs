use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// Cap on retained chat messages so the in-memory history can't grow without
/// bound (it previously held every message for the server's lifetime).
const MAX_HISTORY: usize = 200;

pub struct WebChatServer {
    port: u16,
    history: Arc<Mutex<Vec<ChatMessage>>>,
    stop: Arc<AtomicBool>,
}

impl WebChatServer {
    pub fn new(port: u16, stop: Arc<AtomicBool>) -> Self {
        Self {
            port,
            history: Arc::new(Mutex::new(Vec::new())),
            stop,
        }
    }

    pub fn start(self) -> thread::JoinHandle<()> {
        let port = self.port;
        let history = self.history;
        let stop = self.stop;

        thread::spawn(move || {
            // Bind to loopback only: this server proxies straight to local
            // Ollama, so exposing it on every interface would hand an open AI
            // endpoint to anyone on the network.
            let server = match tiny_http::Server::http(format!("127.0.0.1:{}", port)) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("[web_chat] Failed to start server on port {}: {}", port, e);
                    return;
                }
            };

            eprintln!("[web_chat] Server running at http://localhost:{}", port);

            loop {
                if stop.load(Ordering::SeqCst) {
                    eprintln!(
                        "[web_chat] Stop signal received, shutting down port {}",
                        port
                    );
                    break;
                }
                match server.try_recv() {
                    Ok(Some(request)) => handle_request(request, port, &history),
                    _ => thread::sleep(Duration::from_millis(100)),
                }
            }
        })
    }
}

fn handle_request(
    mut request: tiny_http::Request,
    port: u16,
    history: &Arc<Mutex<Vec<ChatMessage>>>,
) {
    let method = request.method().to_string();
    let url = request.url().to_string();

    match (method.as_str(), url.as_str()) {
        ("GET", "/") => {
            let html = HTML_PAGE.replace("{{PORT}}", &port.to_string());
            let response = tiny_http::Response::from_string(html).with_header(
                tiny_http::Header::from_bytes(
                    &b"Content-Type"[..],
                    &b"text/html; charset=utf-8"[..],
                )
                .unwrap(),
            );
            let _ = request.respond(response);
        }
        ("POST", "/api/chat") => {
            let mut buf = Vec::new();
            let _ = request.as_reader().read_to_end(&mut buf);
            let body = String::from_utf8_lossy(&buf);

            let response_text = match serde_json::from_str::<serde_json::Value>(&body) {
                Ok(val) => {
                    let msg = val["message"].as_str().unwrap_or("");
                    match crate::ai::call_ollama(msg, "qwen2.5-coder:7b") {
                        Ok(reply) => {
                            if let Ok(mut h) = history.lock() {
                                h.push(ChatMessage {
                                    role: "user".to_string(),
                                    content: msg.to_string(),
                                });
                                h.push(ChatMessage {
                                    role: "assistant".to_string(),
                                    content: reply.clone(),
                                });
                                if h.len() > MAX_HISTORY {
                                    let overflow = h.len() - MAX_HISTORY;
                                    h.drain(0..overflow);
                                }
                            }
                            serde_json::json!({ "response": reply }).to_string()
                        }
                        Err(e) => {
                            serde_json::json!({ "response": format!("Error: {}", e) }).to_string()
                        }
                    }
                }
                Err(_) => serde_json::json!({ "response": "Invalid JSON body" }).to_string(),
            };

            let response = tiny_http::Response::from_string(response_text).with_header(
                tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
                    .unwrap(),
            );
            let _ = request.respond(response);
        }
        ("GET", "/api/history") => {
            let json = if let Ok(h) = history.lock() {
                let arr: Vec<serde_json::Value> = h
                    .iter()
                    .map(|m| {
                        serde_json::json!({
                            "role": m.role,
                            "content": m.content,
                        })
                    })
                    .collect();
                serde_json::to_string(&arr).unwrap_or_else(|_| "[]".to_string())
            } else {
                "[]".to_string()
            };

            let response = tiny_http::Response::from_string(json).with_header(
                tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
                    .unwrap(),
            );
            let _ = request.respond(response);
        }
        _ => {
            let response = tiny_http::Response::from_string("Not Found").with_status_code(404);
            let _ = request.respond(response);
        }
    }
}

const HTML_PAGE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>ROS2 AI Chat</title>
<style>
* { margin:0; padding:0; box-sizing:border-box; }
body {
  background:#1e1e2e; color:#cdd6f4; font-family:'JetBrains Mono','Fira Code','Cascadia Code',monospace;
  display:flex; flex-direction:column; height:100vh; overflow:hidden;
}
.header {
  background:#181825; border-bottom:1px solid #313244; padding:12px 20px;
  display:flex; align-items:center; justify-content:space-between; flex-shrink:0;
}
.header h1 { font-size:16px; color:#8be9fd; font-weight:600; letter-spacing:0.5px; }
.header .subtitle { font-size:11px; color:#585b70; }
.messages {
  flex:1; overflow-y:auto; padding:16px 20px; display:flex; flex-direction:column; gap:12px;
}
.messages::-webkit-scrollbar { width:6px; }
.messages::-webkit-scrollbar-track { background:#1e1e2e; }
.messages::-webkit-scrollbar-thumb { background:#45475a; border-radius:3px; }
.msg { max-width:75%; padding:10px 14px; border-radius:8px; line-height:1.5;
       font-size:13px; word-wrap:break-word; white-space:pre-wrap; }
.msg.user {
  align-self:flex-end; background:#1e1e2e; border:1px solid #8be9fd;
  color:#8be9fd;
}
.msg.assistant {
  align-self:flex-start; background:#181825; border:1px solid #45475a;
  color:#cdd6f4;
}
.msg.system {
  align-self:center; background:transparent; color:#585b70; font-size:11px;
  border:none; padding:4px 0;
}
.typing { display:flex; gap:4px; padding:10px 14px; }
.typing span {
  width:6px; height:6px; background:#585b70; border-radius:50%;
  animation: blink 1.4s infinite both;
}
.typing span:nth-child(2) { animation-delay:0.2s; }
.typing span:nth-child(3) { animation-delay:0.4s; }
@keyframes blink { 0%,80%,100%{opacity:0.2} 40%{opacity:1} }
.input-bar {
  background:#181825; border-top:1px solid #313244; padding:12px 20px;
  display:flex; gap:10px; flex-shrink:0;
}
.input-bar input {
  flex:1; background:#1e1e2e; border:1px solid #313244; color:#cdd6f4;
  padding:10px 14px; border-radius:6px; font-family:inherit; font-size:13px;
  outline:none; transition:border-color 0.2s;
}
.input-bar input:focus { border-color:#8be9fd; }
.input-bar input::placeholder { color:#585b70; }
.input-bar button {
  background:#89b4fa; color:#1e1e2e; border:none; padding:10px 20px;
  border-radius:6px; font-family:inherit; font-size:13px; font-weight:600;
  cursor:pointer; transition:background 0.2s;
}
.input-bar button:hover { background:#74c7ec; }
.input-bar button:disabled { background:#45475a; color:#585b70; cursor:not-allowed; }
.error-bar {
  background:#f38ba8; color:#1e1e2e; padding:8px 20px; font-size:12px;
  text-align:center; display:none; flex-shrink:0;
}
</style>
</head>
<body>
<div class="header">
  <h1>ROS2 AI Chat &mdash; localhost:{{PORT}}</h1>
  <span class="subtitle">Powered by Ollama + ros2_info TUI</span>
</div>
<div class="error-bar" id="error-bar"></div>
<div class="messages" id="messages">
  <div class="msg system">Connected. Type a message to start chatting with the AI.</div>
</div>
<div class="input-bar">
  <input type="text" id="input" placeholder="Ask something about ROS2, Rust, or this project..." autocomplete="off">
  <button id="send" onclick="send()">Send</button>
</div>
<script>
const messagesEl = document.getElementById('messages');
const inputEl = document.getElementById('input');
const sendBtn = document.getElementById('send');
const errorBar = document.getElementById('error-bar');
let sending = false;

inputEl.addEventListener('keydown', e => {
  if (e.key === 'Enter' && !e.shiftKey && !sending) { e.preventDefault(); send(); }
});
inputEl.focus();

function scrollToBottom() {
  messagesEl.scrollTop = messagesEl.scrollHeight;
}
scrollToBottom();

function addMsg(role, content) {
  const div = document.createElement('div');
  div.className = 'msg ' + role;
  div.textContent = content;
  messagesEl.appendChild(div);
  scrollToBottom();
  return div;
}

function showTyping() {
  const div = document.createElement('div');
  div.className = 'msg assistant';
  div.id = 'typing';
  div.innerHTML = '<div class="typing"><span></span><span></span><span></span></div>';
  messagesEl.appendChild(div);
  scrollToBottom();
}

function hideTyping() {
  const el = document.getElementById('typing');
  if (el) el.remove();
}

function showError(msg) {
  errorBar.textContent = msg;
  errorBar.style.display = 'block';
  setTimeout(() => { errorBar.style.display = 'none'; }, 5000);
}

async function send() {
  const text = inputEl.value.trim();
  if (!text || sending) return;
  sending = true;
  sendBtn.disabled = true;
  inputEl.value = '';
  addMsg('user', text);
  showTyping();

  try {
    const res = await fetch('/api/chat', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ message: text }),
    });
    if (!res.ok) throw new Error('HTTP ' + res.status);
    const data = await res.json();
    hideTyping();
    addMsg('assistant', data.response || '(empty)');
  } catch (err) {
    hideTyping();
    showError('Connection error: ' + err.message + '. Is Ollama running?');
    addMsg('system', 'Failed to get response. Check that Ollama is running on localhost:11434.');
  } finally {
    sending = false;
    sendBtn.disabled = false;
    inputEl.focus();
  }
}
</script>
</body>
</html>"#;
