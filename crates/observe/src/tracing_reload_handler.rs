use {
    tokio::{
        io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
        net::{UnixListener, UnixStream},
    },
    tracing_subscriber::{reload, EnvFilter},
};

/// Spawns a new thread that listens for connections to a UNIX socket
/// at "/tmp/log_filter_override_<process_name>_<pid>".
/// Whenever a line gets written to that socket the reload handler
/// uses it as the new log filter.
/// To reset to the original log filter send the message "reset".
pub(crate) fn spawn_reload_handler<T: 'static>(
    initial_filter: String,
    reload_handle: reload::Handle<EnvFilter, T>,
) {
    tokio::spawn(async move {
        let id = std::process::id();
        let name = binary_name().unwrap_or_default();

        let socket_path = format!("/tmp/log_filter_override_{name}_{id}.sock");
        tracing::warn!(file = socket_path, "open log filter reload socket");
        let handle = SocketHandle {
            listener: match UnixListener::bind(&socket_path) {
                Ok(sock) => sock,
                Err(e) => match e.kind() {
                    std::io::ErrorKind::AddrInUse => {
                        tracing::warn!("log filter socket file already exists - removing");
                        if let Err(err) = std::fs::remove_file(&socket_path) {
                            tracing::warn!(
                                ?err,
                                file = socket_path,
                                "failed to remove log filter socket"
                            );
                        }
                        UnixListener::bind(&socket_path).expect("socket handle is unique")
                    }
                    _ => panic!("failed to create socket handle: {e:?}"),
                },
            },
            socket_path,
        };

        loop {
            handle_connection(&handle.listener, &initial_filter, &reload_handle).await;
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

async fn handle_connection<T>(
    listener: &UnixListener,
    initial_filter: &str,
    reload_handle: &reload::Handle<EnvFilter, T>,
) {
    let Ok((mut socket, _addr)) = listener.accept().await else {
        tracing::warn!("failed to accept UNIX socket connection");
        return;
    };

    let _ = socket
        .write_all(format!("log filter on process startup was: {initial_filter:?}\n",).as_bytes())
        .await;

    loop {
        let message = read_line(&mut socket).await;

        let filter = match message.as_deref() {
            Some("") => {
                log(&mut socket, "client terminated connection".into()).await;
                break;
            }
            None => {
                log(&mut socket, "failed to read message from socket".into()).await;
                continue;
            }
            Some("reset") => initial_filter,
            Some(message) => message,
        };

        let Ok(env_filter) = EnvFilter::try_new(filter) else {
            log(&mut socket, format!("failed to parse filter: {filter:?}")).await;
            continue;
        };

        match reload_handle.reload(env_filter) {
            Ok(_) => log(&mut socket, format!("applied new filter: {filter:?}")).await,
            Err(err) => log(&mut socket, format!("failed to apply filter: {err:?}")).await,
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
    // Use a fairly high log level to improve chances that this actually gets logged
    // when somebody messed with the log filter.
    tracing::warn!(message);
    let _ = socket.write_all(message.as_bytes()).await;
    let _ = socket.write_all(b"\n").await;
}
