// Expected: exit 1, check=todo-macro (NOT placeholder-code)
// todo!() inside impl method is caught by todo-macro, not placeholder-code.
struct Processor;

impl Processor {
    fn run(&self) {
        todo!()
    }
}

fn main() {
    let p = Processor;
    let _ = &p;
}
