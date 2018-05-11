#[macro_use]
extern crate serde_derive;

extern crate serde;
extern crate serde_json;

#[macro_use]
extern crate structopt;

mod analyze;
mod flow;

use analyze::sequence::{ReSequencer, Sequencer};
use structopt::StructOpt;
use std::net::IpAddr;
use std::net::UdpSocket;
use std::time::Duration;
use flow::Flow;

/// qosmap options
#[derive(StructOpt, Debug)]
struct Opt {
    /// server mode
    #[structopt(short = "s", long = "server")]
    server: bool,
    /// server address
    #[structopt(short = "i", long = "ip", default_value = "0.0.0.0")]
    ip: IpAddr,
    /// server port
    #[structopt(short = "p", long = "port", default_value = "4801")]
    port: u16,
    /// packet rate in packets per second
    #[structopt(short = "r", long = "rate", default_value = "1000")]
    rate: u32,
    /// duration of the test in seconds
    #[structopt(short = "d", long = "duration", default_value = "1")]
    duration: u64,
}

fn store_seq(mut buf: Box<[u8]>, seq: u32) -> Box<[u8]> {
    buf[0] = ((seq >> 24) & 0xff) as u8;
    buf[1] = ((seq >> 16) & 0xff) as u8;
    buf[2] = ((seq >> 8) & 0xff) as u8;
    buf[3] = ((seq >> 0) & 0xff) as u8;
    buf
}

fn main() {
    let opt = Opt::from_args();
    println!("{:?}", opt);

    if opt.server {
        let sk = UdpSocket::bind((opt.ip, opt.port)).expect("bind server");

        let mut reseq = ReSequencer::new(|buf: &[u8]| {
            (buf[3] as u32) | (buf[2] as u32) << 8 | (buf[1] as u32) << 16
                | (buf[0] as u32) << 24
        });

        let mut buffer = [0; 2000];

        println!("Wait for incoming flow...");
        sk.peek(&mut buffer);
        sk.set_read_timeout(Some(Duration::from_millis(10)));

        println!("Receive flow...");
        loop {
            let bytes;
            match sk.recv(&mut buffer) {
                Err(_) => {
                    break;
                }
                Ok(b) => {
                    bytes = b;
                }
            }
            reseq.track(&buffer[..bytes]);
        }
        println!("{:?}", reseq.missing);
        println!(
            "{:?}",
            reseq
                .missing
                .iter()
                .map(|&(a, b)| b - a)
                .fold(0, |acc, len| acc + len)
        );
        println!("{:?}", reseq.dups);
    } else {
        // client

        let sender = UdpSocket::bind("0.0.0.0:0").expect("bind sender");
        sender
            .connect((opt.ip, opt.port))
            .expect("connect to server");

        let mut seq = Sequencer::new(store_seq);
        let pps = opt.rate;
        let secs = opt.duration;

        let mut flow = Flow::from_socket(
            pps,
            10,
            Duration::from_secs(secs),
            move |mut payload: Box<[u8]>| {
                payload = seq.mark(payload);
                Ok(payload)
            },
            sender,
        );
        flow.start_xmit();
    }
}

#[cfg(test)]
mod tests {
    use analyze::sequence::{ReSequencer, Sequencer};
    use std::num::Wrapping;
    use flow::Flow;
    use std::net::UdpSocket;
    use std::time::Duration;
    use std::thread;

    fn fresh_pair_of_socks() -> (UdpSocket, UdpSocket) {
        let port: u16;

        let sender = UdpSocket::bind("127.0.0.1:0").expect("bind sender");
        let receiver = UdpSocket::bind("127.0.0.1:0").expect("bind receiver");

        port = receiver
            .local_addr()
            .expect("get port from receiving socket")
            .port();
        sender
            .connect(("127.0.0.1", port))
            .expect("connect to receiver");

        (sender, receiver)
    }

    #[test]
    fn combine_sequence_with_flow() {
        let (sk_snd, sk_rcv) = fresh_pair_of_socks();

        let mut seq = Sequencer::new(store_seq);
        let mut reseq = ReSequencer::new(|buf: &[u8]| {
            (buf[3] as u32) | (buf[2] as u32) << 8 | (buf[1] as u32) << 16
                | (buf[0] as u32) << 24
        });
        let pps = 1000;
        let secs = 1;

        let mut flow = Flow::from_socket(
            pps,
            10,
            Duration::from_secs(secs),
            move |mut payload: Box<[u8]>| {
                payload = seq.mark(payload);
                Ok(payload)
            },
            sk_snd,
        );

        let sender = thread::spawn(move || {
            flow.start_xmit();
        });

        let receiver = thread::spawn(move || {
            let mut buffer = [0; 2000];
            let sk = sk_rcv;
            sk.set_read_timeout(Some(Duration::from_millis(10))).expect("set timeout");

            loop {
                let bytes;
                match sk.recv(&mut buffer) {
                    Err(_) => {
                        break;
                    }
                    Ok(b) => {
                        bytes = b;
                    }
                }
                reseq.track(&buffer[..bytes]);
            }
            // wait for sender before the socket goes out of scope
            sender.join().expect("wait for sender");

            reseq
        });

        // return reseq from closure
        reseq = receiver.join().expect("wait for receiver");

        assert_eq!(reseq.dups, 0);
        assert_eq!(reseq.missing, []);
        assert_eq!(
            (Wrapping(reseq.last_seq.unwrap()) + Wrapping(2)).0,
            pps / (secs as u32)
        );
    }
}
