use std::collections::hash_map::{DefaultHasher, Entry};
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;

use serde::Serialize;

mod deduped {
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
    pub struct Chip {
        pub name: String,
        pub family: String,
        pub line: String,
        pub die: String,
        pub device_id: u16,
        pub packages: Vec<stm32_data_serde::chip::Package>,
        pub memory: Vec<stm32_data_serde::chip::Memory>,
        pub docs: Vec<String>,
        pub cores: Vec<chip::Core>,
    }

    pub mod chip {
        use serde::{Deserialize, Serialize};

        #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
        pub struct Core {
            pub name: String,
            pub peripheral_hashes: Vec<u64>,
            pub interrupts_hash: u64,
            pub dma_channels_hash: u64,
        }
    }
}

struct Interner<T> {
    hashmap: HashMap<u64, T>,
    duplicates: usize,
}

impl<T: Hash + Debug + PartialEq + Serialize> Interner<T> {
    fn new() -> Self {
        Self {
            hashmap: HashMap::new(),
            duplicates: 0,
        }
    }
    fn insert(&mut self, hash: u64, item: T) {
        match self.hashmap.entry(hash) {
            Entry::Occupied(entry) => {
                assert_eq!(entry.get(), &item, "checking for hash collision");
                self.duplicates += 1;
            }
            Entry::Vacant(entry) => {
                entry.insert(item);
            }
        }
    }

    fn intern(&mut self, item: T) -> u64 {
        let mut hasher = DefaultHasher::new();
        item.hash(&mut hasher);
        let hash = hasher.finish();
        self.insert(hash, item);
        hash
    }

    fn dump_hashmap(&self, path: impl AsRef<Path>) {
        println!(
            "{}: Found {} duplicates, {} unique",
            std::any::type_name::<T>(),
            self.duplicates,
            self.hashmap.len()
        );
        for (hash, data) in &self.hashmap {
            let mut filename = String::new();
            use std::fmt::Write;
            write!(filename, "{hash}.json").unwrap();
            let filename = path.as_ref().join(filename);
            let data = serde_json::to_vec_pretty(&data).unwrap();
            fs::write(filename, data).unwrap()
        }
    }

    fn combine(&mut self, other: Self) {
        let Self { hashmap, duplicates } = other;

        for (hash, item) in hashmap {
            self.insert(hash, item);
        }

        self.duplicates += duplicates
    }
}

struct Shared {
    docs: HashSet<stm32_data_serde::chip::Doc>,
    peripherals: Interner<stm32_data_serde::chip::core::Peripheral>,
    interrupts: Interner<Vec<stm32_data_serde::chip::core::Interrupt>>,
    dma_channels: Interner<Vec<stm32_data_serde::chip::core::DmaChannels>>,
}

impl Shared {
    fn new() -> Self {
        Self {
            docs: HashSet::new(),
            peripherals: Interner::new(),
            interrupts: Interner::new(),
            dma_channels: Interner::new(),
        }
    }

    fn dedup(&mut self, chip: stm32_data_serde::Chip) -> deduped::Chip {
        let stm32_data_serde::Chip {
            name,
            family,
            line,
            die,
            device_id,
            packages,
            memory,
            docs,
            cores,
        } = chip;

        let mut docs: Vec<_> = docs
            .into_iter()
            .map(|doc| {
                let name = doc.name.clone();
                self.docs.insert(doc);
                name
            })
            .collect();
        docs.sort_unstable();

        let cores = cores
            .into_iter()
            .map(|core| {
                let stm32_data_serde::chip::Core {
                    name,
                    peripherals,
                    interrupts,
                    dma_channels,
                } = core;
                let peripheral_hashes = peripherals
                    .into_iter()
                    .map(|peripheral| self.peripherals.intern(peripheral))
                    .collect();
                deduped::chip::Core {
                    name,
                    peripheral_hashes,
                    interrupts_hash: self.interrupts.intern(interrupts),
                    dma_channels_hash: self.dma_channels.intern(dma_channels),
                }
            })
            .collect();

        deduped::Chip {
            name,
            family,
            line,
            die,
            device_id,
            packages,
            memory,
            docs,
            cores,
        }
    }

    fn dump(
        &self,
        docs_dir: impl AsRef<Path>,
        peripherals_dir: impl AsRef<Path>,
        interrupts_dir: impl AsRef<Path>,
        dma_channels_dir: impl AsRef<Path>,
    ) {
        let Self {
            docs,
            peripherals,
            interrupts,
            dma_channels,
        } = self;
        let mut docs: Vec<_> = docs.iter().cloned().collect();
        docs.sort_unstable_by_key(|doc| doc.name.clone());
        fs::write(
            docs_dir.as_ref().join("docs.json"),
            serde_json::to_vec_pretty(&docs).unwrap(),
        )
        .unwrap();
        peripherals.dump_hashmap(peripherals_dir);
        interrupts.dump_hashmap(interrupts_dir);
        dma_channels.dump_hashmap(dma_channels_dir);
    }

    fn combine(&mut self, other: Self) {
        let Self {
            docs,
            peripherals,
            interrupts,
            dma_channels,
        } = other;
        self.docs.extend(docs);
        self.peripherals.combine(peripherals);
        self.interrupts.combine(interrupts);
        self.dma_channels.combine(dma_channels);
    }
}

fn main() {
    use rayon::prelude::*;

    let output_dir = Path::new("output");
    fs::remove_dir_all(output_dir).ok();

    let new_dir = |dir| {
        let dir = output_dir.join(dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    };

    let docs_dir = output_dir;
    let chips_dir = new_dir("chips");
    let peripherals_dir = new_dir("peripherals");
    let interrupts_dir = new_dir("interrupts");
    let dma_channels_dir = new_dir("dma_channels");

    let start = std::time::Instant::now();

    let shared = Path::new("../data/chips")
        .read_dir()
        .unwrap()
        .par_bridge()
        .map(|dir_entry| fs::read(dir_entry.unwrap().path()).unwrap())
        .map(|file| serde_json::from_slice::<stm32_data_serde::Chip>(&file).unwrap())
        .fold(Shared::new, |mut shared, chip| {
            let chip = shared.dedup(chip);
            let filename = chips_dir.join(chip.name.clone() + ".json");
            let data = serde_json::to_vec_pretty(&chip).unwrap();
            fs::write(filename, data).unwrap();
            shared
        })
        .reduce(Shared::new, |mut a, b| {
            a.combine(b);
            a
        });

    shared.dump(docs_dir, peripherals_dir, interrupts_dir, dma_channels_dir);

    let duration = start.elapsed();

    println!("Done in {duration:?}");
}
