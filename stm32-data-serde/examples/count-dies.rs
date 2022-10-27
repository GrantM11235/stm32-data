use std::collections::HashSet;
use std::fs;
use std::path::Path;

fn main() {
    let start = std::time::Instant::now();

    let (count, dies) = Path::new("../data/chips/")
        .read_dir()
        .unwrap()
        .map(|chip| {
            let file = fs::read(chip.unwrap().path()).unwrap();
            let parsed: stm32_data_serde::Chip = serde_json::from_slice(&file).unwrap();
            assert!(!parsed.die.is_empty());
            parsed.die
        })
        .fold((0, HashSet::new()), |(count, mut dies), die| {
            dies.insert(die);
            (count + 1, dies)
        });

    let num_dies = dies.len();

    let duration = start.elapsed();

    println!("Found {num_dies} unique dies in {count} json files in {duration:?}");
}
