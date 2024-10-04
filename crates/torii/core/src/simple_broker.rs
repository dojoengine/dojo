use std::any::{Any, TypeId};
use std::collections::{HashMap, VecDeque};
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Mutex;
use std::task::{Context, Poll};

use futures_channel::mpsc::{self, UnboundedReceiver, UnboundedSender};
use futures_util::{Stream, StreamExt};
use once_cell::sync::Lazy;
use slab::Slab;

static SUBSCRIBERS: Lazy<Mutex<HashMap<TypeId, Box<dyn Any + Send>>>> = Lazy::new(Default::default);

#[derive(Debug)]
pub struct Senders<T> {
    pub slab: Slab<UnboundedSender<T>>,
    pub message_queue: VecDeque<T>,
}

struct BrokerStream<T: Sync + Send + Clone + 'static>(usize, UnboundedReceiver<T>);

fn with_senders<T, F, R>(f: F) -> R
where
    T: Sync + Send + Clone + 'static,
    F: FnOnce(&mut Senders<T>) -> R,
{
    let mut map = SUBSCRIBERS.lock().unwrap();
    let senders = map
        .entry(TypeId::of::<Senders<T>>())
        .or_insert_with(|| Box::new(Senders::<T> {
            slab: Default::default(),
            message_queue: VecDeque::new(),
        }));
    f(senders.downcast_mut::<Senders<T>>().unwrap())
}

impl<T: Sync + Send + Clone + 'static> Drop for BrokerStream<T> {
    fn drop(&mut self) {
        with_senders::<T, _, _>(|senders| senders.slab.remove(self.0));
    }
}

impl<T: Sync + Send + Clone + 'static> Stream for BrokerStream<T> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.1.poll_next_unpin(cx)
    }
}

#[derive(Debug)]
/// A simple broker based on memory
pub struct SimpleBroker<T>(PhantomData<T>);

impl<T: Sync + Send + Clone + 'static> SimpleBroker<T> {
    /// Publish a message that all subscription streams can receive.
    pub fn publish(msg: T) {
        with_senders::<T, _, _>(|senders| {
            senders.message_queue.push_back(msg.clone());
            Self::send_messages(senders);
        });
    }

    fn send_messages(senders: &mut Senders<T>) {
        while let Some(msg) = senders.message_queue.pop_front() {
            let mut failed_senders = Vec::new();
            for (id, sender) in senders.slab.iter_mut() {
                if sender.start_send(msg.clone()).is_err() {
                    failed_senders.push(id);
                }
            }
            for id in failed_senders {
                senders.slab.remove(id);
            }
        }
    }

    /// Subscribe to the message of the specified type and returns a `Stream`.
    pub fn subscribe() -> impl Stream<Item = T> {
        with_senders::<T, _, _>(|senders| {
            let (tx, rx) = mpsc::unbounded();
            let id = senders.slab.insert(tx);
            BrokerStream(id, rx)
        })
    }

    /// Execute the given function with the _subscribers_ of the specified subscription type.
    pub fn with_subscribers<F, R>(f: F) -> R
    where
        T: Sync + Send + Clone + 'static,
        F: FnOnce(&mut Senders<T>) -> R,
    {
        with_senders(f)
    }
}
