extern crate serde_json;

pub mod sequence;

use analyze::sequence::Sequencer;
use control::{ControlMessage, ControlStream};
use flow::Flow;
use std::net::{TcpStream, UdpSocket};
use std::time::Duration;


#[derive(Serialize, Deserialize, Debug)]
pub struct SequencedPayload {
    pub seq: u32,
}

impl SequencedPayload {
    pub fn flatten_into(self, buf: &mut [u8]) {
        let mut payload_vec = serde_json::to_vec(&self).unwrap();

        payload_vec.resize(buf.len(), ' ' as u8);
        buf.copy_from_slice(&payload_vec);
    }
}

use std::net::SocketAddr;
pub fn find_max_pps(
    sock_addr: SocketAddr,
    pktlen: usize,
) -> Result<u32, String> {
    let mut pps = 1000;
    let secs = 3;
    let mut highest_pps: Option<u32> = None;
    let mut no_update_iters = 0;

    let mut ctrl_sk =
        TcpStream::connect(sock_addr).expect("open control connection");

    loop {
        ctrl_sk.send_msg(ControlMessage::RequestFlow)?;
        let udp_port;
        loop {
            match ctrl_sk.recv_msg().expect("initiate new flow") {
                ControlMessage::ExpectFlow(p) => {
                    udp_port = p;
                    break;
                }
                _ => (),
            };
        }

        let sender = UdpSocket::bind(("::", 0)).expect("bind sender");
        sender
            .connect((sock_addr.ip(), udp_port))
            .expect("connect to server");

        let mut seq = Sequencer::new();
        let mut flow = Flow::from_socket(
            pps,
            pktlen,
            Duration::from_secs(secs),
            // XXX this whole concept doesn't look very efficient
            move |mut buf: Box<[u8]>| {
                let payload = SequencedPayload {
                    seq: seq.next_seq(),
                };
                payload.flatten_into(&mut buf);
                Ok(buf)
            },
            sender,
        );
        println!("run flow with pps {}", pps);
        let underruns = flow.start_xmit();
        if underruns > 0 {
            return Err(format!(
                "Could not generate the requested rate of {} pps",
                pps
            ));
        }

        ctrl_sk.send_msg(ControlMessage::TerminateFlow(udp_port))?;
        match ctrl_sk.recv_msg() {
            Ok(ControlMessage::Report(r)) => {
                // println!("{:?}", r);
                let next_pps;
                let missing_sum = r
                    .missing
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
            _ => return Err("unknown control message received".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::sequence::{ReSequencer, Sequencer};
    use std;

    #[test]
    fn seq_instance() {
        let _seq: Sequencer<u8> = Sequencer::new();
    }

    #[test]
    fn seq_wrap() {
        let mut seq = Sequencer::new();
        let mut s: u8 = seq.next_seq();
        assert_eq!(s, u8::default());
        for _ in 0..=std::u8::MAX {
            s = seq.next_seq();
        }
        assert_eq!(s, u8::default());
    }

    #[test]
    fn reseq_instance() {
        let _reseq: ReSequencer<usize> = ReSequencer::new();
    }

    #[test]
    fn reseq_missing() {
        let mut reseq = ReSequencer::new();
        reseq.track(0u32);
        reseq.track(2u32);
        assert_eq!(reseq.missing[0], (1, 1));
    }

    #[test]
    fn reseq_missing_wrapping() {
        let mut reseq = ReSequencer::new();
        reseq.track(std::u32::MAX);
        reseq.track(1u32);
        assert_eq!(reseq.missing[0], (0, 0));
    }

    #[test]
    fn reseq_missing_wrapping_split() {
        let mut reseq = ReSequencer::new();
        reseq.track(std::u32::MAX - 1);
        reseq.track(1u32);
        assert_eq!(reseq.missing[0], (std::u32::MAX, std::u32::MAX));
        assert_eq!(reseq.missing[1], (0, 0));
    }

    #[test]
    fn reseq_dup_cur() {
        let mut reseq = ReSequencer::new();
        reseq.track(0u32);
        reseq.track(0u32);
        assert_eq!(reseq.missing, []);
        assert_eq!(reseq.dups, 1);
    }

    #[test]
    fn reseq_dup_old() {
        let mut reseq = ReSequencer::new();
        reseq.track(2u32);
        reseq.track(0u32);
        assert_eq!(reseq.missing, []);
        assert_eq!(reseq.dups, 1);
    }

    #[test]
    fn reseq_dup_multiple() {
        let mut reseq = ReSequencer::new();
        reseq.track(8u32);
        reseq.track(0u32);
        reseq.track(1u32);
        reseq.track(3u32);
        reseq.track(8u32);
        assert_eq!(reseq.missing, []);
        assert_eq!(reseq.dups, 4);
    }

    #[test]
    fn seq_reseq() {
        let mut s: u32;
        let mut seq = Sequencer::new();
        let mut reseq = ReSequencer::new();

        s = seq.next_seq();
        reseq.track(s);
        assert_eq!(reseq.missing, []);
        assert_eq!(reseq.dups, 0);

        seq.next_seq();
        s = seq.next_seq();
        reseq.track(s);
        assert_eq!(reseq.missing, [(1, 1)]);
        assert_eq!(reseq.dups, 0);
        assert_eq!(s, 2);

        s = seq.next_seq();
        reseq.track(s);
        reseq.track(s);
        assert_eq!(reseq.missing, [(1, 1)]);
        assert_eq!(reseq.dups, 1);
        assert_eq!(s, 3);

        seq.next_seq();
        seq.next_seq();
        seq.next_seq();
        seq.next_seq();
        seq.next_seq();
        s = seq.next_seq();
        reseq.track(s);
        assert_eq!(reseq.missing, [(1, 1), (4, 8)]);
        assert_eq!(reseq.dups, 1);
        assert_eq!(s, 9);

        reseq.track(5u32);
        assert_eq!(reseq.missing, [(1, 1), (4, 4), (6, 8)]);

        reseq.track(4u32);
        assert_eq!(reseq.missing, [(1, 1), (6, 8)]);

        reseq.track(6u32);
        assert_eq!(reseq.missing, [(1, 1), (7, 8)]);

        reseq.track(8u32);
        assert_eq!(reseq.missing, [(1, 1), (7, 7)]);

        reseq.track(1u32);
        assert_eq!(reseq.missing, [(7, 7)]);

        reseq.track(7u32);
        assert_eq!(reseq.missing, []);
    }
}
