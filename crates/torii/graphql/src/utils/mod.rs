pub mod extract_value;
pub mod parse_argument;
pub mod value_accessor;

pub fn csv_to_vec(csv: &str) -> Vec<String> {
    csv.split(',').map(|s| s.trim().to_string()).collect()
}
