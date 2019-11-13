#[macro_use]
extern crate serde_derive;

extern crate serde;
extern crate serde_json;

#[macro_use]
extern crate structopt;

mod analyze;
mod control;
mod flow;

use analyze::sequence::{ReSequencer, SequenceReport};
use analyze::SequencedPayload;
use control::{ControlMessage, ControlStream};
use std::env;
use std::net::ToSocketAddrs;
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::mpsc;
use std::thread;
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

fn main() {
    mainymain(env::args().collect::<Vec<_>>());
}

fn mainymain(args: Vec<String>) {
    let opt = Opt::from_iter(args);
    println!("{:?}", opt);

    let host = match opt.host {
        Some(ref ip) => &ip[..],
        _ => "::",
    };

    if opt.server {
        let tcp_listener = TcpListener::bind((host, opt.port))
            .expect("bind to control port");
        for stream in tcp_listener.incoming() {
            let mut ctrl_sk = stream.unwrap();
            thread::spawn(move || {
                let peer: String =
                    format!("{:?}", ctrl_sk.peer_addr().unwrap());
                serve_client(ctrl_sk).unwrap_or_else(|e| {
                    println!("Error for connection from {}: {}", peer, e);
                });
            });
        }
    } else {
        use analyze::find_max_pps;
        // client
        let mut sock_addrs =
            (host, opt.port).to_socket_addrs().expect("resolve host");
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
        println!("gross_rate {:?}", gross_rate.0.min(gross_rate.1));
    }
}

fn receive_flow<T>(sk: UdpSocket, mut abort_cond: T) -> SequenceReport
where
    T: FnMut() -> bool + Sized,
{
    let mut reseq = ReSequencer::new();

    let mut buffer = [0; 2000];

    println!("Wait for incoming flow...");
    sk.peek(&mut buffer).expect("look for available data");
    sk.set_read_timeout(Some(Duration::from_millis(1000)))
        .expect("set timeout to detect finished flow");

    println!("Receive flow...");
    loop {
        let bytes;
        match sk.recv(&mut buffer) {
            Err(_) => {
                // XXX check abort condition after timeout only
                println!("check abort_cond");
                if abort_cond() {
                    println!("abort");
                    break;
                } else {
                    println!("continue");
                    continue;
                }
            }
            Ok(b) => {
                bytes = b;
            }
        }
        let payload: SequencedPayload =
            serde_json::from_slice(&buffer[..bytes]).unwrap();
        reseq.track(payload.seq);
    }

    SequenceReport {
        last_seq: reseq.last_seq.unwrap_or(0),
        missing: reseq.missing,
        dups: reseq.dups,
        cnt: reseq.cnt,
    }
}

fn serve_client(mut ctrl_sk: TcpStream) -> Result<(), String> {
    let mut workers: Vec<FlowWorker> = Vec::new();

    let host = ctrl_sk
        .local_addr()
        .expect("derive local ip from ctrl socket")
        .ip();

    loop {
        let message = ctrl_sk.recv_msg()?;
        println!("received message: {:?}", message);

        match message {
            ControlMessage::RequestFlow => {
                let w = spawn_flow_worker(host)?;
                ctrl_sk.send_msg(ControlMessage::ExpectFlow(w.port))?;
                workers.push(w);
            }
            ControlMessage::TerminateFlow(port) => {
                let pos = workers
                    .iter()
                    .position(|w| w.port == port)
                    .ok_or("no flow served for that port")?;
                let w = workers.remove(pos);
                w.worker_in
                    .send(ControlMessage::TerminateFlow(port))
                    .map_err(|e| e.to_string())?;
                w.worker_out
                    .recv()
                    .map_err(|e| e.to_string())
                    .and_then(|msg| ctrl_sk.send_msg(msg))?;
                w.worker.join().expect("wait for worker thread")?
            }
            _ => {
                return Err("unsupported control message received".to_string())
            }
        };
    }
}

struct FlowWorker {
    worker: thread::JoinHandle<Result<(), String>>,
    worker_in: mpsc::Sender<ControlMessage>,
    worker_out: mpsc::Receiver<ControlMessage>,
    port: u16,
}

fn spawn_flow_worker(host: std::net::IpAddr) -> Result<FlowWorker, String> {
    let sk = UdpSocket::bind((host, 0)).map_err(|e| e.to_string())?;

    let port = sk
        .local_addr()
        .expect("get port from receiving socket")
        .port();
    let (worker_in_prod, worker_in_cons) = mpsc::channel::<ControlMessage>();
    let (worker_out_prod, worker_out_cons) =
        mpsc::channel::<ControlMessage>();

    let worker = thread::spawn(move || -> Result<(), String> {
        let report = receive_flow(sk, || match worker_in_cons.try_recv() {
            Ok(ControlMessage::TerminateFlow(_)) => true,
            Err(mpsc::TryRecvError::Disconnected) => true,
            _ => false,
        });

        worker_out_prod
            .send(ControlMessage::Report(report))
            .map_err(|e| e.to_string())?;

        Ok(())
    });

    Ok(FlowWorker {
        worker,
        worker_in: worker_in_prod,
        worker_out: worker_out_cons,
        port,
    })
}

#[cfg(test)]
mod tests {
    use analyze::sequence::{ReSequencer, Sequencer};
    use analyze::SequencedPayload;
    use flow::Flow;
    use std::net::UdpSocket;
    use std::num::Wrapping;
    use std::thread;
    use std::time::Duration;

    pub fn fresh_pair_of_socks() -> (UdpSocket, UdpSocket) {
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

        let mut seq = Sequencer::new();
        let mut reseq = ReSequencer::new();
        let pps = 1000;
        let secs = 1;

        let mut flow = Flow::from_socket(
            pps,
            100,
            Duration::from_secs(secs),
            move |mut buf: Box<[u8]>| {
                let payload = SequencedPayload {
                    seq: seq.next_seq(),
                };
                payload.flatten_into(&mut buf);
                Ok(buf)
            },
            sk_snd,
        );

        let sender = thread::spawn(move || {
            flow.start_xmit();
        });

        let receiver = thread::spawn(move || {
            let mut buffer = [0; 2000];
            let sk = sk_rcv;
            sk.set_read_timeout(Some(Duration::from_millis(500)))
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
                let payload: SequencedPayload =
                    ::serde_json::from_slice(&buffer[..bytes])
                        .unwrap_or_else(|_| {
                            println!("bytes: {}", bytes);
                            SequencedPayload { seq: 0u32 }
                        });
                reseq.track(payload.seq);
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
            (Wrapping(reseq.last_seq.unwrap_or_default()) + Wrapping(1)).0,
            pps / (secs as u32)
        );
    }
    //#[test]
    // fn run_main() {
    //   ::mainymain(vec![String::from("qosmap"), String::from("-h")]);
    // }
    #[test]
    #[should_panic(expected = "generate the requested rate")]
    fn run_main_server_client() {
        let _server = thread::spawn(|| {
            ::mainymain(vec![String::from("qosmap"), String::from("-s")]);
        });
        thread::sleep(Duration::from_millis(200));
        let client_opts = vec!["qosmap", "127.0.0.1", "-p", "4801"];
        ::mainymain(client_opts.iter().map(|x| String::from(*x)).collect());
    }
}
