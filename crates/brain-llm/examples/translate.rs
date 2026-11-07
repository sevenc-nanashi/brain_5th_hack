use brain_llm::elan::Elan;
use std::io::Write;
fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert!(
        args.len() == 4,
        "translate MODEL.bin TOKENIZER.tok 'English text'"
    );
    let mut m = Elan::load(
        std::fs::read(&args[1]).unwrap(),
        std::fs::read(&args[2]).unwrap(),
    )
    .unwrap();
    eprintln!(
        "WORK {} bytes; TOKENS {:?}",
        m.allocated_bytes(),
        m.tokenizer.encode(&args[3]).unwrap()
    );
    let start = std::time::Instant::now();
    m.start(&args[3]).unwrap();
    let mut first = true;
    for _ in 0..brain_llm::elan::LIMIT {
        let token = loop {
            if let Some(t) = m.tick().unwrap() {
                break t;
            }
        };
        eprintln!("N {token}");
        if token == 0 {
            break;
        }
        print!("{}", m.tokenizer.decode(token, first));
        std::io::stdout().flush().unwrap();
        first = false;
    }
    println!();
    eprintln!("Elapsed {:?}", start.elapsed());
}
