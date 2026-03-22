// Expected: exit 0
// Comments after return; in if blocks are not unreachable code.
fn append(cwd: &str) {
    if cwd.is_empty() {
        return; // Can't create directory -- skip
    }
    println!("continuing");
}

fn prune(total: usize, active: usize) {
    if total == 0 || active * 2 >= total {
        return; // Not enough stale entries
    }
    println!("pruning");
}

fn main() {
    append(".");
    prune(10, 8);
}
