fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (code, out) = hello::run(&args);
    print!("{out}");
    std::process::exit(code);
}
