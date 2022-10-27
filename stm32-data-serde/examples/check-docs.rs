use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

fn main() {
    let start = std::time::Instant::now();

    let mut docs = HashMap::<_, HashSet<_>>::new();

    Path::new("../data/chips/")
        .read_dir()
        .unwrap()
        .flat_map(|chip| {
            let file = fs::read(chip.unwrap().path()).unwrap();
            let parsed: stm32_data_serde::Chip = serde_json::from_slice(&file).unwrap();
            parsed.docs
        })
        .for_each(|doc| {
            let entry = docs.entry(doc.name.clone());
            entry.or_default().insert(doc);
        });

    for (name, docs) in &docs {
        let len = docs.len();
        if len > 1 {
            panic!("There are {len} different versions of {name}");
        }
    }

    let duration = start.elapsed();
    println!("Found {} unique docs in {duration:?}", docs.len());
}
