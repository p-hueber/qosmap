#[macro_use]
extern crate serde_derive;

extern crate serde;
extern crate serde_json;

#[macro_use]
extern crate structopt;

mod analyze;
mod flow;

use analyze::sequence::{ReSequencer, SequenceReport, Sequencer};
use flow::Flow;
use std::env;
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::time::Duration;
use structopt::StructOpt;

/// qosmap options
#[derive(StructOpt, Debug)]
struct Opt {
    /// server mode
    #[structopt(short = "s", long = "server")]
    server: bool,
    /// server address
    #[structopt(required_unless = "server")]
    host: Option<String>,
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

trait ControlStream {
    fn send_msg(&mut self, ControlMessage);
    fn recv_msg(&mut self) -> Option<ControlMessage>;
}

impl<T> ControlStream for T
where
    T: Read + Write,
{
    fn send_msg(&mut self, msg: ControlMessage) {
        self.write(&serde_json::to_vec(&msg).unwrap())
            .expect("send message");
        self.write(&[0]).expect("terminate message");
        self.flush().expect("complete datagram");
    }

    fn recv_msg(&mut self) -> Option<ControlMessage> {
        use std::io::{BufRead, BufReader};
        let mut buf_stream = BufReader::new(self);
        let mut message_data: Vec<u8> = Vec::new();
        buf_stream
            .read_until(0, &mut message_data)
            .expect("receive message");
        message_data.pop();
        let message: ControlMessage = serde_json::from_slice(&message_data)
            .expect("deserialize control message");

        Some(message)
    }
}

#[derive(Serialize, Deserialize, Debug)]
enum ControlMessage {
    RequestFlow,
    ExpectFlow(u16),
    Report(SequenceReport),
}

fn main() {
    mainymain(env::args().collect::<Vec<_>>());
}

fn mainymain(args: Vec<String>) {
    let opt = Opt::from_iter(args);
    println!("{:?}", opt);

    let host = match opt.host {
        Some(ref ip) => &ip[..],
        _ => "0.0.0.0",
    };

    if opt.server {
        let tcp_listener = TcpListener::bind((host, opt.port))
            .expect("bind to control port");
        for stream in tcp_listener.incoming() {
            let mut ctrl_sk = stream.unwrap();
            let message = ctrl_sk.recv_msg();
            println!("received message: {:?}", message);

            let sk = UdpSocket::bind((host, 0)).expect("bind server");

            let port = sk.local_addr()
                .expect("get port from receiving socket")
                .port();

            match message {
                Some(ControlMessage::RequestFlow) => {
                    ctrl_sk.send_msg(ControlMessage::ExpectFlow(port));
                }
                _ => (),
            };

            let mut reseq = ReSequencer::new(|buf: &[u8]| {
                (buf[3] as u32) | (buf[2] as u32) << 8 | (buf[1] as u32) << 16
                    | (buf[0] as u32) << 24
            });

            let mut buffer = [0; 2000];

            println!("Wait for incoming flow...");
            sk.peek(&mut buffer)
                .expect("look for available data");
            sk.set_read_timeout(Some(Duration::from_millis(10)))
                .expect("set timeout to detect finished flow");

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
            let report = SequenceReport {
                last_seq: reseq.last_seq.unwrap_or(0),
                missing: reseq.missing,
                dups: reseq.dups,
                cnt: reseq.cnt,
            };
            ctrl_sk.send_msg(ControlMessage::Report(report));
        }
    } else {
        // client
        let mut sock_addrs = (host, opt.port)
            .to_socket_addrs()
            .expect("resolve host");
        let sock_addr = sock_addrs.nth(0).unwrap();
        let len: (u32, u32) = (800, 1200);
        let pps = (
            find_max_pps(sock_addr, len.0 as usize).expect("detect max rate"),
            find_max_pps(sock_addr, len.1 as usize).expect("detect max rate"),
        );

        println!("pps {:?}", pps);
        let net_rate: (i64, i64) =
            ((pps.0 * len.0).into(), (pps.1 * len.1).into());
        let overhead = (net_rate.1 - net_rate.0) / (pps.0 - pps.1) as i64;
        println!("overhead {}", overhead);
        let gross_rate = (
            pps.0 as i64 * (len.0 as i64 + overhead),
            pps.1 as i64 * (len.1 as i64 + overhead),
        );
        println!(
            "gross_rate {:?}",
            gross_rate.0.min(gross_rate.1)
        );
    }
}

fn find_max_pps(sock_addr: SocketAddr, pktlen: usize) -> Result<u32, ()> {
    let mut pps = 1000;
    let secs = 3;
    let mut highest_pps: Option<u32> = None;
    let mut no_update_iters = 0;

    loop {
        let mut ctrl_sk =
            TcpStream::connect(sock_addr).expect("open control connection");
        ctrl_sk.send_msg(ControlMessage::RequestFlow);
        let udp_port = match ctrl_sk.recv_msg() {
            Some(ControlMessage::ExpectFlow(udp_port)) => udp_port,
            _ => panic!("Cannot initiate new flow"),
        };
        let mut sender = UdpSocket::bind("0.0.0.0:0").expect("bind sender");
        sender
            .connect((sock_addr.ip(), udp_port))
            .expect("connect to server");

        let mut seq = Sequencer::new(store_seq);
        let mut flow = Flow::from_socket(
            pps,
            pktlen,
            Duration::from_secs(secs),
            move |mut payload: Box<[u8]>| {
                payload = seq.mark(payload);
                Ok(payload)
            },
            sender,
        );
        println!("run flow with pps {}", pps);
        flow.start_xmit();
        sender = flow.to_socket();

        match ctrl_sk.recv_msg() {
            Some(ControlMessage::Report(r)) => {
                // println!("{:?}", r);
                let next_pps;
                let missing_sum = r.missing
                    .iter()
                    .map(|(a, b)| (b + 1) - a)
                    .fold(0, |a, b| a + b);
                println!("missing_sum={}", missing_sum);
                let lost_pps =
                    (missing_sum + (secs as u32) - 1) / (secs as u32);
                let _passed_pps = pps - lost_pps;
                let passed_pps =
                    (r.cnt - r.dups + (secs as u32) - 1) / (secs as u32);
                println!("pps {} expected {}", passed_pps, _passed_pps);
                if passed_pps > highest_pps.unwrap_or_default()
                    || lost_pps == 0
                {
                    highest_pps = Some(passed_pps);
                    next_pps = passed_pps * 2;
                } else {
                    no_update_iters += 1;
                    // retry slightly above the last limit
                    next_pps = passed_pps + (lost_pps + 1) / 2;
                }
                if no_update_iters >= 3 {
                    println!(
                        "determined rate {} B/s",
                        highest_pps.unwrap_or_default() as u64
                            * pktlen as u64
                    );
                    return Ok(highest_pps.unwrap_or_default());
                } else {
                    pps = next_pps;
                }
            }
            _ => return Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use analyze::sequence::{ReSequencer, Sequencer};
    use flow::Flow;
    use std::net::UdpSocket;
    use std::num::Wrapping;
    use std::thread;
    use std::time::Duration;

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

        let mut seq = Sequencer::new(::store_seq);
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
            sk.set_read_timeout(Some(Duration::from_millis(10)))
                .expect("set timeout");

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
    #[test]
    fn run_main() {
        ::mainymain(vec![String::from("qosmap"), String::from("-h")]);
    }
    #[test]
    fn run_main_server_client() {
        let _server = thread::spawn(|| {
            ::mainymain(vec![String::from("qosmap"), String::from("-s")]);
        });
        thread::sleep(Duration::from_millis(200));
        let client_opts = vec!["qosmap", "127.0.0.1", "-p", "4801"];
        ::mainymain(
            client_opts
                .iter()
                .map(|x| String::from(*x))
                .collect(),
        );
    }
}
