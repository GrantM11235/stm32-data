use std::fs;
use std::path::Path;

fn main() {
    use rayon::prelude::*;

    let start = std::time::Instant::now();

    let count = Path::new("../data/chips")
        .read_dir()
        .unwrap()
        .par_bridge()
        .map(|chip| {
            let path = chip.unwrap().path();
            let mut buffer = fs::read(&path).unwrap();
            let parsed: stm32_data_serde::Chip = serde_json::from_slice(&buffer).unwrap();
            buffer.clear();
            serde_json::to_writer_pretty(&mut buffer, &parsed).unwrap();
            fs::write(path, buffer).unwrap();
        })
        .count();

    let duration = start.elapsed();

    println!("Rewrote {count} json files in {duration:?}");
}
