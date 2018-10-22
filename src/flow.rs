use std::net::UdpSocket;
use std::thread::sleep;
use std::time::{Duration, Instant};

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

    pub fn start_xmit(&mut self) -> u32 {
        let gap = Duration::new(0, 1_000_000_000 / self.pps);
        let mut underruns = 0u32;

        // self.sk.set_nonblocking(true);
        let mut recycled_buffers =
            vec![vec![0; self.payload_len].into_boxed_slice(); 10];
        let mut prepared_buffers: Vec<Box<[u8]>> = Vec::new();

        // wait relative to sleep_until (as opposed to now()) to
        // compensate for jitter.
        let started_at = Instant::now();
        let mut sleep_until = started_at + gap;

        while self.duration > Instant::now().duration_since(started_at) {
            let mut now = Instant::now();
            while now < sleep_until || prepared_buffers.is_empty() {
                if !recycled_buffers.is_empty() {
                    let mut data = recycled_buffers.pop().unwrap();
                    data = (self.fill_packet)(data).expect("attach payload");
                    prepared_buffers.insert(0, data);
                } else {
                    sleep(sleep_until.duration_since(now));
                }
                now = Instant::now();
            }

            // if sleep_until + gap < now {
            //    println!("missed a time slot by {:?}", now - sleep_until);
            // }

            while sleep_until < now {
                if prepared_buffers.is_empty() {
                    // println!("buffer underrun");
                    underruns += 1;
                    break;
                }
                let data = prepared_buffers.pop().unwrap();
                self.sk.send(&data).expect("transmit datagram");
                recycled_buffers.insert(0, data);

                underruns = 0;
                sleep_until += gap;
            }
        }
        return underruns;
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

    use tests::fresh_pair_of_socks;
    #[test]
    fn flow_xmit() {
        let (sk, sk_rcv) = fresh_pair_of_socks();

        let size = 100;
        let mut buffer = [0; 2000];
        let mut flow = Flow::from_socket(
            125,
            size,
            Duration::from_millis(1),
            |x| Ok(x),
            sk,
        );
        flow.start_xmit();
        assert!(sk_rcv.peek(&mut buffer).expect("peek a dgram") == size);
    }
}
