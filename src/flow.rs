use std;
use std::time::{Duration, Instant};
use std::net::UdpSocket;
use std::thread::sleep;

pub struct Flow {
    pps: u32,
    payload_len: usize,
    duration: Duration,
    sk: UdpSocket,
}

impl Flow {
    pub fn from_socket(pps: u32, payload_len: usize, duration: Duration, sk: UdpSocket) -> Flow {
        Flow {
            pps,
            payload_len,
            duration,
            sk,
        }
    }
    pub fn to_socket(self) -> UdpSocket {
        self.sk
    }

    pub fn start_xmit(&self) {
        let gap = Duration::new(0, 1_000_000_000/ self.pps );
        let started_at = Instant::now();

        while self.duration > Instant::now().duration_since(started_at) {
            let data = vec![0; self.payload_len];
            sleep(gap);
            self.sk.send(& data);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flow_instance() {
        let mut sk = UdpSocket::bind("127.0.0.1:0").expect("bind socket");
        let _flow =
            Flow::from_socket(125, 100, Duration::from_secs(10), |_| {}, sk);
    }

    #[test]
    fn flow_reclaim_socket() {
        let mut sk = UdpSocket::bind("127.0.0.1:0").expect("bind socket");
        let flow =
            Flow::from_socket(125, 100, Duration::from_secs(10), |_| {}, sk);
        sk = flow.to_socket();
    }

    #[test]
    fn flow_xmit() {
        let mut sk = UdpSocket::bind("127.0.0.1:48002").expect("bind socket");
        let mut sk_rcv = UdpSocket::bind("127.0.0.1:48102").expect("bind socket");
        let size = 100;
        let mut buffer = [0;2000];
        sk.connect("127.0.0.1:48102");
        let mut flow = Flow::from_socket(
            125,
            size,
            Duration::from_millis(1),
            |_| {},
            sk,
        );
        flow.start_xmit();
        sk_rcv.set_nonblocking(true).expect("set receiver to nonblocking");
        assert!(sk_rcv.peek(&mut buffer).expect("peek a dgram") == size);
    }
}
