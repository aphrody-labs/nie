fn main() -> std::io::Result<()> {
    let addr = std::env::var("@ENV@_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_owned());
    let listener = std::net::TcpListener::bind(&addr)?;
    eprintln!("listening on {addr}");
    @ident@::serve(&listener)
}
