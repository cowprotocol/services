use {
    tokio::{
        io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
        net::{UnixListener, UnixStream},
    },
    tracing_subscriber::{reload, EnvFilter, Registry},
};

/// Spawns a new thread that listens for connections to a UNIX socket
/// at "/tmp/log_filter_override_<process_name>_<pid>".
/// Whenever a line gets writtedn to that socket the reload handler
/// uses it as the new log filter.
/// To reset to the original log filter send the message "reset".
pub(crate) fn spawn_reload_handler(
    initial_filter: String,
    reload_handle: reload::Handle<EnvFilter, Registry>,
) {
    tokio::spawn(async move {
        let id = std::process::id();
        let name = binary_name().unwrap_or_default();

        let socket_handle = format!("/tmp/log_filter_override_{name}_{id}.sock");
        tracing::warn!(addr = socket_handle, "open log filter reload socket");
        let listener = UnixListener::bind(socket_handle).expect("socket handle is unique");

        loop {
            handle_connection(&listener, &initial_filter, &reload_handle).await;
        }
    });
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

async fn handle_connection(
    listener: &UnixListener,
    initial_filter: &str,
    reload_handle: &reload::Handle<EnvFilter, Registry>,
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
            None => {
                log(&mut socket, "could not read message from socket".into()).await;
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
