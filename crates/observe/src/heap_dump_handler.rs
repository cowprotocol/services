use {
    std::time::Duration,
    tokio::{
        io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
        net::{UnixListener, UnixStream},
    },
};

/// Spawns a new async task that listens for connections to a UNIX socket
/// at "/tmp/heap_dump_<process_name>.sock".
/// When "dump" command is sent, it generates a heap profile using
/// jemalloc_pprof and streams the binary protobuf data back through the socket.
///
/// Profiling is enabled at runtime via the MALLOC_CONF environment variable.
/// Set MALLOC_CONF=prof:true to enable heap profiling.
///
/// Usage:
/// ```bash
/// # From your local machine (one-liner):
/// kubectl exec <pod> -n <namespace> -- sh -c "echo dump | nc -U /tmp/heap_dump_<binary_name>.sock" > heap.pprof
///
/// # Analyze with pprof:
/// go tool pprof -http=:8080 heap.pprof
/// ```
pub fn spawn_heap_dump_handler() {
    // Check if jemalloc profiling is available at runtime
    // This depends on whether MALLOC_CONF=prof:true was set
    let profiling_available =
        std::panic::catch_unwind(|| jemalloc_pprof::PROF_CTL.as_ref().is_some()).unwrap_or(false);

    if !profiling_available {
        // Profiling is disabled - do nothing
        return;
    }

    tracing::info!("jemalloc heap profiling is active");

    tokio::spawn(async move {
        let name = binary_name().unwrap_or("unknown".to_string());
        let socket_path = format!("/tmp/heap_dump_{name}.sock");

        tracing::info!(socket = socket_path, "heap dump handler started");

        let _ = tokio::fs::remove_file(&socket_path).await;
        let listener = match UnixListener::bind(&socket_path) {
            Ok(listener) => listener,
            Err(err) => {
                tracing::error!(
                    ?err,
                    socket = socket_path,
                    "failed to bind heap dump socket"
                );
                return;
            }
        };
        let handle = SocketHandle {
            listener,
            socket_path,
        };

        loop {
            // Accept connection in main loop, then spawn task for each connection
            // Sequential processing prevents multiple simultaneous expensive heap dumps
            match handle.listener.accept().await {
                Ok((socket, _addr)) => {
                    let mut handle = tokio::spawn(async move {
                        handle_connection_with_socket(socket).await;
                    });

                    // 1-minute timeout to prevent stuck dumps from blocking future requests
                    match tokio::time::timeout(Duration::from_secs(60), &mut handle).await {
                        Ok(Ok(())) => {
                            // Task completed successfully
                        }
                        Ok(Err(err)) => {
                            tracing::error!(?err, "panic in heap dump connection handler");
                        }
                        Err(elapsed) => {
                            handle.abort();
                            tracing::error!(?elapsed, "heap dump request timed out");
                        }
                    }
                }
                Err(err) => {
                    tracing::debug!(?err, "failed to accept connection");
                }
            }
        }
    });
}

struct SocketHandle {
    socket_path: String,
    listener: UnixListener,
}

impl Drop for SocketHandle {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

fn binary_name() -> Option<String> {
    Some(
        std::env::current_exe()
            .ok()?
            .file_name()?
            .to_str()?
            .to_string(),
    )
}

async fn handle_connection_with_socket(mut socket: UnixStream) {
    let message = read_line(&mut socket).await;
    match message.as_deref() {
        Some("dump") => {
            generate_and_stream_dump(&mut socket).await;
        }
        Some("") => {
            tracing::debug!("client disconnected");
        }
        None => {
            tracing::debug!("failed to read message");
        }
        Some(unknown) => {
            tracing::debug!(command = unknown, "unknown command");
        }
    }
}

async fn generate_and_stream_dump(socket: &mut UnixStream) {
    tracing::info!("generating heap dump");

    // PROF_CTL was already verified to be available in spawn_heap_dump_handler
    // so we can safely unwrap here. If this panics, it means there's a serious bug.
    let prof_ctl = jemalloc_pprof::PROF_CTL
        .as_ref()
        .expect("PROF_CTL should be available - checked at handler spawn");

    let pprof_data = {
        let mut lock = prof_ctl.lock().await;
        lock.dump_pprof()
    };

    match pprof_data {
        Ok(pprof_data) => {
            tracing::info!(size_bytes = pprof_data.len(), "heap dump generated");

            if let Err(err) = socket.write_all(&pprof_data).await {
                tracing::warn!(?err, "failed to write heap dump to socket");
            }
        }
        Err(err) => {
            tracing::error!(?err, "failed to generate heap dump");
        }
    }
}

async fn read_line(socket: &mut UnixStream) -> Option<String> {
    let mut reader = BufReader::new(socket);
    let mut buffer = String::new();
    reader.read_line(&mut buffer).await.ok()?;
    Some(buffer.trim().to_owned())
}
