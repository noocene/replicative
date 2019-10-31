use std::{cmp::max, collections::HashMap};

#[derive(Clone, Copy, Eq, PartialEq, Hash, Ord, PartialOrd, Debug)]
#[repr(transparent)]
pub struct Actor(u32);

impl Actor {
    pub(crate) fn new(actor: u32) -> Self {
        Actor(actor)
    }
    pub fn is_valid(&self) -> bool {
        self.0 != 0
    }
    pub fn is_invalid(&self) -> bool {
        self.0 == 0
    }
    pub fn invalid() -> Self {
        Actor(0)
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Hash, Ord, PartialOrd, Debug)]
#[repr(transparent)]
pub struct Moment(u32);

impl Moment {
    fn increment(&mut self) {
        self.0 += 1;
    }
    pub fn new() -> Self {
        Moment(1)
    }
}

#[derive(Clone, Eq, PartialEq, Hash, Ord, PartialOrd, Debug)]
pub struct Shard(pub Actor, pub Moment);

impl Shard {
    #[inline]
    pub fn actor(&self) -> Actor {
        self.0
    }

    #[inline]
    pub fn moment(&self) -> Moment {
        self.1
    }
}

#[derive(Clone, Debug)]
pub struct Clock(HashMap<Actor, Moment>);

impl Clock {
    pub fn new() -> Self {
        Clock(HashMap::new())
    }
}

impl Shard {
    pub fn new(actor: Actor, moment: Moment) -> Self {
        Shard(actor, moment)
    }
}

impl Clock {
    pub fn get(&self, actor: Actor) -> Moment {
        *self.0.get(&actor).unwrap_or(&Moment(0))
    }

    pub fn get_shard(&mut self, actor: Actor) -> Shard {
        let counter = self.increment(actor);
        Shard(actor, counter)
    }

    pub fn increment(&mut self, actor: Actor) -> Moment {
        let entry = self.0.entry(actor).or_insert(Moment(0));
        entry.increment();
        *entry
    }

    pub fn contains(&self, shard: &Shard) -> bool {
        match self.0.get(&shard.0) {
            Some(moment) => *moment >= shard.1,
            None => false,
        }
    }

    pub fn insert(&mut self, shard: Shard) {
        let entry = self.0.entry(shard.0).or_insert(shard.1);
        *entry = max(*entry, shard.1);
    }

    pub fn merge(&mut self, other: &Clock) {
        for (actor, moment) in &other.0 {
            let entry = self.0.entry(*actor).or_insert(*moment);
            *entry = max(*entry, *moment);
        }
    }

    pub(crate) fn prepare(&mut self, actor: Actor) {
        if let Some(moment) = self.0.remove(&Actor::invalid()) {
            self.0.insert(actor, moment);
        }
    }
}
