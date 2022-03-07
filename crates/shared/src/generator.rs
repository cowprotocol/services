use futures::{channel::mpsc, stream::FusedStream, SinkExt, Stream};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

/// Creates a stream from an async generator function.
///
/// This is similar to [`unfold`](https://docs.rs/futures/0.3.21/futures/stream/fn.unfold.html) in
/// the sense that it bridges Future to Stream. In addition, it can be more ergonomic because it
/// allows the compiler to automatically create the state machine that you would otherwise have to
/// write by hand in the same way that async/await syntax makes it easier to create a Future.
///
/// The generator takes an argument `Sender` who's `send` function is used to yield values to the
/// Stream.
///
/// The generator must ensure that the sender does not outlive the generator. This is usually the
/// case but could be violated by leaking the sender or moving it into a global task. If you want to
/// use a task then you do not need this function because you can communicate over a channel
/// directly.
pub fn async_generator_to_stream<T, Fut>(
    generator: impl FnOnce(Sender<T>) -> Fut,
) -> impl Stream<Item = T> + FusedStream
where
    Fut: Future<Output = ()>,
{
    let (sender, receiver) = mpsc::channel(0);
    let generator = generator(Sender(sender));
    StreamAndFuture::new(receiver, Some(generator))
}

pub struct Sender<T>(mpsc::Sender<T>);

impl<T> Sender<T> {
    /// Yields a value from the generator into the stream.
    pub async fn send(&mut self, t: T) {
        // We assume that the receiver has not been dropped so sending cannot fail.
        // We use SinkExt::send instead of SinkExt::feed. `send` blocks until the value
        // has been received (read from the stream). This works because the channel has 0 capacity
        // and counts as flushed only when it has space for at least 1 message.
        // `feed` would send the value without waiting for it to be read. This would make the the
        // generator future calling this function progress too early.
        self.0.send(t).await.unwrap();
    }
}

/// Forward items from the stream while simultaneously running the future.
///
/// This can be useful when the future is in some way tied to the stream.
///
/// The stream ends when the inner stream ends. This means the future is dropped at this point.
#[pin_project::pin_project]
pub struct StreamAndFuture<St, Fut> {
    // future should come first in the struct so that it gets dropped first which protects against
    // weird Future implementations that try to use Sender on drop.
    #[pin]
    future: Option<Fut>,
    #[pin]
    stream: St,
}

impl<St, Fut> StreamAndFuture<St, Fut> {
    pub fn new(stream: St, future: Option<Fut>) -> Self {
        Self { stream, future }
    }
}

impl<T, St, Fut> Stream for StreamAndFuture<St, Fut>
where
    St: Stream<Item = T>,
    Fut: Future<Output = ()>,
{
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some(fut) = self.as_mut().project().future.as_pin_mut() {
            match fut.poll(cx) {
                Poll::Ready(()) => self.as_mut().project().future.set(None),
                Poll::Pending => (),
            }
        }
        self.project().stream.poll_next(cx)
    }
}

impl<St, Fut> FusedStream for StreamAndFuture<St, Fut>
where
    St: FusedStream,
    Fut: Future<Output = ()>,
{
    fn is_terminated(&self) -> bool {
        self.stream.is_terminated()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{FutureExt, StreamExt};

    #[test]
    fn works() {
        async fn generator(mut sender: Sender<usize>) {
            for i in 0..3 {
                sender.send(i).await;
            }
        }
        let stream = async_generator_to_stream(|sender| async { generator(sender).await });
        futures::pin_mut!(stream);
        assert_eq!(stream.next().now_or_never().unwrap(), Some(0));
        assert_eq!(stream.next().now_or_never().unwrap(), Some(1));
        assert_eq!(stream.next().now_or_never().unwrap(), Some(2));
        assert_eq!(stream.next().now_or_never().unwrap(), None);
    }

    #[test]
    fn fused() {
        async fn generator(_: Sender<usize>) {}
        let stream = async_generator_to_stream(generator);
        futures::pin_mut!(stream);
        assert!(!stream.is_terminated());
        assert_eq!(stream.next().now_or_never().unwrap(), None);
        assert!(stream.is_terminated());
        assert_eq!(stream.next().now_or_never().unwrap(), None);
    }

    #[test]
    fn pending() {
        async fn generator(_: Sender<usize>) {
            futures::future::pending().await
        }
        let stream = async_generator_to_stream(generator);
        futures::pin_mut!(stream);
        assert_eq!(stream.next().now_or_never(), None);
    }

    #[test]
    fn unpin() {
        let mut stream = async_generator_to_stream(|_: Sender<()>| futures::future::pending());
        // Can call StreamExt::next without pin_mut because the generator future is Unpin.
        let _ = stream.next();
    }

    #[test]
    fn weird_future() {
        struct S(Sender<()>);
        impl Future for S {
            type Output = ();
            fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
                Poll::Pending
            }
        }
        impl Drop for S {
            fn drop(&mut self) {
                // Pending because Sender::send should block until the value has been read.
                assert!(self.0.send(()).now_or_never().is_none());
            }
        }
        let stream = async_generator_to_stream(S);
        // This line panics if the future is dropped before the stream in StreamAndFuture.
        std::mem::drop(stream);
    }
}
