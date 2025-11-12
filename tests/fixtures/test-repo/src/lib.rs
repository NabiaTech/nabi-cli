/// Test Rust module for regression testing

pub struct TestStruct {
    pub name: String,
    pub value: i32,
}

impl TestStruct {
    pub fn new(name: String) -> Self {
        TestStruct {
            name,
            value: 0,
        }
    }

    pub fn test_method(&self, x: i32) -> i32 {
        x * 2
    }
}

pub fn test_function(x: i32, y: i32) -> i32 {
    x + y
}

pub fn another_function() {
    // Another function
}
