use std::{
    collections::HashMap,
    env,
    fs::{self, File, OpenOptions},
    io::{self, BufRead, BufReader},
};

const DEFAULT_FILENAME: &str = "crates/benches/gas_usage.txt";

fn main() {
    let filename = env::args().nth(1).unwrap_or_else(|| DEFAULT_FILENAME.into());

    let file = match OpenOptions::new().create(false).read(true).open(&filename) {
        Ok(file) => file,
        Err(err) => {
            eprintln!("Failed to open file {}: {}", filename, err);
            std::process::exit(1);
        }
    };

    let reader = BufReader::new(file);
    let mut map: HashMap<String, Vec<(u64, Option<String>)>> = HashMap::new();

    for (line_number, line) in reader.lines().enumerate() {
        let line = match line {
            Ok(line) => line,
            Err(err) => {
                eprintln!("Error reading line {}: {}", line_number + 1, err);
                continue;
            }
        };

        let segments: Vec<_> = line.split('\t').take(3).collect();

        let (name, gas, calldata) = match segments.len() {
            3 => (segments[0], segments[1], Some(String::from(segments[2]))),
            2 => (segments[0], segments[1], None),
            _ => {
                eprintln!("Invalid line format: {}", line);
                continue;
            }
        };

        let gas = match gas.split_whitespace().nth(1) {
            Some(gas) => match gas.parse::<u64>() {
                Ok(gas) => gas,
                Err(err) => {
                    eprintln!("Error parsing gas value in line {}: {}", line_number + 1, err);
                    continue;
                }
            },
            None => {
                eprintln!("Invalid gas format in line {}", line_number + 1);
                continue;
            }
        };

        map.entry(name.to_string()).or_default().push((gas, calldata));
    }

    let mut pairs = map.into_iter().collect::<Vec<_>>();
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
        let max_calldata = if let Some(calldata) = calldata.last().cloned().flatten() {
            format!(" for {}", calldata)
        } else {
            String::new()
        };

        println!("\tmin: {}{}", gas[0], min_calldata);
        println!("\tmax: {}{}", gas[gas.len() - 1], max_calldata);
        println!(
            "\taverage: {}",
            gas.iter().sum::<u64>() / gas.len() as u64
        );
        println!("\tmedian: {}", gas[gas.len() / 2]);
    }
}
