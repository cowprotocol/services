use {
    anyhow::Context,
    std::{
        ffi::{CString, c_char},
        os::unix::prelude::OsStrExt,
        path::PathBuf,
        str::FromStr,
        time::Duration,
    },
    tokio::{
        io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
        net::{UnixListener, UnixStream},
    },
};

pub struct JemallocMemoryProfiler {
    process_name: String,
    active: tokio::sync::Mutex<bool>,
}

impl JemallocMemoryProfiler {
    pub fn new(process_name: &str) -> Option<Self> {
        if std::env::var("_RJEM_MALLOC_CONF").is_err() {
            tracing::info!("_RJEM_MALLOC_CONF is not set, memory profiler is disabled");
            return None;
        }

        let Ok::<bool, _>(active) = (unsafe { tikv_jemalloc_ctl::raw::read(PROF_ACTIVE) }) else {
            tracing::error!("failed to read memory profiler state, disabling");
            return None;
        };

        tracing::info!(active, "jemalloc memory profiler initialized");

        Some(Self {
            process_name: process_name.to_string(),
            active: tokio::sync::Mutex::new(active),
        })
    }

    pub fn run(self) {
        tokio::spawn(async move {
            let process_name = self.process_name.as_str();
            let socket_path = std::env::var("PROFILER_CMD_FILE")
                .unwrap_or_else(|_| format!("/tmp/profiler_cmd_{process_name}.sock").to_string());

            tracing::info!(file = socket_path, "opening profiler command socket");
            let _ = tokio::fs::remove_file(&socket_path).await;

            let handle = SocketHandle {
                listener: UnixListener::bind(&socket_path).expect("socket handle is unique"),
                socket_path: socket_path.clone(),
            };

            tracing::info!(
                "jemalloc memory profiler background task started; Waiting for socket connections"
            );

            loop {
                self.handle_connection(&handle.listener).await;
            }
        });
    }
}

impl JemallocMemoryProfiler {
    async fn set_enabled(&self, enabled: bool, socket: &mut UnixStream) -> bool {
        let mut state = self.active.lock().await;
        match unsafe { tikv_jemalloc_ctl::raw::update(PROF_ACTIVE, enabled) } {
            Ok(was_enabled) => {
                *state = enabled;
                was_enabled != enabled
            }
            Err(err) => {
                log(
                    socket,
                    format!("failed to set memory profiler state to {enabled}: {err:?}"),
                )
                .await;
                false
            }
        }
    }

    async fn dump_prof(&self, socket: &mut UnixStream) {
        let state = self.active.lock().await;
        // Hold the lock until the dump is complete.
        if !*state {
            log(
                socket,
                "memory profiler is not active, cannot dump".to_string(),
            )
            .await;
            return;
        }

        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let process_name = self.process_name.as_str();
        let filename = format!("jemalloc_dump_{process_name}_{timestamp}.heap");
        let full_path = match Self::get_dump_dir_path(socket).await {
            Ok(path) => path.join(filename),
            Err(err) => {
                log(
                    socket,
                    format!("failed to get dump dir path, dump was not saved: {err:?}"),
                )
                .await;
                return;
            }
        };

        {
            let Some(bytes) = CString::new(full_path.as_os_str().as_bytes()).ok() else {
                log(
                    socket,
                    format!("failed to create CString from path {full_path:?}"),
                )
                .await;
                return;
            };

            log(
                socket,
                format!("saving jemalloc profiling dump to {full_path:?}"),
            )
            .await;
            let mut bytes = bytes.into_bytes_with_nul();
            let ptr = bytes.as_mut_ptr().cast::<c_char>();
            if let Err(err) = unsafe { tikv_jemalloc_ctl::raw::write(PROF_DUMP, ptr) } {
                log(
                    socket,
                    format!("failed to dump jemalloc profiling data: {err:?}"),
                )
                .await;
                return;
            }
        }

        log(
            socket,
            format!("saved jemalloc profiling dump to {full_path:?}"),
        )
        .await;
    }

    async fn get_dump_dir_path(socket: &mut UnixStream) -> anyhow::Result<PathBuf> {
        let dump_dir_path_str = match std::env::var("MEM_DUMP_PATH").ok() {
            Some(path) => path,
            None => {
                log(
                    socket,
                    "MEM_DUMP_PATH is not set, using system temp directory as default".to_string(),
                )
                .await;
                std::env::temp_dir().to_string_lossy().to_string()
            }
        };

        PathBuf::from_str(&dump_dir_path_str).context(format!(
            "failed to parse dump dir path: {dump_dir_path_str}"
        ))
    }

    async fn handle_connection(&self, listener: &UnixListener) {
        match listener.accept().await {
            Ok((mut socket, _)) => loop {
                let message = Self::read_line(&mut socket).await;
                match message.as_deref() {
                    Some("") => {
                        log(&mut socket, "client terminated connection".into()).await;
                        break;
                    }
                    None => {
                        log(&mut socket, "failed to read message from socket".into()).await;
                        break;
                    }
                    Some(line) => {
                        if let Err(err) = self.handle_command(line, &mut socket).await {
                            log(
                                &mut socket,
                                format!("error handling profiler command {line:?}: {err:?}"),
                            )
                            .await;
                        }
                        continue;
                    }
                }
            },
            Err(err) => {
                tracing::error!(?err, "error accepting connection");
            }
        }
    }

    async fn read_line(socket: &mut UnixStream) -> Option<String> {
        let mut reader = BufReader::new(socket);
        let mut buffer = String::new();
        reader.read_line(&mut buffer).await.ok()?;
        Some(buffer.trim().to_owned())
    }

    async fn handle_command(&self, command: &str, socket: &mut UnixStream) -> anyhow::Result<()> {
        match ProfilerCommand::from_str(command).map_err(|err| anyhow::anyhow!(err))? {
            ProfilerCommand::Enable => {
                if self.set_enabled(true, socket).await {
                    log(socket, "jemalloc active profiling enabled".to_string()).await;
                }
            }
            ProfilerCommand::Disable => {
                if self.set_enabled(false, socket).await {
                    log(socket, "jemalloc active profiling disabled".to_string()).await;
                }
            }
            ProfilerCommand::Dump => {
                self.dump_prof(socket).await;
            }
            ProfilerCommand::RunFor(duration) => {
                if self.set_enabled(true, socket).await {
                    log(socket, "jemalloc active profiling enabled".to_string()).await;
                } else {
                    log(
                        socket,
                        "jemalloc active profiling was already enabled, will run for the \
                         specified duration"
                            .to_string(),
                    )
                    .await;
                    return Ok(());
                }

                log(
                    socket,
                    format!(
                        "jemalloc active profiling will run for {duration:?}, after which a dump \
                         will be created and profiling disabled"
                    ),
                )
                .await;
                tokio::time::sleep(duration).await;

                self.dump_prof(socket).await;

                if self.set_enabled(false, socket).await {
                    log(socket, "jemalloc active profiling disabled".to_string()).await;
                }
            }
        }
        Ok(())
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

struct SocketHandle {
    listener: UnixListener,
    socket_path: String,
}

impl Drop for SocketHandle {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

// @todo: deduplicate
/// Logs the message in this process' logs and reports it back to the
/// connected socket.
async fn log(socket: &mut UnixStream, message: String) {
    // Use a fairly high log level to improve chances that this actually gets logged
    // when somebody messed with the log filter.
    tracing::warn!(message);
    let _ = socket.write_all(message.as_bytes()).await;
    let _ = socket.write_all(b"\n").await;
}
