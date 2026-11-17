use reqwest::{Client, Response, StatusCode};
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, watch};

pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) league-analysis pick helper";

fn is_retry_status(status: StatusCode) -> bool {
    matches!(status.as_u16(), 429 | 500 | 502 | 503 | 504)
}

pub struct RateLimiter {
    interval: Duration,
    next_at: Mutex<Instant>,
}

impl RateLimiter {
    pub fn new(per_second: f64) -> Self {
        Self {
            interval: Duration::from_secs_f64(1.0 / per_second.max(0.1)),
            next_at: Mutex::new(Instant::now()),
        }
    }

    pub async fn wait(&self) {
        let sleep_until = {
            let mut next = self.next_at.lock().await;
            let now = Instant::now();
            let at = now.max(*next);
            *next = at + self.interval;
            at
        };
        let now = Instant::now();
        if sleep_until > now {
            tokio::time::sleep(sleep_until - now).await;
        }
    }
}

pub struct Fetcher {
    pub http: Client,
    pub limit: RateLimiter,
    inflight: Mutex<HashMap<String, watch::Receiver<Option<Arc<Value>>>>>,
    failures: AtomicUsize,
}

impl Fetcher {
    pub fn new(per_second: f64, timeout: Duration) -> Self {
        let http = Client::builder()
            .timeout(timeout)
            .user_agent(USER_AGENT)
            .build()
            .expect("reqwest client");
        Self {
            http,
            limit: RateLimiter::new(per_second),
            inflight: Mutex::new(HashMap::new()),
            failures: AtomicUsize::new(0),
        }
    }

    pub fn failures(&self) -> usize {
        self.failures.load(Ordering::Relaxed)
    }

    pub fn note_failure(&self) {
        self.failures.fetch_add(1, Ordering::Relaxed);
    }

    /// Deduplicate identical concurrent fetches for `key`.
    pub async fn once<F, Fut>(&self, key: &str, f: F) -> Arc<Value>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Arc<Value>>,
    {
        let (tx, rx) = {
            let mut map = self.inflight.lock().await;
            if let Some(existing) = map.get(key) {
                let mut rx = existing.clone();
                drop(map);
                loop {
                    if let Some(val) = rx.borrow().as_ref() {
                        return Arc::clone(val);
                    }
                    if rx.changed().await.is_err() {
                        return Arc::new(Value::Object(Default::default()));
                    }
                }
            }
            let (tx, rx) = watch::channel(None);
            map.insert(key.to_string(), rx.clone());
            (tx, rx)
        };

        let result = f().await;
        let _ = tx.send(Some(Arc::clone(&result)));
        let _ = rx.borrow();
        let mut map = self.inflight.lock().await;
        map.remove(key);
        result
    }

    pub async fn get(&self, url: &str, params: &[(&str, &str)], attempts: usize) -> Result<Response, reqwest::Error> {
        for i in 1..=attempts {
            self.limit.wait().await;
            match self.http.get(url).query(params).send().await {
                Ok(resp) => {
                    if !is_retry_status(resp.status()) || i == attempts {
                        return Ok(resp);
                    }
                    let wait = resp
                        .headers()
                        .get("Retry-After")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse::<f64>().ok())
                        .map(|s| Duration::from_secs_f64(s.clamp(0.1, 10.0)))
                        .unwrap_or_else(|| Duration::from_secs(1 << i));
                    tokio::time::sleep(wait).await;
                }
                Err(e) => {
                    if i == attempts {
                        return Err(e);
                    }
                    tokio::time::sleep(Duration::from_secs(1 << i)).await;
                }
            }
        }
        unreachable!()
    }
}
