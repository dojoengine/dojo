use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, Write};
use std::process::Command;

use camino::{Utf8Path, Utf8PathBuf};
use clap::Parser;
use colored::Colorize;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

// just keep test functions starting with this prefix
const BENCH_TEST_FILTER: &str = "bench_";

// default threshold for the ratio between the old and new costs
const DEFAULT_THRESHOLD: u128 = 3;

// extract gas cost data using this prefix
const GAS_TAG: &str = "l2_gas";

const BETTER_TAG: &str = "[BETTER]";
const WORSE_TAG: &str = "[WORSE]";
const EQUAL_TAG: &str = "[EQUAL]";
const REMOVED_TAG: &str = "[REMOVED]";
const CREATED_TAG: &str = "[CREATED]";

// where to store reference files
const REF_DIR_PATH: &str = "bin/cairo-bench/references";

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct TestCost {
    name: String,
    cost: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TestCompare {
    Created(u128),
    Updated((u128, u128)),
    Removed,
}

#[derive(PartialEq, Eq)]
enum BenchResultFiltering {
    None,
    ChangeOnly,
    WorseOnly,
    BetterOnly,
}

#[derive(PartialEq, Eq)]
enum BenchResultSorting {
    None,
    WorstFirst,
    BestFirst,
}

#[derive(Parser, Debug)]
#[command(
    version,
    about = "Tool to benchmark Cairo tests.",
    long_about = "Tool for comparing Cairo test performance by measuring their gas cost. It \
                  allows tracking performance evolution over time by comparing test results with \
                  a reference file.\n- Test functions must start with `bench_` to be taken into \
                  account.\n- Gas cost must follow the output format defined in \
                  GasCounterTrait::end_csv()."
)]
struct Args {
    #[arg(long, help = "Path to a Scarb.toml manifest file")]
    pub manifest_path: Option<Utf8PathBuf>,

    #[arg(short = 's', long, help = "Show the scarb test output.")]
    pub show_scarb_output: bool,

    #[arg(long, help = "Threshold for the ratio between the old and new costs (in %).")]
    pub threshold: Option<u128>,

    #[arg(long, help = "Override the reference file corresponding to the provided manifest file.")]
    pub update_ref: bool,

    #[arg(long, help = "Just show what has changed.")]
    pub change_only: bool,

    #[arg(long, help = "Just show tests that got worse.")]
    pub worse_only: bool,

    #[arg(long, help = "Just show tests that got better.")]
    pub better_only: bool,

    #[arg(long, help = "Show the worst test first.")]
    pub worst_first: bool,

    #[arg(long, help = "Show the best test first..")]
    pub best_first: bool,
}

impl BenchResultFiltering {
    fn from_args(args: &Args) -> Self {
        if args.change_only {
            BenchResultFiltering::ChangeOnly
        } else if args.worse_only {
            BenchResultFiltering::WorseOnly
        } else if args.better_only {
            BenchResultFiltering::BetterOnly
        } else {
            BenchResultFiltering::None
        }
    }
}

impl BenchResultSorting {
    fn from_args(args: &Args) -> Self {
        if args.worst_first {
            BenchResultSorting::WorstFirst
        } else if args.best_first {
            BenchResultSorting::BestFirst
        } else {
            BenchResultSorting::None
        }
    }
}

/// Check that there is no conflict between the provided arguments.
fn validate_args(args: &Args) {
    if [args.change_only, args.worse_only, args.better_only].into_iter().filter(|b| *b).count() > 1
    {
        print!("[ERROR] change_only, worse_only, and better_only are mutually exclusive.");
        std::process::exit(1);
    }

    if args.worst_first && args.best_first {
        print!("[ERROR] worst_first and best_first are mutually exclusive.");
        std::process::exit(1);
    }
}

/// Sort test costs by name.
fn sort_tests(tests: &mut [TestCost]) {
    tests.sort_by(|a, b| a.name.partial_cmp(&b.name).unwrap());
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

        if !output.status.success() {
            panic!("{}", String::from_utf8(output.stderr).unwrap());
        }

        String::from_utf8(output.stdout).unwrap()
    };

    output
}

/// Parse the scarb test output by extracting the gas cost data, and sort the tests by name.
fn parse_output(output: &str) -> Vec<TestCost> {
    let mut tests: Vec<TestCost> = output
        .lines()
        .filter(|line| line.contains(GAS_TAG))
        .map(|line| {
            let items = line.split(" ").collect::<Vec<_>>();
            let name = items[1]
                .split("::")
                .last()
                .unwrap()
                .strip_prefix(BENCH_TEST_FILTER)
                .unwrap()
                .to_string();
            let cost = items.last().unwrap().strip_prefix("~").unwrap().strip_suffix(")").unwrap();
            let cost = cost
                .parse::<u128>()
                .unwrap_or_else(|_| panic!("Failed to convert {} to u128", cost));
            TestCost { name, cost }
        })
        .collect();

    sort_tests(&mut tests);
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

/// Read the reference file and parse the test costs, and sort the tests by name.
fn read_ref_tests(ref_file: &Utf8PathBuf) -> Vec<TestCost> {
    println!("{}", format!("Reading reference file: {}", ref_file.file_name().unwrap()).blue());

    let content = fs::read_to_string(ref_file)
        .unwrap_or_else(|_| panic!("Failed to read reference file: {}", ref_file));

    let mut tests: Vec<TestCost> = serde_json::from_str(&content)
        .unwrap_or_else(|_| panic!("Failed to parse reference file: {}", ref_file));

    sort_tests(tests.as_mut_slice());
    tests
}

/// Write the test costs to the reference file.
fn write_ref_file(ref_file: &Utf8PathBuf, tests: &[TestCost]) {
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
fn compare_tests(ref_tests: &[TestCost], new_tests: &[TestCost]) -> HashMap<String, TestCompare> {
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
                result
                    .insert(new_test_value.name.clone(), TestCompare::Created(new_test_value.cost));
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
        result.insert(new_test.unwrap().name.clone(), TestCompare::Created(new_test.unwrap().cost));
        for new_test_value in new_it {
            result.insert(new_test_value.name.clone(), TestCompare::Created(new_test_value.cost));
        }
    }

    result
}

/// Get the ratio and difference between the old and new costs.
fn get_ratio(old_cost: u128, new_cost: u128) -> (f64, i128) {
    let diff = new_cost as i128 - old_cost as i128;
    (diff as f64 * 100_f64 / old_cost as f64, diff)
}

/// Get the ratio indicator for the provided old and new costs.
fn get_ratio_indicator(old_cost: &u128, new_cost: &u128) -> String {
    match new_cost.partial_cmp(old_cost).unwrap() {
        Ordering::Less | Ordering::Greater => {
            let (ratio, diff) = get_ratio(*old_cost, *new_cost);
            format!("({ratio:+.2}%) / {diff:+}")
        }
        Ordering::Equal => "".to_string(),
    }
}

/// Filter and sort the results according to the provided criteria.
fn filter_and_sort_result(
    result: &HashMap<String, TestCompare>,
    filtering_criteria: BenchResultFiltering,
    sorting_criteria: BenchResultSorting,
    threshold: f64,
) -> Vec<(&String, &TestCompare)> {
    result
        .iter()
        .filter(|(_, status)| match status {
            TestCompare::Updated((old_cost, new_cost)) => match filtering_criteria {
                BenchResultFiltering::ChangeOnly => {
                    compare_costs(new_cost, old_cost, threshold) != Ordering::Equal
                }
                BenchResultFiltering::WorseOnly => {
                    compare_costs(new_cost, old_cost, threshold) == Ordering::Greater
                }
                BenchResultFiltering::BetterOnly => {
                    compare_costs(new_cost, old_cost, threshold) == Ordering::Less
                }
                BenchResultFiltering::None => true,
            },
            TestCompare::Created(_) | TestCompare::Removed => {
                filtering_criteria == BenchResultFiltering::None
            }
        })
        .sorted_by(|(a_name, a_status), (b_name, b_status)| match sorting_criteria {
            BenchResultSorting::WorstFirst | BenchResultSorting::BestFirst => {
                match (a_status, b_status) {
                    (
                        TestCompare::Updated((a_old_cost, a_new_cost)),
                        TestCompare::Updated((b_old_cost, b_new_cost)),
                    ) => {
                        let (mut a_ratio, _) = get_ratio(*a_old_cost, *a_new_cost);
                        let (mut b_ratio, _) = get_ratio(*b_old_cost, *b_new_cost);

                        if a_ratio.abs() < threshold {
                            a_ratio = 0.0;
                        }

                        if b_ratio.abs() < threshold {
                            b_ratio = 0.0;
                        }

                        let res = if let BenchResultSorting::WorstFirst = sorting_criteria {
                            b_ratio.partial_cmp(&a_ratio).unwrap()
                        } else {
                            a_ratio.partial_cmp(&b_ratio).unwrap()
                        };

                        if res == Ordering::Equal {
                            a_name.partial_cmp(b_name).unwrap()
                        } else {
                            res
                        }
                    }
                    (TestCompare::Updated(_), _) => Ordering::Less,
                    (_, TestCompare::Updated(_)) => Ordering::Greater,
                    (_, _) => a_name.partial_cmp(b_name).unwrap(),
                }
            }
            BenchResultSorting::None => a_name.partial_cmp(b_name).unwrap(),
        })
        .collect::<Vec<_>>()
}

/// Print the result of a single test.
fn print_single_test_result(tag: &str, name: &str, cost: u128, ratio: &str) {
    let s = format!("{:<10} {:<48} {:<16} {}", tag, name, cost, ratio);
    let s = match tag {
        REMOVED_TAG | CREATED_TAG => s.bright_black(),
        WORSE_TAG => s.red(),
        BETTER_TAG => s.green(),
        _ => s.into(),
    };

    println!("{}", s);
}

fn compare_costs(new_cost: &u128, old_cost: &u128, threshold: f64) -> Ordering {
    let (ratio, _) = get_ratio(*old_cost, *new_cost);
    if ratio > threshold {
        Ordering::Greater
    } else if ratio < -1.0 * threshold {
        Ordering::Less
    } else {
        Ordering::Equal
    }
}

/// Print the comparison results.
fn print_compare_result(
    result: &HashMap<String, TestCompare>,
    filtering_criteria: BenchResultFiltering,
    sorting_criteria: BenchResultSorting,
    threshold: f64,
) {
    let mut has_results = false;

    println!("{}", "Comparing tests...".blue());

    for (name, status) in
        filter_and_sort_result(result, filtering_criteria, sorting_criteria, threshold)
    {
        has_results = true;
        match status {
            TestCompare::Removed => print_single_test_result(REMOVED_TAG, name, 0, ""),
            TestCompare::Created(cost) => print_single_test_result(CREATED_TAG, name, *cost, ""),
            TestCompare::Updated((old_cost, new_cost)) => {
                match compare_costs(new_cost, old_cost, threshold) {
                    Ordering::Less => {
                        print_single_test_result(
                            BETTER_TAG,
                            name,
                            *new_cost,
                            &get_ratio_indicator(old_cost, new_cost),
                        );
                    }
                    Ordering::Greater => {
                        print_single_test_result(
                            WORSE_TAG,
                            name,
                            *new_cost,
                            &get_ratio_indicator(old_cost, new_cost),
                        );
                    }
                    Ordering::Equal => print_single_test_result(EQUAL_TAG, name, *new_cost, ""),
                };
            }
        }
    }

    if !has_results {
        println!("No results found.");
    }
}

fn main() {
    let args = Args::parse();

    validate_args(&args);

    let bench_result_filtering = BenchResultFiltering::from_args(&args);
    let bench_result_sorting = BenchResultSorting::from_args(&args);
    let threshold = args.threshold.unwrap_or(DEFAULT_THRESHOLD) as f64;

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
        let results = compare_tests(&ref_test_results, &new_test_results);

        if args.update_ref {
            write_ref_file(&ref_file, &new_test_results);
        }

        print_compare_result(&results, bench_result_filtering, bench_result_sorting, threshold);
    } else {
        write_ref_file(&ref_file, &new_test_results);
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
        ("C".to_string(), TestCompare::Created(1)),
        ("D".to_string(), TestCompare::Updated((2, 1))),
        ("E".to_string(), TestCompare::Created(1)),
    ]);

    let res = compare_tests(&old_tests, &new_tests);
    assert_eq!(res, expected);
}

#[test]
fn test_get_ratio_indicator() {
    assert_eq!(get_ratio_indicator(&123, &123), "".to_string());
    assert_eq!(get_ratio_indicator(&60, &30), "(-50.00%) / -30".to_string());
    assert_eq!(get_ratio_indicator(&30, &60), "(+100.00%) / +30".to_string());
}

#[test]
fn test_filtering_results_without_filtering() {
    let result = HashMap::<String, TestCompare>::from([
        ("A".to_string(), TestCompare::Updated((1, 2))),
        ("B".to_string(), TestCompare::Created(1)),
        ("C".to_string(), TestCompare::Updated((1, 1))),
        ("D".to_string(), TestCompare::Updated((2, 1))),
        ("E".to_string(), TestCompare::Removed),
    ]);
    let expected_names = vec!["A", "B", "C", "D", "E"];

    let filtered =
        filter_and_sort_result(&result, BenchResultFiltering::None, BenchResultSorting::None, 0.0);

    assert_eq!(
        filtered.iter().map(|x| x.0).collect::<Vec<_>>(),
        expected_names,
        "Filtering results should return the same results when no filtering is applied."
    );
}

#[test]
fn test_filtering_results_change_only() {
    let result = HashMap::<String, TestCompare>::from([
        ("A".to_string(), TestCompare::Updated((1, 2))),
        ("B".to_string(), TestCompare::Created(1)),
        ("C".to_string(), TestCompare::Updated((1, 1))),
        ("D".to_string(), TestCompare::Updated((2, 1))),
        ("E".to_string(), TestCompare::Removed),
    ]);
    let expected_names = vec!["A", "D"];

    let filtered = filter_and_sort_result(
        &result,
        BenchResultFiltering::ChangeOnly,
        BenchResultSorting::None,
        0.0,
    );

    assert_eq!(
        filtered.iter().map(|x| x.0).collect::<Vec<_>>(),
        expected_names,
        "Should only return tests that have changed."
    );
}

#[test]
fn test_filtering_results_better_only() {
    let result = HashMap::<String, TestCompare>::from([
        ("A".to_string(), TestCompare::Updated((1, 2))),
        ("B".to_string(), TestCompare::Created(1)),
        ("C".to_string(), TestCompare::Updated((1, 1))),
        ("D".to_string(), TestCompare::Updated((2, 1))),
        ("E".to_string(), TestCompare::Removed),
    ]);
    let expected_names = vec!["D"];

    let filtered = filter_and_sort_result(
        &result,
        BenchResultFiltering::BetterOnly,
        BenchResultSorting::None,
        0.0,
    );

    assert_eq!(
        filtered.iter().map(|x| x.0).collect::<Vec<_>>(),
        expected_names,
        "Should only return tests that have improved."
    );
}

#[test]
fn test_filtering_results_worse_only() {
    let result = HashMap::<String, TestCompare>::from([
        ("A".to_string(), TestCompare::Updated((1, 2))),
        ("B".to_string(), TestCompare::Created(1)),
        ("C".to_string(), TestCompare::Updated((1, 1))),
        ("D".to_string(), TestCompare::Updated((2, 1))),
        ("E".to_string(), TestCompare::Removed),
    ]);
    let expected_names = vec!["A"];

    let filtered = filter_and_sort_result(
        &result,
        BenchResultFiltering::WorseOnly,
        BenchResultSorting::None,
        0.0,
    );

    assert_eq!(
        filtered.iter().map(|x| x.0).collect::<Vec<_>>(),
        expected_names,
        "Should only return tests that have worsened."
    );
}

#[test]
fn test_sorting_results_worst_first() {
    let result = HashMap::<String, TestCompare>::from([
        ("Z".to_string(), TestCompare::Updated((1, 2))),
        ("B".to_string(), TestCompare::Created(1)),
        ("C".to_string(), TestCompare::Updated((1, 1))),
        ("A".to_string(), TestCompare::Updated((1, 2))),
        ("F".to_string(), TestCompare::Updated((1, 3))),
        ("G".to_string(), TestCompare::Updated((1, 4))),
        ("D".to_string(), TestCompare::Updated((2, 1))),
        ("E".to_string(), TestCompare::Removed),
    ]);

    let expected_names = vec!["G", "F", "A", "Z", "C", "D", "B", "E"];

    let filtered = filter_and_sort_result(
        &result,
        BenchResultFiltering::None,
        BenchResultSorting::WorstFirst,
        0.0,
    );

    assert_eq!(
        filtered.iter().map(|x| x.0).collect::<Vec<_>>(),
        expected_names,
        "Should sort tests by worst first."
    );
}

#[test]
fn test_sorting_results_best_first() {
    let result = HashMap::<String, TestCompare>::from([
        ("Z".to_string(), TestCompare::Updated((1, 2))),
        ("B".to_string(), TestCompare::Created(1)),
        ("C".to_string(), TestCompare::Updated((1, 1))),
        ("F".to_string(), TestCompare::Updated((1, 3))),
        ("G".to_string(), TestCompare::Updated((1, 4))),
        ("D".to_string(), TestCompare::Updated((2, 1))),
        ("A".to_string(), TestCompare::Updated((2, 1))),
        ("E".to_string(), TestCompare::Removed),
    ]);

    let expected_names = vec!["A", "D", "C", "Z", "F", "G", "B", "E"];

    let filtered = filter_and_sort_result(
        &result,
        BenchResultFiltering::None,
        BenchResultSorting::BestFirst,
        0.0,
    );

    assert_eq!(
        filtered.iter().map(|x| x.0).collect::<Vec<_>>(),
        expected_names,
        "Should sort tests by worst first."
    );
}

#[test]
fn test_sorting_result_with_threshold() {
    let result = HashMap::<String, TestCompare>::from([
        ("Z".to_string(), TestCompare::Updated((10, 20))),
        ("B".to_string(), TestCompare::Created(10)),
        ("C".to_string(), TestCompare::Updated((10, 14))),
        ("A".to_string(), TestCompare::Updated((10, 10))),
        ("F".to_string(), TestCompare::Updated((10, 30))),
        ("G".to_string(), TestCompare::Updated((10, 40))),
        ("D".to_string(), TestCompare::Updated((13, 10))),
        ("E".to_string(), TestCompare::Removed),
    ]);

    let expected_names = vec!["G", "F", "Z", "A", "C", "D", "B", "E"];

    let filtered = filter_and_sort_result(
        &result,
        BenchResultFiltering::None,
        BenchResultSorting::WorstFirst,
        50.0,
    );

    assert_eq!(
        filtered.iter().map(|x| x.0).collect::<Vec<_>>(),
        expected_names,
        "Should sort tests by worst first."
    );
}
