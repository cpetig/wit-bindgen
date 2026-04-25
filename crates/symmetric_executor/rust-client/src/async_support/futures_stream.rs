use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use crate::async_support::StreamResult;

use super::StreamReader;

/// A wrapper around [`StreamReader`] that implements [`futures::Stream`].
///
/// Obtain one via [`StreamReader::into_stream`]
pub struct RawStreamReaderStream<O: 'static> {
    state: StreamAdapterState<O>,
}

// /// Convenience alias for the common vtable-based case.
//pub type StreamReaderStream<T> = StreamReader<T>;

type ReadNextFut<O> = Pin<Box<dyn Future<Output = (StreamReader<O>, StreamResult, Vec<O>)>>>;

enum StreamAdapterState<O: 'static> {
    /// The reader is idle and ready for the next read.
    Idle(StreamReader<O>),
    /// A read is in progress.
    Reading(ReadNextFut<O>),
    /// Results to draw from
    Results(StreamReader<O>, Vec<O>),
    /// The stream has been exhausted.
    Complete,
}

impl<O: Send + Unpin + 'static> futures::stream::Stream for RawStreamReaderStream<O> {
    type Item = O;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // All variants of `StreamAdapterState` are `Unpin`, so `Pin<&mut Self>`
        // can be freely projected.
        loop {
            match core::mem::replace(&mut self.state, StreamAdapterState::Complete) {
                StreamAdapterState::Idle(mut reader) => {
                    let fut: ReadNextFut<O> = Box::pin(async move {
                        let read = reader.read(Vec::with_capacity(3));
                        let (res, item) = read.await;
                        (reader, res, item)
                    });
                    self.state = StreamAdapterState::Reading(fut);
                    // Loop to immediately poll the new future.
                }
                StreamAdapterState::Reading(mut fut) => match fut.as_mut().poll(cx) {
                    Poll::Pending => {
                        self.state = StreamAdapterState::Reading(fut);
                        return Poll::Pending;
                    }
                    Poll::Ready((reader, StreamResult::Complete(_v), mut vec)) => {
                        if !vec.is_empty() {
                            let item = vec.remove(0);
                            self.state = StreamAdapterState::Results(reader, vec);
                            return Poll::Ready(Some(item));
                        } else {
                            self.state = StreamAdapterState::Idle(reader);
                        }
                    }
                    Poll::Ready((_reader, _, _vec)) => {
                        self.state = StreamAdapterState::Complete;
                        return Poll::Ready(None);
                    }
                },
                StreamAdapterState::Results(reader, mut vec) => {
                    if !vec.is_empty() {
                        let item = vec.remove(0);
                        self.state = StreamAdapterState::Results(reader, vec);
                        return Poll::Ready(Some(item));
                    } else {
                        self.state = StreamAdapterState::Idle(reader);
                    }
                }
                StreamAdapterState::Complete => {
                    self.state = StreamAdapterState::Complete;
                    return Poll::Ready(None);
                }
            }
        }
    }
}

impl<O: 'static> StreamReader<O> {
    /// Convert this reader into a [`futures::Stream`].
    pub fn into_stream(self) -> RawStreamReaderStream<O> {
        RawStreamReaderStream {
            state: StreamAdapterState::Idle(self),
        }
    }
}
