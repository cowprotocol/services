use {
    std::{
        io::{BufRead, BufReader, Write},
        os::unix::net::{UnixListener, UnixStream},
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
    std::thread::spawn(move || {
        let id = std::process::id();
        let name = std::env::current_exe()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();

        let socket_handle = format!("/tmp/log_filter_override_{name}_{id}.sock");
        tracing::warn!(addr = socket_handle, "open log filter reload socket");
        let listener = UnixListener::bind(socket_handle).unwrap();

        loop {
            handle_connection(&listener, &initial_filter, &reload_handle);
        }
    });
}

fn handle_connection(
    listener: &UnixListener,
    initial_filter: &str,
    reload_handle: &reload::Handle<EnvFilter, Registry>,
) {
    let (mut socket, _addr) = listener.accept().unwrap();
    let _ = socket
        .write_all(format!("log filter on process startup was: {initial_filter:?}\n",).as_bytes());

    loop {
        let message = read_line(&mut socket);

        let filter = match message.as_str() {
            "reset" => initial_filter,
            _ => &message,
        };

        let Ok(env_filter) = EnvFilter::try_new(filter) else {
            log(&mut socket, format!("failed to parse filter: {filter:?}"));
            continue;
        };

        match reload_handle.reload(env_filter) {
            Ok(_) => log(&mut socket, format!("applied new filter: {filter:?}")),
            Err(err) => log(&mut socket, format!("failed to apply filter: {err:?}")),
        }
    }
}

fn read_line(socket: &mut UnixStream) -> String {
    let mut reader = BufReader::new(socket);
    let mut buffer = String::new();
    reader.read_line(&mut buffer).unwrap();
    buffer.trim().to_owned()
}

/// Logs the message in this process' logs and reports it back to the
/// connected socket.
fn log(socket: &mut UnixStream, message: String) {
    // Use a fairly high log level to improve chances that this actually gets logged
    // when somebody messed with the log filter.
    tracing::warn!(message);
    let _ = socket.write_all(message.as_bytes());
    let _ = socket.write_all(b"\n");
}
