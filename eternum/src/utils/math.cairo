// --------------------- //
//  Useful Math          //
// --------------------- //

// returns if time has passed
fn get_past_time(current: u128, time_stamp: u128) -> bool {
    if (current < time_stamp) {
        return true;
    } else {
        return false;
    }
}

// returns 
fn u128_div_remainder(value: u128, divider: u128) -> (u128, u128) {
    return ((value / divider), (value % divider));
}

fn get_percentage_by_bp(value: u128, bp: u128) -> u128 {
    return (value * bp) / 1000_u128;
}
