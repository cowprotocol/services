use {
    tokio::{
        io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
        net::{UnixListener, UnixStream},
    },
};

/// Spawns a new thread that listens for connections to a UNIX socket
/// at "/tmp/heap_dump_<process_name>_<pid>.sock".
/// When "dump" command is sent, it generates a heap profile using jemalloc_pprof
/// and streams the binary protobuf data back through the socket.
///
/// Usage:
/// ```bash
/// # From your local machine (one-liner):
/// kubectl exec orderbook-pod -- sh -c "echo dump | nc -U /tmp/heap_dump_orderbook_*.sock" > heap.pb
///
/// # Analyze with pprof:
/// go tool pprof -http=:8080 heap.pb
/// ```
pub fn spawn_heap_dump_handler() {
    tokio::spawn(async move {
        let id = std::process::id();
        let name = binary_name().unwrap_or_default();

        let socket_path = format!("/tmp/heap_dump_{name}_{id}.sock");
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

    let _ = socket
        .write_all(b"heap dump handler ready. send 'dump' to generate profile\n")
        .await;

    loop {
        let message = read_line(&mut socket).await;

        match message.as_deref() {
            Some("") => {
                log(&mut socket, "client terminated connection".into()).await;
                break;
            }
            None => {
                log(&mut socket, "failed to read message from socket".into()).await;
                continue;
            }
            Some("dump") => {
                generate_and_stream_dump(&mut socket).await;
                break; // Close connection after sending dump
            }
            Some(unknown) => {
                log(
                    &mut socket,
                    format!("unknown command: {unknown:?}. use 'dump'"),
                )
                .await;
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
            let error_msg = "jemalloc profiling not initialized\n";
            tracing::error!(error_msg);
            let _ = socket.write_all(error_msg.as_bytes()).await;
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
            let error_msg = format!("error generating heap dump: {e:?}\n");
            tracing::error!(error = ?e, "failed to generate heap dump");
            let _ = socket.write_all(error_msg.as_bytes()).await;
        }
    }
}

async fn read_line(socket: &mut UnixStream) -> Option<String> {
    let mut reader = BufReader::new(socket);
    let mut buffer = String::new();
    reader.read_line(&mut buffer).await.ok()?;
    Some(buffer.trim().to_owned())
}

/// Logs the message in this process' logs and reports it back to the
/// connected socket.
async fn log(socket: &mut UnixStream, message: String) {
    tracing::warn!(message);
    let _ = socket.write_all(message.as_bytes()).await;
    let _ = socket.write_all(b"\n").await;
}
