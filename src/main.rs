// Use all the autocxx types which might be handy.
use autocxx::prelude::*;

include_cpp! {
    #include "sample.hpp"
    safety!(unsafe_ffi)
    generate!("print_value") // allowlist a function
    generate!("DoMath")
    generate!("Goat")
}

fn main() {
    ffi::print_value(123);
    println!("Hello, world! - C++ math should say 12={}", ffi::DoMath(4));
    let mut goat = ffi::Goat::new().within_box();
    goat.as_mut().add_a_horn();
    goat.as_mut().add_a_horn();
    assert_eq!(
        goat.describe().as_ref().unwrap().to_string_lossy(),
        "This goat has 2 horns."
    );
    // assert_eq!(ffi::do_math(12, 13), 25);
    // print!("do_math: {}\n", ffi::do_math(20, 30));
    // let mut goat = ffi::Goat::new().within_unique_ptr(); // returns a cxx::UniquePtr, i.e. a std::unique_ptr
    // goat.pin_mut().add_a_horn();
    // goat.pin_mut().add_a_horn();
    // assert_eq!(goat.describe().as_ref().unwrap().to_string_lossy(), "This goat has 2 horns.");
}