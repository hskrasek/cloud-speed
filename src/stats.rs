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

/// Calculates the p-th percentile of a slice of f64 values.
///
/// Uses linear interpolation between values for non-integer positions.
///
/// # Arguments
/// * `values` - A mutable slice of f64 values (will be sorted in place)
/// * `p` - The percentile to calculate, must be in range [0.0, 1.0]
///
/// # Returns
/// * `Some(percentile)` - The calculated percentile value
/// * `None` - If the slice is empty or p is outside [0.0, 1.0]
///
/// # Examples
/// ```
/// let mut values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
/// let p90 = percentile_f64(&mut values, 0.9);
/// ```
pub fn percentile_f64(values: &mut [f64], p: f64) -> Option<f64> {
    // Handle edge cases
    if values.is_empty() {
        return None;
    }

    if !(0.0..=1.0).contains(&p) {
        return None;
    }

    let len = values.len();

    // Single element case
    if len == 1 {
        return Some(values[0]);
    }

    // Sort the values
    values.sort_by(|a, b| a.total_cmp(b));

    // Handle boundary cases
    if p == 0.0 {
        return Some(values[0]);
    }
    if p == 1.0 {
        return Some(values[len - 1]);
    }

    // Calculate position using linear interpolation
    // Position in the sorted array (0-indexed)
    let pos = (len - 1) as f64 * p;
    let lower_idx = pos.floor() as usize;
    let upper_idx = pos.ceil() as usize;
    let fraction = pos - pos.floor();

    // If position is exactly on an index, return that value
    if lower_idx == upper_idx {
        return Some(values[lower_idx]);
    }

    // Linear interpolation between adjacent values
    let lower_val = values[lower_idx];
    let upper_val = values[upper_idx];
    Some(lower_val + fraction * (upper_val - lower_val))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // Tests for median_f64
    #[test]
    fn test_median_f64_empty_slice() {
        let mut values: Vec<f64> = vec![];
        assert_eq!(median_f64(&mut values), None);
    }

    #[test]
    fn test_median_f64_single_element() {
        let mut values = vec![42.0];
        assert_eq!(median_f64(&mut values), Some(42.0));
    }

    #[test]
    fn test_median_f64_odd_length() {
        // Odd length - median is the middle element
        let mut values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(median_f64(&mut values), Some(3.0));
    }

    #[test]
    fn test_median_f64_even_length() {
        // Even length - median is average of two middle elements
        let mut values = vec![1.0, 2.0, 3.0, 4.0];
        assert_eq!(median_f64(&mut values), Some(2.5));
    }

    #[test]
    fn test_median_f64_unsorted_input() {
        // Should work with unsorted input
        let mut values = vec![5.0, 1.0, 3.0, 2.0, 4.0];
        assert_eq!(median_f64(&mut values), Some(3.0));
    }

    #[test]
    fn test_median_f64_two_elements() {
        let mut values = vec![10.0, 20.0];
        assert_eq!(median_f64(&mut values), Some(15.0));
    }

    #[test]
    fn test_median_f64_result_in_range() {
        // Median should always be between min and max
        let mut values = vec![10.0, 50.0, 30.0, 20.0, 40.0];
        let result = median_f64(&mut values).unwrap();
        assert!(result >= 10.0 && result <= 50.0);
    }

    // Tests for percentile_f64
    #[test]
    fn test_percentile_f64_empty_slice() {
        let mut values: Vec<f64> = vec![];
        assert_eq!(percentile_f64(&mut values, 0.5), None);
    }

    #[test]
    fn test_percentile_f64_single_element() {
        let mut values = vec![42.0];
        assert_eq!(percentile_f64(&mut values, 0.0), Some(42.0));
        assert_eq!(percentile_f64(&mut values, 0.5), Some(42.0));
        assert_eq!(percentile_f64(&mut values, 1.0), Some(42.0));
    }

    #[test]
    fn test_percentile_f64_boundary_p_values() {
        let mut values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(percentile_f64(&mut values, 0.0), Some(1.0));

        let mut values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(percentile_f64(&mut values, 1.0), Some(5.0));
    }

    #[test]
    fn test_percentile_f64_invalid_p() {
        let mut values = vec![1.0, 2.0, 3.0];
        assert_eq!(percentile_f64(&mut values, -0.1), None);
        assert_eq!(percentile_f64(&mut values, 1.1), None);
    }

    #[test]
    fn test_percentile_f64_median() {
        // Odd length - median is middle element
        let mut values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(percentile_f64(&mut values, 0.5), Some(3.0));

        // Even length - median is interpolated
        let mut values = vec![1.0, 2.0, 3.0, 4.0];
        assert_eq!(percentile_f64(&mut values, 0.5), Some(2.5));
    }

    #[test]
    fn test_percentile_f64_90th() {
        let mut values =
            vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];
        // Position = 9 * 0.9 = 8.1, so interpolate between index 8 (9.0) and 9 (10.0)
        // Result = 9.0 + 0.1 * (10.0 - 9.0) = 9.1
        let result = percentile_f64(&mut values, 0.9).unwrap();
        assert!((result - 9.1).abs() < 0.0001);
    }

    #[test]
    fn test_percentile_f64_unsorted_input() {
        let mut values = vec![5.0, 1.0, 3.0, 2.0, 4.0];
        assert_eq!(percentile_f64(&mut values, 0.5), Some(3.0));
    }

    #[test]
    fn test_percentile_f64_result_in_range() {
        let values = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        for p in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let result = percentile_f64(&mut values.clone(), p).unwrap();
            assert!(result >= 10.0 && result <= 50.0);
        }
    }

    // Property-based tests for median_f64
    // Feature: cloudflare-speedtest-parity, Property 1: Median Calculation Correctness
    // Validates: Requirements 2.4
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property: For any non-empty slice of f64 values, the median is always
        /// between the minimum and maximum values (inclusive)
        #[test]
        fn median_result_within_bounds(
            values in prop::collection::vec(
                prop::num::f64::NORMAL | prop::num::f64::POSITIVE | prop::num::f64::NEGATIVE,
                1..100
            ).prop_filter("no NaN or infinite values", |v| v.iter().all(|x| x.is_finite()))
        ) {
            let mut values_clone = values.clone();
            let min_val = values.iter().cloned().min_by(|a, b| a.total_cmp(b)).unwrap();
            let max_val = values.iter().cloned().max_by(|a, b| a.total_cmp(b)).unwrap();

            let result = median_f64(&mut values_clone);

            prop_assert!(result.is_some());
            let median_val = result.unwrap();
            prop_assert!(
                median_val >= min_val && median_val <= max_val,
                "Median {} should be in range [{}, {}]",
                median_val, min_val, max_val
            );
        }

        /// Property: For odd-length slices, the median equals the middle element after sorting
        #[test]
        fn median_odd_length_is_middle_element(
            values in prop::collection::vec(
                prop::num::f64::NORMAL | prop::num::f64::POSITIVE | prop::num::f64::NEGATIVE,
                1..50
            )
            .prop_filter("no NaN or infinite values", |v| v.iter().all(|x| x.is_finite()))
            .prop_filter("odd length", |v| v.len() % 2 == 1)
        ) {
            let mut values_clone = values.clone();
            let mut sorted = values.clone();
            sorted.sort_by(|a, b| a.total_cmp(b));
            let expected_median = sorted[sorted.len() / 2];

            let result = median_f64(&mut values_clone);

            prop_assert!(result.is_some());
            prop_assert!(
                (result.unwrap() - expected_median).abs() < f64::EPSILON,
                "Median {} should equal middle element {} for odd-length slice",
                result.unwrap(), expected_median
            );
        }

        /// Property: For even-length slices, the median equals the average of the two middle elements
        #[test]
        fn median_even_length_is_average_of_middle_two(
            values in prop::collection::vec(
                prop::num::f64::NORMAL | prop::num::f64::POSITIVE | prop::num::f64::NEGATIVE,
                2..50
            )
            .prop_filter("no NaN or infinite values", |v| v.iter().all(|x| x.is_finite()))
            .prop_filter("even length", |v| v.len() % 2 == 0)
        ) {
            let mut values_clone = values.clone();
            let mut sorted = values.clone();
            sorted.sort_by(|a, b| a.total_cmp(b));
            let mid = sorted.len() / 2;
            let expected_median = (sorted[mid - 1] + sorted[mid]) / 2.0;

            let result = median_f64(&mut values_clone);

            prop_assert!(result.is_some());
            prop_assert!(
                (result.unwrap() - expected_median).abs() < 1e-10,
                "Median {} should equal average of middle elements {} for even-length slice",
                result.unwrap(), expected_median
            );
        }
    }

    // Property-based tests for percentile_f64
    // Feature: cloudflare-speedtest-parity, Property 4: Percentile Aggregation Correctness
    // Validates: Requirements 4.3, 5.4
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        /// Property: For any non-empty slice and valid percentile p, the result
        /// is always between the minimum and maximum values (inclusive)
        #[test]
        fn percentile_result_within_bounds(
            values in prop::collection::vec(
                prop::num::f64::NORMAL | prop::num::f64::POSITIVE | prop::num::f64::NEGATIVE,
                1..100
            ).prop_filter("no NaN or infinite values", |v| v.iter().all(|x| x.is_finite())),
            p in 0.0f64..=1.0f64
        ) {
            let mut values_clone = values.clone();
            let min_val = values.iter().cloned().min_by(|a, b| a.total_cmp(b)).unwrap();
            let max_val = values.iter().cloned().max_by(|a, b| a.total_cmp(b)).unwrap();

            let result = percentile_f64(&mut values_clone, p);

            prop_assert!(result.is_some());
            let percentile_val = result.unwrap();
            prop_assert!(
                percentile_val >= min_val && percentile_val <= max_val,
                "Percentile {} = {} should be in range [{}, {}]",
                p, percentile_val, min_val, max_val
            );
        }

        /// Property: Percentile ordering - for p1 < p2, percentile(p1) <= percentile(p2)
        #[test]
        fn percentile_ordering(
            values in prop::collection::vec(
                prop::num::f64::NORMAL | prop::num::f64::POSITIVE | prop::num::f64::NEGATIVE,
                2..100
            ).prop_filter("no NaN or infinite values", |v| v.iter().all(|x| x.is_finite())),
            p1 in 0.0f64..=1.0f64,
            p2 in 0.0f64..=1.0f64
        ) {
            let (lower_p, higher_p) = if p1 <= p2 { (p1, p2) } else { (p2, p1) };

            let mut values_clone1 = values.clone();
            let mut values_clone2 = values.clone();

            let result1 = percentile_f64(&mut values_clone1, lower_p);
            let result2 = percentile_f64(&mut values_clone2, higher_p);

            prop_assert!(result1.is_some());
            prop_assert!(result2.is_some());
            prop_assert!(
                result1.unwrap() <= result2.unwrap(),
                "percentile({}) = {} should be <= percentile({}) = {}",
                lower_p, result1.unwrap(), higher_p, result2.unwrap()
            );
        }

        /// Property: For p=0.9 (90th percentile), approximately 90% of values should be <= result
        /// Note: With linear interpolation and small sample sizes, the exact percentage can vary.
        /// We use realistic network measurement values (positive, bounded) for this test.
        #[test]
        fn percentile_90th_covers_approximately_90_percent(
            values in prop::collection::vec(
                // Use realistic network measurement values (0.1ms to 10000ms)
                0.1f64..10000.0f64,
                20..100  // Minimum 20 samples for meaningful percentile
            )
        ) {
            let mut values_clone = values.clone();
            let result = percentile_f64(&mut values_clone, 0.9);

            prop_assert!(result.is_some());
            let p90 = result.unwrap();

            // Count how many values are <= p90
            let count_below = values.iter().filter(|&&v| v <= p90).count();
            let percentage = count_below as f64 / values.len() as f64;

            // With linear interpolation and sufficient samples, at least ~85% of values
            // should be <= the 90th percentile
            prop_assert!(
                percentage >= 0.85,
                "90th percentile {} should have at least 85% of values below it, but only {:.1}% are",
                p90, percentage * 100.0
            );
        }
    }
}
