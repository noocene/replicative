use replicative::{register::Register, Replicant};

use futures::{executor::ThreadPool, StreamExt, SinkExt};

fn main() {
    let mut register = Register::new("test");
    let mut register2 = Register::new("test");
    register.set("hello");

    let mut replicant = Replicant::new(&mut register, 1);
    let mut replicant2 = Replicant::new(&mut register2, 2);
    ThreadPool::new().unwrap().spawn_ok(async move {
        while let Some(action) = replicant.next().await {
            println!("{:?}", action);
            replicant2.send(action).await.unwrap();

        }
    });
    register.set("gamer");
}
