use replicative::{register::Register, Replicant};

fn main() {
    let mut register = Register::new("test");
    register.set("hello");
    let _ = Replicant::new(&mut register, 1);
    register.set("gamer");
}
