use crate::{
    clock::{Actor, Clock, Moment, Shard},
    Handle, Replicative,
};
use std::{borrow::Cow, collections::BTreeMap, fmt::Debug, mem::replace};

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

pub struct Register<T: Send + Debug + Clone + 'static> {
    state: State<T>,
    local: Actor,
    handle: Cache<T>,
}

#[derive(Clone)]
pub struct State<T: Clone> {
    content: BTreeMap<Actor, Value<T>>,
    clock: Clock,
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
            state: State { content, clock },
            local,
            handle: Cache::new(),
        }
    }
    pub fn get(&self) -> T {
        self.state.content.values().next().unwrap().data.clone()
    }
    pub fn set(&mut self, data: T) {
        let local = self.local;
        let latest = self.state.clock.increment(local);
        let mut new_content = BTreeMap::new();
        new_content.insert(
            self.local,
            Value {
                data: data.clone(),
                latest,
            },
        );
        let removed = replace(&mut self.state.content, new_content)
            .into_iter()
            .filter_map(|(actor, value)| {
                if actor == self.local {
                    None
                } else {
                    Some(Shard(actor, value.latest))
                }
            })
            .collect();
        let local = self.local;
        self.handle.dispatch(Op {
            shard: Shard(local, latest),
            data,
            removed,
        })
    }
}

impl<T: Debug + Clone + Send + 'static> Replicative for Register<T> {
    type Op = Op<T>;
    type State = State<T>;

    fn prepare(&mut self, this: Actor, handle: Handle<Self>) {
        self.local = this;
        self.state.clock.prepare(this);
        if let Some(value) = self.state.content.remove(&Actor::invalid()) {
            self.state.content.insert(this, value);
        }
        self.handle.prepare(handle, this);
    }
    fn apply(&mut self, op: Self::Op) {
        for Shard(actor, latest) in op.removed {
            if let Some(value) = self.state.content.remove(&actor) {
                if value.latest > latest {
                    self.state.content.insert(actor, value);
                }
            }
        }
        self.state.clock.insert(op.shard.clone());
        let data = Value {
            data: op.data,
            latest: op.shard.moment(),
        };
        if let Some(existing) = self.state.content.insert(op.shard.actor(), data) {
            if existing.latest > op.shard.moment() {
                self.state.content.insert(op.shard.actor(), existing);
            }
        }
    }
    fn merge(&mut self, state: Self::State) {}
    fn fetch<'a>(&'a self) -> Cow<'a, Self::State> {
        Cow::Borrowed(&self.state)
    }
}
