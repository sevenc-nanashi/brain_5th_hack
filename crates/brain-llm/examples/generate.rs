use brain_llm::{Model, Tokenizer};
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let mut model = Model::load(&std::fs::read(&args[1]).unwrap()).unwrap();
    let tok = Tokenizer::load(&std::fs::read(&args[2]).unwrap()).unwrap();
    let prompt = "Once upon a time";
    let tokens = tok.encode(prompt);
    for token in &tokens {
        eprintln!("P {token}");
    }
    let mut current = tokens[0];
    for pos in 0..tokens.len() + 31 {
        let next = model.forward(current, pos);
        let next = if pos + 1 < tokens.len() {
            tokens[pos + 1]
        } else {
            next
        };
        eprintln!("N {next}");
        if next == 1 {
            break;
        }
        print!("{}", tok.decode(current, next));
        current = next;
    }
    println!();
}
