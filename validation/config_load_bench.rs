use std::{env, hint::black_box, path::Path, time::Instant};

use serde_json::Map;
use tokeneconomizer::plugin::config::load_config;

fn main() {
    let root = env::args_os()
        .nth(1)
        .expect("configuration directory required");
    let root = Path::new(&root);
    let repeats: u128 = env::args()
        .nth(2)
        .expect("repeat count required")
        .parse()
        .unwrap();
    assert!(repeats > 0);
    let environment = Map::new();
    let expected = load_config(root, None, &environment);
    println!("{}", serde_json::to_string(&expected).unwrap());

    for _ in 0..100 {
        black_box(load_config(black_box(root), None, black_box(&environment)));
    }

    let start = Instant::now();
    for _ in 0..repeats {
        black_box(load_config(black_box(root), None, black_box(&environment)));
    }
    println!("{}", start.elapsed().as_nanos() / repeats);
}
