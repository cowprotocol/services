use {
    std::time::Duration,
    tikv_jemalloc_ctl::{arenas, epoch, stats},
    tokio::{
        io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
        net::{UnixListener, UnixStream},
    },
};

/// Spawns a new async task that listens for connections to a UNIX socket
/// at "/tmp/heap_dump_<process_name>.sock".
/// When "dump" command is sent, it generates a heap profile using
/// jemalloc_pprof and streams the binary protobuf data back through the socket.
/// When "stats" command is sent, it streams back a JSON report of jemalloc's
/// own counters (allocated/active/resident/retained/...), which is the cheapest
/// way to see the gap between live heap and process RSS.
///
/// Profiling is enabled at runtime via the MALLOC_CONF environment variable.
/// Set MALLOC_CONF=prof:true to enable heap profiling.
///
/// Usage:
/// ```bash
/// # Heap profile (one-liner):
/// kubectl exec <pod> -n <namespace> -- sh -c "echo dump | nc -U /tmp/heap_dump_<binary_name>.sock" > heap.pprof
///
/// # Analyze with pprof:
/// go tool pprof -http=:8080 heap.pprof
///
/// # Allocator stats (live heap vs RSS vs retained):
/// kubectl exec <pod> -n <namespace> -- sh -c "echo stats | nc -U /tmp/heap_dump_<binary_name>.sock"
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
        Some("stats") => {
            write_jemalloc_stats(&mut socket).await;
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
        let pprof_data = lock.dump_pprof();
        // While assembling the heap dump a global symbol cache gets filled with
        // the resolved identifiers. As that is quite large and does not get freed
        // automatically we do it explicitly here.
        backtrace::clear_symbol_cache();
        pprof_data
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

/// Streams a JSON report of jemalloc's internal counters. `allocated` tracks
/// the live application bytes (what the heap profile samples), while `resident`
/// approximates the allocator's contribution to RSS. A large `resident -
/// allocated` gap points at retained/fragmented pages (decay/arena tuning)
/// rather than at the application's live data.
async fn write_jemalloc_stats(socket: &mut UnixStream) {
    let report = match jemalloc_stats_report() {
        Ok(report) => report,
        Err(err) => {
            tracing::warn!(?err, "failed to read jemalloc stats");
            format!("failed to read jemalloc stats: {err}\n")
        }
    };
    if let Err(err) = socket.write_all(report.as_bytes()).await {
        tracing::warn!(?err, "failed to write jemalloc stats to socket");
    }
}

/// Snapshot of jemalloc's global counters. All byte counts are raw (consumers
/// can convert to MiB with e.g. `jq '.resident / 1048576'`).
#[derive(serde::Serialize)]
struct JemallocStats {
    /// Number of active arenas.
    narenas: u32,
    /// Live application bytes (what the heap profile samples).
    allocated: usize,
    /// Bytes in active pages.
    active: usize,
    /// Physically resident bytes (≈ allocator RSS).
    resident: usize,
    /// Total bytes mapped from the OS.
    mapped: usize,
    /// Unmapped bytes held back from the OS by decay.
    retained: usize,
    /// Allocator bookkeeping bytes.
    metadata: usize,
    /// `active - allocated`: internal fragmentation within active pages.
    fragmentation: usize,
    /// `resident - allocated`: allocator RSS overhead over live data.
    overhead: usize,
}

fn jemalloc_stats_report() -> Result<String, tikv_jemalloc_ctl::Error> {
    // jemalloc caches the stats; advancing the epoch refreshes the snapshot.
    epoch::advance()?;

    let allocated = stats::allocated::read()?;
    let active = stats::active::read()?;
    let resident = stats::resident::read()?;

    let stats = JemallocStats {
        narenas: arenas::narenas::read()?,
        allocated,
        active,
        resident,
        mapped: stats::mapped::read()?,
        retained: stats::retained::read()?,
        metadata: stats::metadata::read()?,
        fragmentation: active.saturating_sub(allocated),
        overhead: resident.saturating_sub(allocated),
    };

    // Serializing a struct of primitives cannot fail.
    let mut report =
        serde_json::to_string_pretty(&stats).expect("jemalloc stats serialize infallibly");
    report.push('\n');
    Ok(report)
}

async fn read_line(socket: &mut UnixStream) -> Option<String> {
    let mut reader = BufReader::new(socket);
    let mut buffer = String::new();
    reader.read_line(&mut buffer).await.ok()?;
    Some(buffer.trim().to_owned())
}
