use nix::ifaddrs::getifaddrs;
use nix::sys::socket::InetAddr;
use nix::sys::socket::SockAddr;
use std::net::SocketAddr;

use std::net::TcpListener;

fn main() {
    dump_tcp_listener();
}


fn dump_tcp_listener() -> std::io::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:0")?;

    println!("Local addr: {}", listener.local_addr()?);
    Ok(())
}
