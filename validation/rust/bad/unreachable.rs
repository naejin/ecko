// Expected: exit 1, check=unreachable-code
fn example() -> i32 {
    return 42;
    println!("dead");
    0
}

fn main() {
    let _ = example();
}
