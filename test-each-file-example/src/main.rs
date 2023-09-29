use test_each_file::test_each_file;

fn main() {
    println!("Hello, world!");
}


fn test(good: bool, content: &str) {

}

test_each_file!{ "./test-each-file-example/resources/", good, |x| test(true, x) }