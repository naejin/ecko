// Expected: exit 0
use std::collections::HashMap;

fn merge<K, V>(base: HashMap<K, V>, overrides: HashMap<K, V>) -> HashMap<K, V>
where
    K: std::hash::Hash + Eq,
{
    let mut result = base;
    for (k, v) in overrides {
        result.insert(k, v);
    }
    result
}

fn main() {
    let a = HashMap::from([("a", 1), ("b", 2)]);
    let b = HashMap::from([("b", 3), ("c", 4)]);
    let merged = merge(a, b);
    println!("{:?}", merged);
}
