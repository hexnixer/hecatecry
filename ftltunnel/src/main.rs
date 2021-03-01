use std::net::SocketAddr;
use std::path::Path;
use std::str::FromStr;

use libftltunnel::Transaction;

fn main() {
    let listen = SocketAddr::from_str("127.0.0.1:5000").unwrap();
    let peer = SocketAddr::from_str("127.0.0.1:3400").unwrap();
    let mut tran = Transaction::new(peer, listen, 11500, 32, 20, b"").unwrap();
    tran.offer_send(&Path::new("listener.py").to_path_buf()).unwrap();
    println!("offered send!");
}
