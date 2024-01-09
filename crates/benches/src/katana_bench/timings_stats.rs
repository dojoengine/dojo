use tokio::time::Instant;

use crate::katana_bench::BLOCK_TIME;

pub fn timetable_stats(times: Vec<Instant>) -> usize {
    let mut left = 0;
    let mut right = 0;
    let mut max = 0;

    loop {
        if right == times.len() {
            break;
        }

        if times[right] - times[left] > BLOCK_TIME {
            left += 1;
        } else {
            right += 1;
            let current = right - left;
            if current > max {
                max = current;
            }
        }
    }

    println!("max in time window: {}", max); // This will be replaced by parsing the output of the katana in the next PR
    max
}
