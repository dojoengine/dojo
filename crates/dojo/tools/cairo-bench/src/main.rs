use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, Write};
use std::process::Command;

use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
use colored::Colorize;
use serde::{Deserialize, Serialize};

// just keep test functions starting with this prefix
const BENCH_TEST_FILTER: &str = "bench_";

// extract gas cost data using this prefix
const GAS_TAG: &str = "#GAS#";

const BETTER_TAG: &str = "[BETTER]";
const WORSE_TAG: &str = "[WORSE]";
const EQUAL_TAG: &str = "[EQUAL]";
const REMOVED_TAG: &str = "[REMOVED]";
const CREATED_TAG: &str = "[CREATED]";

// where to store reference files
const REF_DIR_PATH: &str = "crates/dojo/tools/cairo-bench/references";

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct TestCost {
    name: String,
    cost: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TestCompare {
    Created,
    Updated((u128, u128)),
    Removed,
}

#[derive(Parser, Debug)]
#[command(
    version,
    about = "Tool to benchmark Cairo tests.",
    long_about = "Tool for comparing Cairo test performance by measuring their gas cost. It \
                  allows tracking performance evolution over time by comparing results with a \
                  reference file.\n- Test functions must start with `bench_` to be taken into \
                  account.\n- Gas cost must follow the output format defined in \
                  GasCounterTrait::end_csv()."
)]
struct Args {
    #[arg(long, help = "Path to a Scarb.toml manifest file")]
    pub manifest_path: Option<Utf8PathBuf>,

    #[arg(
        short = 'u',
        long,
        help = "Override the current reference file attached to the provided manifest file."
    )]
    pub update_ref: bool,

    #[arg(short = 'c', long, help = "Just show what has changed.")]
    pub change_only: bool,

    #[arg(short = 'w', long, help = "Just show tests that got worse.")]
    pub worse_only: bool,

    #[arg(short = 's', long, help = "Show the scarb test output.")]
    pub show_scarb_output: bool,
}
/// Run tests tagged with the correct bench prefix for the provided manifest file.
fn execute_scarb(manifest_path: &str, show_scarb_output: bool) -> String {
    let args = ["--manifest-path", manifest_path, "test", BENCH_TEST_FILTER];

    println!("{}", "Running tests...".blue());

    let output = if show_scarb_output {
        let mut child = Command::new("scarb")
            .args(args)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap_or_else(|_| panic!("Failed to run scarb at path: {}.", manifest_path));

        let mut output = String::new();
        let stdout = child.stdout.take().expect("Failed to capture stdout");

        let mut reader = std::io::BufReader::new(stdout);
        let mut line = String::new();
        while reader.read_line(&mut line).unwrap() > 0 {
            print!("{}", line);
            output.push_str(&line);
            line.clear();
        }

        // Wait for the command to complete
        let status = child.wait().expect("Failed to wait for scarb");
        if !status.success() {
            panic!("Scarb command failed with exit code: {}", status);
        }

        output
    } else {
        let output = Command::new("scarb")
            .args(args)
            .output()
            .unwrap_or_else(|_| panic!("Failed to run scarb at path: {}.", manifest_path));

        String::from_utf8(output.stdout).unwrap()
    };

    output
}

/// Parse the scarb test output by extracting the gas cost data.
fn parse_output(output: &str) -> Vec<TestCost> {
    let mut tests: Vec<TestCost> = output
        .lines()
        .filter(|line| line.contains(GAS_TAG))
        .filter_map(|line| {
            let line = line.replace(GAS_TAG, "");
            let items = line.split(';').collect::<Vec<_>>();

            if items.len() == 2 {
                let name = items[0].to_string();
                let cost = items[1]
                    .parse::<u128>()
                    .unwrap_or_else(|_| panic!("Failed to convert {} to u128", items[1]));

                Some(TestCost { name, cost })
            } else {
                None
            }
        })
        .collect();

    tests.sort_by(|a, b| a.name.partial_cmp(&b.name).unwrap());
    tests
}

/// Get the root path of the git repository.
fn get_git_root_path() -> Utf8PathBuf {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .unwrap_or_else(|_| panic!("Failed to get git root path"));
    let root_path = String::from_utf8(output.stdout).expect("Failed to parse git root path");
    Utf8Path::new(root_path.trim()).to_path_buf()
}

/// Get the path to the reference file for the provided manifest file.
fn get_ref_file(manifest_path: &Utf8PathBuf) -> Utf8PathBuf {
    let root_path = get_git_root_path();
    let ref_root_path = root_path.join(REF_DIR_PATH);

    let manifest_path = manifest_path.canonicalize_utf8().unwrap();
    let manifest_dir = manifest_path.parent().unwrap();

    let manifest_dir = manifest_dir
        .strip_prefix(root_path)
        .expect("Failed to remove root path from manifest directory");

    let base_name =
        manifest_dir.components().map(|comp| comp.as_str()).collect::<Vec<&str>>().join("__");

    ref_root_path.join(format!("{base_name}__bench"))
}

/// Read the reference file and parse the test costs.
fn read_ref_tests(ref_file: &Utf8PathBuf) -> Vec<TestCost> {
    let content = fs::read_to_string(ref_file)
        .unwrap_or_else(|_| panic!("Failed to read reference file: {}", ref_file));

    serde_json::from_str(&content)
        .unwrap_or_else(|_| panic!("Failed to parse reference file: {}", ref_file))
}

/// Write the test costs to the reference file.
fn write_ref_file(ref_file: &Utf8PathBuf, tests: Vec<TestCost>) {
    let ref_dir = ref_file.parent().unwrap();

    if !ref_dir.exists() {
        fs::create_dir_all(ref_dir).expect("Failed to create references directory");
    }

    let mut file = fs::File::create(ref_file)
        .unwrap_or_else(|_| panic!("Failed to create the reference file: {}", ref_file));

    let tests_json = serde_json::to_string_pretty(&tests).unwrap();

    println!("{}", format!("Writing reference file: {}", ref_file).blue());
    file.write_all(tests_json.as_bytes())
        .unwrap_or_else(|_| panic!("Failed to write reference file: {}", ref_file));
}

/// Get the path to the manifest file.
fn get_manifest_path(manifest_path: Option<Utf8PathBuf>) -> Utf8PathBuf {
    if let Some(manifest_path) = manifest_path {
        manifest_path.clone()
    } else {
        let current_dir = Utf8PathBuf::from_path_buf(std::env::current_dir().unwrap())
            .expect("Invalid UTF-8 path");
        current_dir.join("Scarb.toml")
    }
}

/// Compare the reference tests with the new tests and return a map of test names to their
/// comparison status.
fn compare_tests(
    ref_tests: &Vec<TestCost>,
    new_tests: &Vec<TestCost>,
) -> HashMap<String, TestCompare> {
    let mut result = HashMap::<String, TestCompare>::new();

    let mut ref_it = ref_tests.iter();
    let mut new_it = new_tests.iter();

    let mut ref_test = ref_it.next();
    let mut new_test = new_it.next();

    while ref_test.is_some() && new_test.is_some() {
        let ref_test_value = ref_test.unwrap();
        let new_test_value = new_test.unwrap();

        match ref_test_value.name.partial_cmp(&new_test_value.name).unwrap() {
            Ordering::Less => {
                result.insert(ref_test_value.name.clone(), TestCompare::Removed);
                ref_test = ref_it.next();
            }
            Ordering::Greater => {
                result.insert(new_test_value.name.clone(), TestCompare::Created);
                new_test = new_it.next();
            }
            Ordering::Equal => {
                result.insert(
                    ref_test_value.name.clone(),
                    TestCompare::Updated((ref_test_value.cost, new_test_value.cost)),
                );
                ref_test = ref_it.next();
                new_test = new_it.next();
            }
        }
    }

    if ref_test.is_some() {
        result.insert(ref_test.unwrap().name.clone(), TestCompare::Removed);
        for ref_test_value in ref_it {
            result.insert(ref_test_value.name.clone(), TestCompare::Removed);
        }
    }

    if new_test.is_some() {
        result.insert(new_test.unwrap().name.clone(), TestCompare::Created);
        for new_test_value in new_it {
            result.insert(new_test_value.name.clone(), TestCompare::Created);
        }
    }

    result
}

/// Get the ratio indicator for the provided old and new costs.
fn get_ratio_indicator(old_cost: &u128, new_cost: &u128) -> String {
    match new_cost.partial_cmp(&old_cost).unwrap() {
        Ordering::Less => {
            let ratio = (old_cost - new_cost) * 100 / old_cost;
            format!("(-{ratio}%)")
        }
        Ordering::Greater => {
            let ratio = (new_cost - old_cost) * 100 / old_cost;
            format!("(+{ratio}%)")
        }
        Ordering::Equal => "".to_string(),
    }
}

/// Print the comparison results.
fn print_compare_result(result: &HashMap<String, TestCompare>) {
    fn print(tag: &str, name: &str, ratio: &str) {
        let s = format!("{:<10} {:<48} {}", tag, name, ratio);
        let s = match tag {
            REMOVED_TAG | CREATED_TAG => s.bright_black(),
            WORSE_TAG => s.red(),
            BETTER_TAG => s.green(),
            _ => s.into(),
        };

        println!("{}", s);
    }

    println!("{}", "Comparing tests...".blue());

    for (name, status) in result {
        match status {
            TestCompare::Removed => print(REMOVED_TAG, &name, ""),
            TestCompare::Created => print(CREATED_TAG, &name, ""),
            TestCompare::Updated((old_cost, new_cost)) => {
                match new_cost.partial_cmp(old_cost).unwrap() {
                    Ordering::Less => {
                        print(BETTER_TAG, &name, &get_ratio_indicator(old_cost, new_cost));
                    }
                    Ordering::Greater => {
                        print(WORSE_TAG, &name, &get_ratio_indicator(old_cost, new_cost));
                    }
                    Ordering::Equal => print(EQUAL_TAG, &name, ""),
                };
            }
        }
    }
}

fn main() {
    let args = Args::parse();
    let manifest_path = get_manifest_path(args.manifest_path);

    if !manifest_path.exists() {
        println!("[ERROR] Manifest not found: {}", manifest_path);
        return;
    }

    let output = execute_scarb(manifest_path.as_str(), args.show_scarb_output);
    let new_test_results = parse_output(&output);

    let ref_file = get_ref_file(&manifest_path);

    if ref_file.exists() {
        let ref_test_results = read_ref_tests(&ref_file);
        let mut results = compare_tests(&ref_test_results, &new_test_results);

        if args.update_ref {
            write_ref_file(&ref_file, new_test_results);
        }

        if args.change_only {
            results.retain(|_, status| match status {
                TestCompare::Updated((old_cost, new_cost)) => old_cost != new_cost,
                TestCompare::Created | TestCompare::Removed => true,
            });
        }

        if args.worse_only {
            results.retain(|_, status| match status {
                TestCompare::Updated((old_cost, new_cost)) => new_cost > old_cost,
                _ => false,
            });
        }

        print_compare_result(&results);
    } else {
        write_ref_file(&ref_file, new_test_results);
    }
}

#[test]
fn test_compare_tests() {
    let old_tests = vec![
        TestCost { name: "A".to_string(), cost: 1 },
        TestCost { name: "B".to_string(), cost: 1 },
        TestCost { name: "D".to_string(), cost: 2 },
    ];
    let new_tests = vec![
        TestCost { name: "B".to_string(), cost: 2 },
        TestCost { name: "C".to_string(), cost: 1 },
        TestCost { name: "D".to_string(), cost: 1 },
        TestCost { name: "E".to_string(), cost: 1 },
    ];
    let expected = HashMap::<String, TestCompare>::from([
        ("A".to_string(), TestCompare::Removed),
        ("B".to_string(), TestCompare::Updated((1, 2))),
        ("C".to_string(), TestCompare::Created),
        ("D".to_string(), TestCompare::Updated((2, 1))),
        ("E".to_string(), TestCompare::Created),
    ]);

    let res = compare_tests(&old_tests, &new_tests);
    assert_eq!(res, expected);
}

#[test]
fn test_get_ratio_indicator() {
    assert_eq!(get_ratio_indicator(&123, &123), "".to_string());
    assert_eq!(get_ratio_indicator(&60, &30), "(-50%)".to_string());
    assert_eq!(get_ratio_indicator(&30, &60), "(+100%)".to_string());
}
