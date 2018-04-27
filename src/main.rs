mod analyze;
mod flow;

use analyze::sequence::{ReSequencer, Sequencer};

fn main() {
    {
        let mut s: u32 = 0;
        let mut seq = Sequencer::new(|d: &mut u32, v| {
            *d = v;
        });
        let mut reseq = ReSequencer::new(|d: &u32| *d);
        seq.mark(&mut s);
        reseq.track(&s);
        seq.mark(&mut s);
        seq.mark(&mut s);
        reseq.track(&s);
        seq.mark(&mut s);
        reseq.track(&s);
        println!("{:?}\n", reseq.missing);
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
    fn store_seq(buf: &mut [u8], seq: u32) {
        buf[0] = ((seq >> 24) & 0xff) as u8;
        buf[1] = ((seq >> 16) & 0xff) as u8;
        buf[2] = ((seq >> 8) & 0xff) as u8;
        buf[3] = ((seq >> 0) & 0xff) as u8;
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

        // capture new sequencer in fill_packet closure
        let fill_packet = move |mut payload: &mut [u8]| {
            seq.mark(&mut payload);
        };

        let mut flow = Flow::from_socket(
            pps,
            10,
            Duration::from_secs(secs),
            fill_packet,
            sk_snd,
        );

        let sender = thread::spawn(move || {
            flow.start_xmit();
        });

        let receiver = thread::spawn(move || {
            let mut buffer = [0; 2000];
            let sk = sk_rcv;
            sk.set_read_timeout(Some(Duration::from_millis(10)));

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
