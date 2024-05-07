fn any_none<T>(arr: @Array<Option<T>>) -> bool {
    let mut i = 0;
    let mut res = false;
    loop {
        if i >= arr.len() { break; }

        if arr.at(i).is_none() { 
            res = true;
            break;
        }
        i += 1;
    };
    res
}

fn sum(arr: Array<Option<u32>>) -> u32 {
    let mut i = 0;
    let mut res = 0;

    loop {
        if i >= arr.len() { break res; }
        res += (*arr.at(i)).unwrap();
        i += 1;
    }
}
