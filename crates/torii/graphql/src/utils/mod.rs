pub mod extract_value;
pub mod parse_argument;
pub mod value_accessor;

pub fn format_name(input: &str) -> (String, String) {
    let name = input.to_lowercase();
    let type_name = input.to_string();
    (name, type_name)
}

pub fn csv_to_vec(csv: &str) -> Vec<String> {
    csv.split(',').map(|s| s.trim().to_string()).collect()
}
