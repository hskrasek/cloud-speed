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

pub fn median_f64(test_durations: &mut [f64]) -> Option<f64> {
    let len = test_durations.len();

    if len == 0 {
        return None;
    }

    let mid = len / 2;

    if len % 2 == 1 {
        let (_, median, _) =
            test_durations.select_nth_unstable_by(mid, |a, b| a.total_cmp(b));

        return Some(*median);
    }

    let (_, upper, _) =
        test_durations.select_nth_unstable_by(mid, |a, b| a.total_cmp(b));
    let upper_val = *upper;
    let lower_val = test_durations[..mid]
        .iter()
        .copied()
        .max_by(|a, b| a.total_cmp(b))
        .unwrap();

    Some((lower_val + upper_val) / 2.0)
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
