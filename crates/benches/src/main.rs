use std::collections::HashMap;
use std::env;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader};

const DEFAULT_FILENAME: &str = "crates/benches/gas_usage.txt";
fn main() {
    let filename = env::args().nth(1).unwrap_or(DEFAULT_FILENAME.into());

    let file = OpenOptions::new().create(false).read(true).open(filename).expect(
        "Failed to open gas_usage.txt: run tests first with `cargo test bench -- --ignored` and \
         pass correct filename",
    );
    let reader = BufReader::new(file);

    let mut map: HashMap<String, Vec<(u64, Option<String>)>> = HashMap::new();

    // Collect info from all runs
    for line in reader.lines() {
        let line = line.unwrap();
        let segments = line.split('\t').take(3).collect::<Vec<_>>();

        let (name, gas, calldata) = match segments.len() {
            3 => (segments[0], segments[1], Some(String::from(segments[2]))),
            2 => (segments[0], segments[1], None),
            _ => panic!("Invalid line: {}", line),
        };

        let gas = gas.split(' ').nth(1).expect("Invalid gas format");
        let gas = gas.parse::<u64>().unwrap();

        if let Some(el) = map.get_mut(name) {
            el.push((gas, calldata));
        } else {
            map.insert(String::from(name), vec![(gas, calldata)]);
        }
    }

    let mut pairs = map.into_iter().map(|(name, runs)| (name, runs)).collect::<Vec<_>>();
    pairs.sort_by_key(|(key, _)| key.clone());

    for (name, mut runs) in pairs {
        runs.sort_by_key(|(gas, _)| *gas);
        let (gas, calldata): (Vec<_>, Vec<_>) = runs.into_iter().unzip();

        println!("{}:", name);

        if gas[0] == *gas.last().unwrap() {
            println!("\tconstant: {}", gas[0]);
            continue;
        }

        let min_calldata = if let Some(calldata) = calldata[0].clone() {
            format!(" for {}", calldata)
        } else {
            String::new()
        };
        let max_calldata = if let Some(calldata) = calldata[calldata.len() - 1].clone() {
            format!(" for {}", calldata)
        } else {
            String::new()
        };

        println!("\tmin: {}{}", gas[0], min_calldata);
        println!("\tmax: {}{}", gas[gas.len() - 1], max_calldata);
        println!("\taverage: {}", gas.iter().sum::<u64>() / gas.len() as u64);
        println!("\tmedian: {}", gas[gas.len() / 2]);
    }
}
