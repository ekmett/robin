use robin::{Meta,Sender,Receiver};
use std::env;
use std::fs;
use std::net::SocketAddr;
use std::net::UdpSocket;
use std::path::Path;

// 12232 -- sender
// 12233 -- receiver

// cargo run send 127.0.0.255:12233 some_file
pub fn main() {
  let args: Vec<String> = env::args().collect();
  if args.len() < 2 {
    println!("No arguments");
    usage(1);
  }
  match args[1].as_str() {
    "send" => do_send(args),
    "recv" => do_recv(args),
    "help" => usage(0),
    _ => {
      println!("Unknown command"); usage(1);
    }
  }
}

fn usage(exit_code : i32) -> ! {
  let args: Vec<String> = env::args().collect();
  let exe = Path::new(&args[0]).file_name().expect("expected filename").to_string_lossy();
  println!("Usage: {} <command>", exe);
  println!("");
  println!("{} help",exe);
  println!("{} send <host:port> <filename>",exe);
  println!("{} recv [port]",exe);
  std::process::exit(exit_code);
}

fn do_send(args: Vec<String>) {
  if args.len() != 4 {
    usage(1);
  }
  let dest: SocketAddr = args[2].parse().expect("cannot parse destination socket");
  let file = &args[3];
  // reuse addr?
  let out = UdpSocket::bind("0.0.0.0:12232").expect("Unable to bind socket");
  out.set_broadcast(true).unwrap();

  println!("Sending {} to {}",file,dest);

  let contents = fs::read(file).expect("cannot read file");
  Sender::new().send(
    file.clone(),
      &contents,
      |p| { out.send_to(p,dest).expect("error sending packet"); }
  )
}

fn do_recv(args: Vec<String>) {
  if args.len() != 2 {
    usage(2);
  }

  let mut receiver = Receiver::new();
  // let address: SocketAddr = args[2].parse().expect("cannot parse destination socket");
  let socket = UdpSocket::bind("0.0.0.0:12233").expect("Could not bind to address");
  let mut buf = [0; 65536];
  loop { 
    let (bytes, _source) = socket.recv_from(&mut buf).expect("Didn't receive data");
    receiver.recv(&buf[..bytes], |meta:Meta, contents:Vec<u8>| {
      println!("{:?}\n{} bytes received",meta,contents.len());
    });
  }
}

