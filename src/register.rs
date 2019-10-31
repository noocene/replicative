use crate::{
    clock::{Actor, Clock, Moment, Shard},
    Handle, Replicative,
};
use std::{
    collections::BTreeMap,
    fmt::Debug,
    mem::replace,
    sync::{Arc, Mutex},
};

#[derive(Debug)]
pub struct Op<T: Clone> {
    shard: Shard,
    data: T,
    removed: Vec<Shard>,
}

#[derive(Clone)]
struct Value<T: Clone> {
    data: T,
    latest: Moment,
}

enum Cache<T: Send + Debug + Clone + 'static> {
    Cache(Option<Op<T>>),
    Handle(Handle<Register<T>>),
}

impl<T: Debug + Clone + Send + 'static> Cache<T> {
    fn new() -> Self {
        Cache::Cache(None)
    }
    fn dispatch(&mut self, op: Op<T>) {
        match self {
            Cache::Cache(item) => {
                item.replace(op);
            }
            Cache::Handle(handle) => handle.dispatch(op),
        };
    }
    fn prepare(&mut self, mut handle: Handle<Register<T>>, local: Actor) {
        if let Cache::Cache(op) = self {
            if let Some(mut op) = op.take() {
                op.shard.0 = local;
                handle.dispatch(op)
            }
        }
        *self = Cache::Handle(handle)
    }
}

struct RegisterState<T: Send + Debug + Clone + 'static> {
    content: BTreeMap<Actor, Value<T>>,
    clock: Clock,
    local: Actor,
    handle: Cache<T>,
}

#[derive(Clone)]
pub struct Register<T: Send + Debug + Clone + 'static> {
    state: Arc<Mutex<RegisterState<T>>>,
}

impl<T: Clone + Debug + Send> Register<T> {
    pub fn new(data: T) -> Self {
        let local = Actor::invalid();
        let latest = Moment::new();
        let mut content = BTreeMap::new();
        let mut clock = Clock::new();
        content.insert(local, Value { data, latest });
        clock.insert(Shard(local, latest));
        Register {
            state: Arc::new(Mutex::new(RegisterState {
                content,
                clock,
                local,
                handle: Cache::new(),
            })),
        }
    }
    pub fn get(&self) -> T {
        self.state
            .lock()
            .unwrap()
            .content
            .values()
            .next()
            .unwrap()
            .data
            .clone()
    }
    pub fn set(&mut self, data: T) {
        let mut state = self.state.lock().unwrap();
        let local = state.local;
        let latest = state.clock.increment(local);
        let mut new_content = BTreeMap::new();
        new_content.insert(
            state.local,
            Value {
                data: data.clone(),
                latest,
            },
        );
        let removed = replace(&mut state.content, new_content)
            .into_iter()
            .filter_map(|(actor, value)| {
                if actor == state.local {
                    None
                } else {
                    Some(Shard(actor, value.latest))
                }
            })
            .collect();
        let local = state.local;
        state.handle.dispatch(Op {
            shard: Shard(local, latest),
            data,
            removed,
        })
    }
}

impl<T: Debug + Clone + Send + 'static> Replicative for Register<T> {
    type Op = Op<T>;

    fn prepare(&mut self, this: Actor, handle: Handle<Self>) {
        let mut state = self.state.lock().unwrap();
        state.local = this;
        state.clock.prepare(this);
        if let Some(value) = state.content.remove(&Actor::invalid()) {
            state.content.insert(this, value);
        }
        state.handle.prepare(handle, this);
    }
    fn apply(&mut self, op: Self::Op) {
        let mut state = self.state.lock().unwrap();
        for Shard(actor, latest) in op.removed {
            if let Some(value) = state.content.remove(&actor) {
                if value.latest > latest {
                    state.content.insert(actor, value);
                }
            }
        }
        state.clock.insert(op.shard.clone());
        let data = Value {
            data: op.data,
            latest: op.shard.moment(),
        };
        if let Some(existing) = state.content.insert(op.shard.actor(), data) {
            if existing.latest > op.shard.moment() {
                state.content.insert(op.shard.actor(), existing);
            }
        }
    }
}
