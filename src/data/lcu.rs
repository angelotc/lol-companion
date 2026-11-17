use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use regex::Regex;
use reqwest::{Client, Response, StatusCode};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio_rustls::rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use tokio_rustls::rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use tokio_rustls::rustls::{ClientConfig, DigitallySignedStruct, SignatureScheme};
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::{Connector, connect_async_tls_with_config};
use tracing::debug;

use crate::domain::config::DEFAULT_LOCKFILE;
use crate::domain::lcu::{LcuCreds, normalize_position, parse_lockfile_text};
use crate::domain::scoring::LANES;

pub const SESSION_EVENT: &str = "OnJsonApiEvent_lol-champ-select_v1_session";
pub const PHASE_EVENT: &str = "OnJsonApiEvent_lol-gameflow_v1_gameflow-phase";

static SCAN_FAILED_AT: Mutex<Option<Instant>> = Mutex::new(None);
const SCAN_RETRY: Duration = Duration::from_secs(180);

#[cfg(windows)]
fn league_ux_running() -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
    };
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return false;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut found = false;
        if Process32FirstW(snap, &mut entry) != 0 {
            loop {
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
                if name.eq_ignore_ascii_case("LeagueClientUx.exe") {
                    found = true;
                    break;
                }
                if Process32NextW(snap, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snap);
        found
    }
}

#[cfg(not(windows))]
fn league_ux_running() -> bool {
    false
}

fn creds_from_process() -> Result<LcuCreds, String> {
    if !league_ux_running() {
        return Err("client not running (LeagueClientUx.exe not active)".into());
    }
    if let Ok(guard) = SCAN_FAILED_AT.lock()
        && let Some(t) = *guard
        && t.elapsed() < SCAN_RETRY
    {
        return Err("client not running (recent scan found none)".into());
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let cmd = "Get-CimInstance Win32_Process -Filter \"Name='LeagueClientUx.exe'\" | Select-Object -ExpandProperty CommandLine";
        let out = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", cmd])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| format!("client not running ({e})"))?;
        let stdout = String::from_utf8_lossy(&out.stdout);
        static PORT_RE: OnceLock<Regex> = OnceLock::new();
        static TOKEN_RE: OnceLock<Regex> = OnceLock::new();
        let port_re = PORT_RE.get_or_init(|| Regex::new(r"--app-port=(\d+)").unwrap());
        let tok_re = TOKEN_RE.get_or_init(|| Regex::new(r"--remoting-auth-token=([\w-]+)").unwrap());
        if let (Some(p), Some(t)) = (port_re.captures(&stdout), tok_re.captures(&stdout))
            && let Ok(port) = p[1].parse::<u16>()
        {
            if let Ok(mut g) = SCAN_FAILED_AT.lock() {
                *g = None;
            }
            return Ok(LcuCreds {
                port,
                password: t[1].to_string(),
                protocol: "https".into(),
                pid: 0,
                name: "LeagueClient".into(),
            });
        }
    }
    if let Ok(mut g) = SCAN_FAILED_AT.lock() {
        *g = Some(Instant::now());
    }
    Err("client not running (no lockfile, no creds)".into())
}

pub fn read_lockfile(path: Option<&str>) -> Result<LcuCreds, String> {
    let p = Path::new(path.unwrap_or(DEFAULT_LOCKFILE));
    if let Ok(txt) = fs::read_to_string(p)
        && let Some(creds) = parse_lockfile_text(&txt)
    {
        return Ok(creds);
    }
    creds_from_process()
}

#[derive(Clone)]
pub struct LcuClient {
    base_url: String,
    auth_header: String,
    http: Client,
}

impl LcuClient {
    pub fn new(creds: &LcuCreds) -> Self {
        let auth = STANDARD.encode(format!("riot:{}", creds.password));
        let http = Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(Duration::from_secs(5))
            .build()
            .expect("lcu reqwest client");
        Self {
            base_url: format!("https://127.0.0.1:{}", creds.port),
            auth_header: format!("Basic {auth}"),
            http,
        }
    }

    pub async fn get(&self, path: &str, timeout: Option<Duration>) -> Result<Response, reqwest::Error> {
        let mut req = self
            .http
            .get(format!("{}{path}", self.base_url))
            .header("Authorization", &self.auth_header);
        if let Some(t) = timeout {
            req = req.timeout(t);
        }
        req.send().await
    }

    pub async fn get_session(&self) -> Result<Option<Value>, String> {
        let r = self
            .get("/lol-champ-select/v1/session", None)
            .await
            .map_err(|e| e.to_string())?;
        if r.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let r = r.error_for_status().map_err(|e| e.to_string())?;
        let val: Value = r.json().await.map_err(|e| e.to_string())?;
        Ok(Some(val))
    }

    pub async fn get_pickable_ids(&self) -> Result<HashSet<i32>, String> {
        let r = self
            .get("/lol-champ-select/v1/pickable-champion-ids", None)
            .await
            .map_err(|e| e.to_string())?;
        if r.status() == StatusCode::NOT_FOUND {
            return Ok(HashSet::new());
        }
        let r = r.error_for_status().map_err(|e| e.to_string())?;
        let ids: Vec<i32> = r.json().await.map_err(|e| e.to_string())?;
        Ok(ids.into_iter().collect())
    }

    pub async fn get_phase(&self) -> Result<String, String> {
        let r = self
            .get("/lol-gameflow/v1/gameflow-phase", None)
            .await
            .map_err(|e| e.to_string())?;
        let r = r.error_for_status().map_err(|e| e.to_string())?;
        let val: Value = r.json().await.map_err(|e| e.to_string())?;
        Ok(val.as_str().unwrap_or("None").to_string())
    }

    pub async fn get_lobby_positions(&self) -> Vec<String> {
        let Ok(r) = self.get("/lol-lobby/v2/lobby", None).await else {
            return Vec::new();
        };
        if r.status() != StatusCode::OK {
            return Vec::new();
        }
        let Ok(val) = r.json::<Value>().await else {
            return Vec::new();
        };
        let Some(me) = val.get("localMember") else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for key in ["firstPositionPreference", "secondPositionPreference"] {
            let raw = me.get(key).and_then(|v| v.as_str()).unwrap_or("");
            let pos = normalize_position(raw);
            if LANES.contains(&pos.as_str()) && !out.contains(&pos) {
                out.push(pos);
            }
        }
        out
    }
}

#[derive(Debug)]
struct NoCertVerify;

impl ServerCertVerifier for NoCertVerify {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, tokio_rustls::rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, tokio_rustls::rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, tokio_rustls::rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::ED25519,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
            SignatureScheme::RSA_PSS_SHA512,
        ]
    }
}

#[derive(Debug, Clone)]
pub enum LcuUpdate {
    Phase(String),
    Session(Option<Value>),
}

pub async fn watch_session(lockfile_path: Arc<tokio::sync::RwLock<String>>, tx: mpsc::UnboundedSender<LcuUpdate>) {
    let mut delay = Duration::from_secs(1);
    let max_backoff = Duration::from_secs(15);
    let mut connected = false;

    let tls_cfg = Arc::new(
        ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoCertVerify))
            .with_no_client_auth(),
    );

    loop {
        let path = lockfile_path.read().await.clone();
        match read_lockfile(Some(&path)) {
            Ok(creds) => {
                let auth = STANDARD.encode(format!("riot:{}", creds.password));
                let url = format!("wss://127.0.0.1:{}/", creds.port);
                if let Ok(mut req) = url.into_client_request() {
                    if let Ok(val) = HeaderValue::from_str(&format!("Basic {auth}")) {
                        req.headers_mut().insert("Authorization", val);
                    }
                    let connector = Connector::Rustls(Arc::clone(&tls_cfg));
                    match connect_async_tls_with_config(req, None, false, Some(connector)).await {
                        Ok((mut ws, _)) => {
                            let _ = ws.send(Message::Text(json!([5, SESSION_EVENT]).to_string().into())).await;
                            let _ = ws.send(Message::Text(json!([5, PHASE_EVENT]).to_string().into())).await;
                            connected = true;
                            delay = Duration::from_secs(1);

                            let client = LcuClient::new(&creds);
                            if let Ok(phase) = client.get_phase().await {
                                let _ = tx.send(LcuUpdate::Phase(phase));
                            }
                            if let Ok(sess) = client.get_session().await {
                                let _ = tx.send(LcuUpdate::Session(sess));
                            }

                            while let Some(msg) = ws.next().await {
                                let Ok(msg) = msg else {
                                    break;
                                };
                                let txt = match msg {
                                    Message::Text(t) => t.to_string(),
                                    Message::Close(_) => break,
                                    _ => continue,
                                };
                                if txt.is_empty() {
                                    continue;
                                }
                                let Ok(payload) = serde_json::from_str::<Value>(&txt) else {
                                    continue;
                                };
                                let Some(arr) = payload.as_array() else {
                                    continue;
                                };
                                if arr.len() < 3 || arr[0].as_i64() != Some(8) {
                                    continue;
                                }
                                let topic = arr[1].as_str().unwrap_or("");
                                let body = &arr[2];
                                if topic == PHASE_EVENT {
                                    let ph = body
                                        .get("data")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("None")
                                        .to_string();
                                    let _ = tx.send(LcuUpdate::Phase(ph));
                                } else if topic == SESSION_EVENT {
                                    let is_del = body.get("eventType").and_then(|v| v.as_str()) == Some("Delete");
                                    let data = if is_del {
                                        None
                                    } else {
                                        body.get("data").cloned().filter(|v| !v.is_null())
                                    };
                                    let _ = tx.send(LcuUpdate::Session(data));
                                }
                            }
                        }
                        Err(e) => {
                            debug!("lcu ws connect: {e}");
                        }
                    }
                }
            }
            Err(e) => {
                debug!("lcu creds: {e}");
            }
        }

        if connected {
            connected = false;
            let _ = tx.send(LcuUpdate::Phase("None".into()));
            let _ = tx.send(LcuUpdate::Session(None));
        }
        tokio::time::sleep(delay).await;
        delay = (delay * 2).min(max_backoff);
    }
}
