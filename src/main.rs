use replicative::{register::Register, Replicant};

use futures::{executor::ThreadPool, StreamExt};

fn main() {
    let mut register = Register::new("test");
    register.set("hello");

    let mut replicant = Replicant::new(&mut register, 1);
    ThreadPool::new().unwrap().spawn_ok(async move {
        while let Some(action) = replicant.next().await {
            println!("{:?}", action);
        }
    });
    register.set("gamer");
}
