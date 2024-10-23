#[inline(always)]
fn is_letter(c: u8) -> bool {
    (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z')
}


#[inline(always)]
fn is_numeric(c: u8) -> bool {
    c >= '0' && c <= '9'
}

/// Verifies that the provided name is valid according to the following RegEx: ^[a-zA-Z0-9_]+$
pub fn is_name_valid(name: @ByteArray) -> bool {
    let mut i = 0;
    loop {
        if i >= name.len() {
            break true;
        }

        let c = name.at(i).unwrap();

        if !is_letter(c) && !is_numeric(c) && c != '_' {
            break false;
        }

        i += 1;
    }
}
