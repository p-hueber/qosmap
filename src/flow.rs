use std::time::{Duration, Instant};
use std::net::UdpSocket;
use std::thread::sleep;

pub struct Flow<F>
where
    F: FnMut(Box<[u8]>) -> Result<Box<[u8]>, &'static str>,
{
    pps: u32,
    payload_len: usize,
    duration: Duration,
    fill_packet: F,
    sk: UdpSocket,
}

impl<F> Flow<F>
where
    F: FnMut(Box<[u8]>) -> Result<Box<[u8]>, &'static str>,
{
    pub fn from_socket(
        pps: u32,
        payload_len: usize,
        duration: Duration,
        fill_packet: F,
        sk: UdpSocket,
    ) -> Flow<F> {
        Flow {
            pps,
            payload_len,
            duration,
            fill_packet,
            sk,
        }
    }

    pub fn to_socket(self) -> UdpSocket {
        self.sk
    }

    pub fn start_xmit(&mut self) {
        let gap = Duration::new(0, 1_000_000_000 / self.pps);
        let started_at = Instant::now();
        let mut sleep_until = started_at;

        while self.duration > Instant::now().duration_since(started_at) {
            let mut data = vec![0; self.payload_len].into_boxed_slice();

            // wait relative to sleep_until (as opposed to now()) to
            // compensate for jitter.
            sleep_until += gap;

            data = (self.fill_packet)(data).expect("attach payload");
            // capture 'now' and check for a negative duration to avoid panic
            let now = Instant::now();
            if now < sleep_until {
                sleep(sleep_until.duration_since(now));
            }

            self.sk.send(&data);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flow_instance() {
        let sk = UdpSocket::bind("127.0.0.1:0").expect("bind socket");
        let _flow = Flow::from_socket(
            125,
            100,
            Duration::from_secs(10),
            |x| Ok(x),
            sk,
        );
    }

    #[test]
    fn flow_reclaim_socket() {
        let sk = UdpSocket::bind("127.0.0.1:0").expect("bind socket");
        let flow = Flow::from_socket(
            125,
            100,
            Duration::from_secs(10),
            |x| Ok(x),
            sk,
        );
        flow.to_socket();
    }

    #[test]
    fn flow_xmit() {
        let sk = UdpSocket::bind("127.0.0.1:48002").expect("bind socket");
        let sk_rcv = UdpSocket::bind("127.0.0.1:48102").expect("bind socket");
        let size = 100;
        let mut buffer = [0; 2000];
        sk.connect("127.0.0.1:48102");
        let mut flow = Flow::from_socket(
            125,
            size,
            Duration::from_millis(1),
            |x| Ok(x),
            sk,
        );
        flow.start_xmit();
        sk_rcv
            .set_nonblocking(true)
            .expect("set receiver to nonblocking");
        assert!(sk_rcv.peek(&mut buffer).expect("peek a dgram") == size);
    }
}
