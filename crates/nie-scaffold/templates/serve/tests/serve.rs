use std::io::{Read as _, Write as _};
use std::net::{TcpListener, TcpStream};

fn get(addr: std::net::SocketAddr, path: &str) -> String {
    let mut s = TcpStream::connect(addr).expect("connect");
    write!(s, "GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").expect("write");
    let mut out = String::new();
    s.read_to_string(&mut out).expect("read");
    out
}

#[test]
fn health_and_not_found() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    std::thread::spawn(move || {
        let _ = @ident@::serve(&listener);
    });
    let ok = get(addr, "/health");
    assert!(ok.starts_with("HTTP/1.1 200"), "{ok}");
    assert!(ok.contains("\"ok\":true"));
    assert!(get(addr, "/x").starts_with("HTTP/1.1 404"));
}
