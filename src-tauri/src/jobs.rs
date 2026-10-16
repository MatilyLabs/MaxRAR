use serde::Serialize;
use serde_json::Value;
use std::time::{Duration, Instant};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tauri::{AppHandle, Emitter};

type Tokens = Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>;
type ProgressCallback = Box<dyn Fn(f64, String) + Send + Sync>;

struct Registration {
    tokens: Tokens,
    id: String,
}
impl Drop for Registration {
    fn drop(&mut self) {
        if let Ok(mut tokens) = self.tokens.lock() {
            tokens.remove(&self.id);
        }
    }
}

#[derive(Clone)]
pub struct Jobs {
    tokens: Tokens,
    permits: Arc<tokio::sync::Semaphore>,
}

impl Default for Jobs {
    fn default() -> Self {
        Self {
            tokens: Arc::default(),
            permits: Arc::new(tokio::sync::Semaphore::new(1)),
        }
    }
}

#[derive(Clone, Serialize)]
pub struct JobEvent {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub progress: Option<f64>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt: Option<Value>,
}

impl Jobs {
    pub fn idle_permit(&self) -> Result<tokio::sync::OwnedSemaphorePermit, String> {
        let tokens = self
            .tokens
            .lock()
            .map_err(|_| "Job registry is unavailable")?;
        if !tokens.is_empty() {
            return Err("Finish archive jobs before installing an update".into());
        }
        self.permits
            .clone()
            .try_acquire_owned()
            .map_err(|_| "Finish archive jobs before installing an update".into())
    }

    pub fn cancel(&self, id: &str) -> Result<bool, String> {
        let tokens = self
            .tokens
            .lock()
            .map_err(|_| "Job registry is unavailable")?;
        if let Some(token) = tokens.get(id) {
            token.store(true, Ordering::Relaxed);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub async fn run<T, F>(
        &self,
        app: AppHandle,
        id: String,
        kind: &'static str,
        operation: F,
    ) -> Result<T, String>
    where
        T: Serialize + Send + 'static,
        F: FnOnce(Arc<AtomicBool>, ProgressCallback) -> Result<T, String> + Send + 'static,
    {
        if id.is_empty() || id.len() > 128 {
            return Err("A job id of 1–128 characters is required".into());
        }
        let token = Arc::new(AtomicBool::new(false));
        {
            let mut tokens = self
                .tokens
                .lock()
                .map_err(|_| "Job registry is unavailable")?;
            if tokens.contains_key(&id) {
                return Err("That job id is already active".into());
            }
            tokens.insert(id.clone(), token.clone());
        }
        let _registration = Registration {
            tokens: self.tokens.clone(),
            id: id.clone(),
        };
        let emit =
            |status: &str, progress: Option<f64>, message: String, receipt: Option<Value>| {
                let _ = app.emit(
                    "wonrar://job",
                    JobEvent {
                        id: id.clone(),
                        kind: kind.into(),
                        status: status.into(),
                        progress,
                        message,
                        receipt,
                    },
                );
            };
        emit("queued", None, "Waiting for a worker".into(), None);
        let acquire = self.permits.clone().acquire_owned();
        tokio::pin!(acquire);
        let permit = loop {
            if token.load(Ordering::Relaxed) {
                let message = "Operation cancelled before starting".to_owned();
                emit("cancelled", None, message.clone(), None);
                return Err(message);
            }
            tokio::select! {
                permit = &mut acquire => match permit {
                    Ok(permit) => break permit,
                    Err(error) => {
                        let message = error.to_string();
                        emit("failed", None, message.clone(), None);
                        return Err(message);
                    }
                },
                _ = tokio::time::sleep(Duration::from_millis(50)) => {}
            }
        };
        let result = if token.load(Ordering::Relaxed) {
            Err("Operation cancelled before starting".into())
        } else {
            emit("running", Some(0.0), "Starting operation".into(), None);
            let app_progress = app.clone();
            let id_progress = id.clone();
            let last_progress = Mutex::new(Instant::now() - Duration::from_millis(50));
            let callback = Box::new(move |fraction: f64, message: String| {
                let Ok(mut last) = last_progress.lock() else {
                    return;
                };
                if last.elapsed() < Duration::from_millis(50) {
                    return;
                }
                *last = Instant::now();
                let _ = app_progress.emit(
                    "wonrar://job",
                    JobEvent {
                        id: id_progress.clone(),
                        kind: kind.into(),
                        status: "running".into(),
                        progress: Some(fraction.clamp(0.0, 1.0)),
                        message,
                        receipt: None,
                    },
                );
            });
            let worker_token = token.clone();
            tauri::async_runtime::spawn_blocking(move || operation(worker_token, callback))
                .await
                .map_err(|e| e.to_string())
                .and_then(|r| r)
        };
        drop(permit);
        match &result {
            Ok(receipt) => emit(
                "completed",
                Some(1.0),
                "Operation completed".into(),
                serde_json::to_value(receipt).ok(),
            ),
            Err(error) => emit(
                if token.load(Ordering::Relaxed) {
                    "cancelled"
                } else {
                    "failed"
                },
                None,
                error.clone(),
                None,
            ),
        }
        result
    }
}

#[cfg(test)]
mod update_guard_tests {
    use super::*;
    #[test]
    fn update_installation_excludes_archive_workers() {
        let jobs = Jobs::default();
        let permit = jobs.idle_permit().unwrap();
        assert!(jobs.permits.clone().try_acquire_owned().is_err());
        drop(permit);
        assert!(jobs.idle_permit().is_ok());
    }
    #[test]
    fn queued_jobs_also_block_update_installation() {
        let jobs = Jobs::default();
        jobs.tokens
            .lock()
            .unwrap()
            .insert("queued".into(), Arc::new(AtomicBool::new(false)));
        assert!(jobs.idle_permit().is_err());
    }
}
