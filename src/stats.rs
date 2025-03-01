use std::time::Duration;

pub fn median(test_durations: &Vec<Duration>) -> f64 {
    let mut test_durations_u128 = test_durations
        .iter()
        .map(|duration| duration.as_millis())
        .collect::<Vec<u128>>();

    test_durations_u128.sort();

    let mid = test_durations_u128.len() / 2;

    if test_durations_u128.len() % 2 == 0 {
        mean(&vec![test_durations_u128[mid - 1], test_durations_u128[mid]])
    } else {
        test_durations_u128[mid] as f64
    }
}

pub fn mean(test_durations: &Vec<u128>) -> f64 {
    let sum = test_durations.iter().sum::<u128>();

    sum as f64 / test_durations.len() as f64
}

pub fn quartile(test_durations: &Vec<Duration>, percentile: f64) -> f64 {
    let mut test_durations_u128 = test_durations
        .iter()
        .map(|duration| duration.as_millis())
        .collect::<Vec<u128>>();

    test_durations_u128.sort_by(|a, b| a.cmp(b));

    let pos = (test_durations_u128.len() - 1) as f64 * percentile;
    let base = pos.floor();
    let rest = pos - base;

    if base as usize + 1 < test_durations_u128.len() {
        return test_durations_u128[base as usize] as f64
            + rest
                * (test_durations_u128[base as usize + 1] as f64
                    - test_durations_u128[base as usize] as f64);
    }

    test_durations_u128[base as usize] as f64
}
