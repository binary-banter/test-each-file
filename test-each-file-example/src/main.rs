use test_each_file::test_each_file;

fn main() {
    println!("Hello, world!");
}


fn test(content: &str) {

}

test_each_file!{ for ["a", "b"] in "./test-each-file-example/resources/" as good => test}

// test_each_file!{ "./test-each-file-example/resources/", good, |x| test(true, x) }