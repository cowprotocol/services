use {
    std::{
        ffi::{CString, c_char},
        os::unix::prelude::OsStrExt,
        path::PathBuf,
        str::FromStr,
        sync::Arc,
    },
    tokio::signal::unix::{SignalKind, signal},
};

#[derive(Clone)]
pub struct JemallocMemoryProfiler {
    inner: Arc<Inner>,
}

impl JemallocMemoryProfiler {
    pub fn new() -> Option<Self> {
        if std::env::var("_RJEM_MALLOC_CONF").is_err() {
            tracing::warn!("_RJEM_MALLOC_CONF is not set, memory profiler is disabled");
            return None;
        }
        let dump_dir_path_str = std::env::var("MEM_DUMP_PATH").ok().unwrap_or_else(|| {
            tracing::info!("MEM_DUMP_PATH is not set, using default /tmp/dump");
            "/tmp/dump".to_string()
        });
        let Some(dump_dir_path) = PathBuf::from_str(&dump_dir_path_str).ok() else {
            tracing::warn!(
                "Invalid MEM_DUMP_PATH: {dump_dir_path_str}, memory profiler is disabled"
            );
            return None;
        };

        let Ok::<bool, _>(active) = (unsafe { tikv_jemalloc_ctl::raw::read(PROF_ACTIVE) }) else {
            tracing::error!("failed to read memory profiler state");
            return None;
        };

        Some(Self {
            inner: Arc::new(Inner {
                active: tokio::sync::Mutex::new(active),
                dump_dir_path,
            }),
        })
    }

    pub fn run(self) {
        tokio::spawn(async move {
            let mut sigusr2 = match signal(SignalKind::user_defined2()) {
                Ok(signal) => signal,
                Err(err) => {
                    tracing::error!(?err, "failed to bind to SIGUSR2");
                    return;
                }
            };

            tracing::info!("jemalloc memory profiler background task started; Waiting for SIGUSR2");

            while sigusr2.recv().await.is_some() {
                tracing::info!("SIGUSR2 received: triggering memory profiling dump");

                // Enable profiler
                if !self.set_enabled(true).await {
                    tracing::warn!("failed to enable jemalloc profiler");
                    continue;
                }

                // Perform dump
                self.dump().await;

                // Disable profiler
                if !self.set_enabled(false).await {
                    tracing::warn!("failed to disable jemalloc profiler");
                }

                tracing::info!("jemalloc memory profiler dump complete");
            }
        });
    }
}

impl JemallocMemoryProfiler {
    async fn set_enabled(&self, enabled: bool) -> bool {
        let mut state = self.inner.active.lock().await;
        match unsafe { tikv_jemalloc_ctl::raw::update(PROF_ACTIVE, enabled) } {
            Ok(was_enabled) => {
                *state = enabled;
                was_enabled != enabled
            }
            Err(err) => {
                tracing::error!(?err, "failed to update memory profiler state");
                false
            }
        }
    }

    async fn dump(&self) {
        let state = self.inner.active.lock().await;
        if !*state {
            tracing::error!("memory profiler is not active, cannot dump");
        }

        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let filename = format!("jemalloc_dump_{timestamp}.heap");
        let full_path = self.inner.dump_dir_path.join(filename);
        {
            let Some(bytes) = CString::new(full_path.as_os_str().as_bytes()).ok() else {
                tracing::error!(?full_path, "failed to create CString from path");
                return;
            };

            let mut bytes = bytes.into_bytes_with_nul();
            let ptr = bytes.as_mut_ptr().cast::<c_char>();
            if let Err(err) = unsafe { tikv_jemalloc_ctl::raw::write(PROF_DUMP, ptr) } {
                tracing::error!(?err, "failed to dump jemalloc profiling data");
            }
        }

        tracing::info!(?full_path, "saved the jemalloc profiling dump");
    }
}

struct Inner {
    active: tokio::sync::Mutex<bool>,
    dump_dir_path: PathBuf,
}

const PROF_ACTIVE: &[u8] = b"prof.active\0";
const PROF_DUMP: &[u8] = b"prof.dump\0";
