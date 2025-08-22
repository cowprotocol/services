use {
    std::{
        ffi::{CString, c_char},
        os::unix::prelude::OsStrExt,
        path::PathBuf,
        str::FromStr,
        time::Duration,
    },
    tokio::signal::unix::{SignalKind, signal},
};

pub struct JemallocMemoryProfiler {
    process_name: String,
    active: tokio::sync::Mutex<bool>,
    dump_dir_path: PathBuf,
}

impl JemallocMemoryProfiler {
    pub fn new(process_name: &str) -> Option<Self> {
        if std::env::var("_RJEM_MALLOC_CONF").is_err() {
            tracing::info!("_RJEM_MALLOC_CONF is not set, memory profiler is disabled");
            return None;
        }
        let dump_dir_path_str = std::env::var("MEM_DUMP_PATH").ok().unwrap_or_else(|| {
            tracing::info!("MEM_DUMP_PATH is not set, using system temp directory as default");
            std::env::temp_dir().to_string_lossy().to_string()
        });
        let Some(dump_dir_path) = PathBuf::from_str(&dump_dir_path_str).ok() else {
            tracing::warn!(
                "Invalid MEM_DUMP_PATH: {dump_dir_path_str}, memory profiler is disabled"
            );
            return None;
        };

        let Ok::<bool, _>(active) = (unsafe { tikv_jemalloc_ctl::raw::read(PROF_ACTIVE) }) else {
            tracing::error!("failed to read memory profiler state, disabling");
            return None;
        };

        tracing::info!(
            ?dump_dir_path,
            active,
            "jemalloc memory profiler initialized"
        );

        Some(Self {
            process_name: process_name.to_string(),
            active: tokio::sync::Mutex::new(active),
            dump_dir_path,
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

                let Some(command) = std::env::var("PROFILER_COMMAND")
                    .ok()
                    .and_then(|cmd| ProfilerCommand::from_str(cmd.as_str()).ok())
                else {
                    tracing::warn!(
                        "PROFILER_COMMAND is not set or invalid, skipping jemalloc memory \
                         profiling"
                    );
                    continue;
                };

                match command {
                    ProfilerCommand::Enable => {
                        if self.set_enabled(true).await {
                            tracing::info!("jemalloc active profiling enabled");
                        }
                    }
                    ProfilerCommand::Disable => {
                        if self.set_enabled(false).await {
                            tracing::info!("jemalloc active profiling disabled");
                        }
                    }
                    ProfilerCommand::Dump => {
                        self.dump_prof().await;
                    }
                    ProfilerCommand::RunFor(duration) => {
                        if self.set_enabled(true).await {
                            tracing::info!("jemalloc active profiling enabled");
                        } else {
                            tracing::warn!(
                                "jemalloc active profiling was already enabled, disable it first \
                                 and try again"
                            );
                            continue;
                        }

                        tracing::info!(
                            "jemalloc active memory profiling will be executing for {duration:?}"
                        );
                        tokio::time::sleep(duration).await;

                        self.dump_prof().await;

                        if self.set_enabled(false).await {
                            tracing::info!("jemalloc active profiling disabled");
                        }
                    }
                }
            }
        });
    }
}

impl JemallocMemoryProfiler {
    async fn set_enabled(&self, enabled: bool) -> bool {
        let mut state = self.active.lock().await;
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

    async fn dump_prof(&self) {
        let state = self.active.lock().await;
        // Hold the lock until the dump is complete.
        if !*state {
            tracing::error!("memory profiler is not active, cannot dump");
            return;
        }

        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let process_name = self.process_name.as_str();
        let filename = format!("jemalloc_dump_{process_name}_{timestamp}.heap");
        let full_path = self.dump_dir_path.join(filename);
        {
            let Some(bytes) = CString::new(full_path.as_os_str().as_bytes()).ok() else {
                tracing::error!(?full_path, "failed to create CString from path");
                return;
            };

            let mut bytes = bytes.into_bytes_with_nul();
            let ptr = bytes.as_mut_ptr().cast::<c_char>();
            tracing::info!(?full_path, "dumping jemalloc profiling data");
            if let Err(err) = unsafe { tikv_jemalloc_ctl::raw::write(PROF_DUMP, ptr) } {
                tracing::error!(?err, "failed to dump jemalloc profiling data");
            }
        }

        tracing::info!(?full_path, "saved the jemalloc profiling dump");
    }
}

#[derive(Debug)]
enum ProfilerCommand {
    Enable,
    Disable,
    RunFor(Duration),
    Dump,
}

impl FromStr for ProfilerCommand {
    type Err = String;

    fn from_str(str: &str) -> Result<Self, Self::Err> {
        let lower = str.to_lowercase();
        if lower == "enable" {
            Ok(ProfilerCommand::Enable)
        } else if lower == "disable" {
            Ok(ProfilerCommand::Disable)
        } else if lower == "dump" {
            Ok(ProfilerCommand::Dump)
        } else if let Some(arg) = lower
            .strip_prefix("run_for(")
            .and_then(|rest| rest.strip_suffix(")"))
        {
            let dur = humantime::parse_duration(arg)
                .map_err(|err| format!("invalid duration {arg:?}: {err}"))?;
            Ok(ProfilerCommand::RunFor(dur))
        } else {
            Err(format!("Unknown command: {str}"))
        }
    }
}

const PROF_ACTIVE: &[u8] = b"prof.active\0";
const PROF_DUMP: &[u8] = b"prof.dump\0";
