fn main() {}

#[cfg(test)]
mod tests {
    use test_each_file::test_each_file;

    fn simple_test(input: &str) {
        assert!(input.split_whitespace().all(|n| n.parse::<usize>().is_ok()));
    }

    fn complex_test([input, output]: [&str; 2]) {
        assert_eq!(
            input
                .split_whitespace()
                .map(|n| n.parse::<usize>().unwrap())
                .sum::<usize>(),
            output.parse().unwrap()
        )
    }

    test_each_file! { in "./examples/readme/resources_simple/" as simple => simple_test}
    test_each_file! { for ["in", "out"] in "./examples/readme/resources_complex/" as complex => complex_test}
    test_each_file! { in "./examples/readme/resources_simple/" as closure => |c: &str| assert!(c.contains(" ")) }
    test_each_file! { for ["in", "out"] in "./examples/readme/resources_complex/" as example => |[a, b]: [&str; 2]| assert_ne!(a, b) }
}

