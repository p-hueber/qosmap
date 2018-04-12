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
