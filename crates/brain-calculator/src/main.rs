#![no_std]
#![no_main]
use brain_calculator::Calculator;

#[reflex::main]
fn main() -> Calculator {
    Calculator::new()
}
