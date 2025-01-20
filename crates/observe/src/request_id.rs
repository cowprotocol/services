//! This module supplies the tools to associate 1 identifier with a task.
//! That identifier is accessable globally just for that task. The idea
//! is that this identifier is supposed to tie together related logs. That
//! is easy to accomplish in a single process (simply use a tracing span)
//! but if you want to tie together logs across multiple processes things
//! can get messier.
//! The most obvious option is to take that identifier and pass that through
//! the process until you make some request to another process and give that
//! process the identifier in your request.
//! However, if would do that the identifier would basically show up everywhere
//! although other components don't care about it and it doesn't even change
//! any behaviour in the process.
//! Instead we use task local storage that is globally visible but only
//! individual to each task. That way we can populate the storage with the
//! identifier once and not care about dragging it through the code base.
//! And when we issue requests to another process we can simply fetch the
//! current identifier specific to our task and send that along with the
//! request.
use {std::future::Future, tokio::task::JoinHandle};

tokio::task_local! {
    pub static REQUEST_ID: String;
}

/// Tries to read the `request_id` from this task's storage.
/// Returns `None` if task local storage was not initialized or is empty.
pub fn get_task_local_storage() -> Option<String> {
    let mut id = None;
    let _ = REQUEST_ID.try_with(|cell| {
        id = Some(cell.clone());
    });
    id
}

/// Sets the tasks's local id to the passed in value for the given scope.
pub async fn set_task_local_storage<F, R>(id: String, scope: F) -> R
where
    F: Future<Output = R>,
{
    REQUEST_ID.scope(id, scope).await
}

/// Spawns a new task and ensures it uses the same request id as the current
/// task (if present). This allows for tracing requests across task boundaries.
pub fn spawn_task_with_current_request_id<F>(future: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    if let Some(id) = get_task_local_storage() {
        tokio::task::spawn(set_task_local_storage(id, future))
    } else {
        tokio::task::spawn(future)
    }
}

/// Takes a `tower::Service` and embeds it in a `make_service` function that
/// spawns one of these services per incoming request.
/// But crucially before spawning that service task local storage will be
/// initialized with some request id.
/// Either that gets taken from the requests `X-REQUEST-ID` header of if that's
/// missing a globally unique request number will be generated.
#[macro_export]
macro_rules! make_service_with_task_local_storage {
    ($service:expr) => {{
        {
            let internal_request_id = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            hyper::service::make_service_fn(move |_| {
                let warp_svc = $service.clone();
                let internal_request_id = internal_request_id.clone();
                async move {
                    let svc =
                        hyper::service::service_fn(move |req: hyper::Request<hyper::Body>| {
                            let mut warp_svc = warp_svc.clone();
                            let id = if let Some(header) = req.headers().get("X-Request-ID") {
                                String::from_utf8_lossy(header.as_bytes()).to_string()
                            } else {
                                format!(
                                    "{}",
                                    internal_request_id
                                        .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                                )
                            };
                            let span = tracing::info_span!("request", id);
                            let handle_request = observe::request_id::set_task_local_storage(
                                id,
                                hyper::service::Service::call(&mut warp_svc, req),
                            );
                            tracing::Instrument::instrument(handle_request, span)
                        });
                    Ok::<_, std::convert::Infallible>(svc)
                }
            })
        }
    }};
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn request_id_copied_to_new_task() {
        // use channels to enforce that assertions happen in the desired order.
        // First we assert that the parent task's storage is empty after we
        // spawned the child task.
        // Afterwards we assert that the child task still has the parent task's
        // value at the time of spawning.
        let (sender1, receiver1) = tokio::sync::oneshot::channel();
        let (sender2, receiver2) = tokio::sync::oneshot::channel();

        spawn_task_with_current_request_id(async {
            assert_eq!(None, get_task_local_storage());
        })
        .await
        .unwrap();

        // create a task with some task local value
        let _ = set_task_local_storage("1234".into(), async {
            // spawn a new task that copies the parent's task local value
            spawn_task_with_current_request_id(async {
                receiver1.await.unwrap();
                assert_eq!(Some("1234".into()), get_task_local_storage());
                sender2.send(()).unwrap();
            });
        })
        .await;

        // task local value is not populated outside of the previous scope
        assert_eq!(None, get_task_local_storage());
        sender1.send(()).unwrap();

        // block test until the important assertion happened
        receiver2.await.unwrap();
    }
}
