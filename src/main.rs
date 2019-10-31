use replicative::*;
use std::collections::HashSet;

use futures::{executor::block_on, StreamExt};

fn main() {
    let mut set = Grow::<HashSet<_>>::new();
    set.insert(Leaf::new("one".to_string()));
    let mut set_two = Grow::<HashSet<_>>::new();
    set_two.insert(Leaf::new("two".to_string()));

    block_on(async {
        while let Some(op) = set.next().await {
            set_two.apply(op);
        }
    });

    println!("{:?}", set);
    println!("{:?}", set_two);
}
