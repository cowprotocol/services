use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
};

/// Spawns a new thread that listens for connections to a UNIX socket
/// at "/tmp/heap_dump_<process_name>.sock".
/// When "dump" command is sent, it generates a heap profile using
/// jemalloc_pprof and streams the binary protobuf data back through the socket.
///
/// Usage:
/// ```bash
/// # From your local machine (one-liner):
/// kubectl exec orderbook-pod -- sh -c "echo dump | nc -U /tmp/heap_dump_orderbook.sock" > heap.pprof
///
/// # Analyze with pprof:
/// go tool pprof -http=:8080 heap.pprof
/// ```
pub fn spawn_heap_dump_handler() {
    tokio::spawn(async move {
        let name = binary_name().unwrap_or_default();

        let socket_path = format!("/tmp/heap_dump_{name}.sock");
        tracing::warn!(file = socket_path, "open heap dump socket");
        let _ = tokio::fs::remove_file(&socket_path).await;
        let handle = SocketHandle {
            listener: UnixListener::bind(&socket_path).expect("socket handle is unique"),
            socket_path,
        };

        loop {
            handle_connection(&handle.listener).await;
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

async fn handle_connection(listener: &UnixListener) {
    let Ok((mut socket, _addr)) = listener.accept().await else {
        tracing::warn!("failed to accept UNIX socket connection");
        return;
    };

    loop {
        let message = read_line(&mut socket).await;

        match message.as_deref() {
            Some("") => {
                tracing::debug!("client terminated connection");
                break;
            }
            None => {
                tracing::warn!("failed to read message from socket");
                break;
            }
            Some("dump") => {
                generate_and_stream_dump(&mut socket).await;
                break; // Close connection after sending dump
            }
            Some(unknown) => {
                tracing::warn!(?unknown, "unknown command received");
                break;
            }
        }
    }
}

async fn generate_and_stream_dump(socket: &mut UnixStream) {
    tracing::info!("generating heap dump via jemalloc_pprof");

    // Access the global profiling controller
    let prof_ctl = match jemalloc_pprof::PROF_CTL.as_ref() {
        Some(ctl) => ctl,
        None => {
            tracing::error!("jemalloc profiling not initialized");
            return;
        }
    };

    let mut prof_ctl = prof_ctl.lock().await;

    match prof_ctl.dump_pprof() {
        Ok(pprof_data) => {
            tracing::info!(size = pprof_data.len(), "heap dump generated");

            // Stream binary protobuf data directly
            if let Err(e) = socket.write_all(&pprof_data).await {
                tracing::warn!(error = ?e, "failed to write heap dump to socket");
            } else {
                tracing::info!("heap dump streamed successfully");
            }
        }
        Err(e) => {
            tracing::error!(error = ?e, "failed to generate heap dump");
        }
    }
}

async fn read_line(socket: &mut UnixStream) -> Option<String> {
    let mut reader = BufReader::new(socket);
    let mut buffer = String::new();
    reader.read_line(&mut buffer).await.ok()?;
    Some(buffer.trim().to_owned())
}
