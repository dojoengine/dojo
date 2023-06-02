pub mod value_accessor;

pub fn remove_quotes(s: &str) -> String {
    s.replace(&['\"', '\''][..], "")
}

pub fn format_name(input: &str) -> (String, String) {
    let name = input.to_lowercase();
    let type_name = input
        .chars()
        .enumerate()
        .map(|(i, c)| {
            if i == 0 {
                c.to_uppercase().collect::<String>()
            } else {
                c.to_lowercase().collect::<String>()
            }
        })
        .collect::<String>();
    (name, type_name)
}
