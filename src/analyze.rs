pub mod sequence {
    use std;
    use std::num::Wrapping;
    pub struct ReSequencer<T> {
        last_seq: Option<u32>,
        pub missing: Vec<(u32, u32)>,
        pub dups: u32,
        // TODO return Result()
        read_seq: fn(& T) -> u32,
    }

    impl<T> ReSequencer<T> {
        pub fn new(read_seq: fn(& T) -> u32) -> ReSequencer<T> {
            ReSequencer{
                last_seq: Option::None,
                missing: vec![],
                dups: 0,
                read_seq
            }
        }
        pub fn track(&mut self, data: &T) {
            let one = Wrapping(1u32);
            let seq = Wrapping((self.read_seq)(data));
            let expected: Wrapping<u32>;
            
            if self.last_seq == Option::None {
                self.last_seq = Some(seq.0);
                return;
            } else {
                expected = Wrapping(self.last_seq.unwrap()) + one;
            }
            
            if expected == seq {
                self.last_seq = Some(seq.0);
                return;
            }
            let mut found: Option<usize> = None;
            for (idx, v) in self.missing.iter_mut().enumerate() {
                if v.1 >= seq.0 && seq.0 <= v.0 {
                    found = Some(idx);
                    break;
                }
            }
                
            match found {
                Some(idx) => {
                    let v = self.missing[idx];
                    if v.0 == v.1 {
                        self.missing.remove(idx);
                    } else if v.0 == seq.0 {
                        self.missing[idx].0 = (seq + one).0;
                    } else if v.1 == seq.0 {
                        self.missing[idx].1 = (seq - one).0;
                    } else {
                        let tmp = v.1;
                        self.missing[idx].1 = (seq - one).0;
                        self.missing.insert(idx, ((seq + one).0, tmp));
                    }
                },
                None => {
                    if (seq - expected).0 > (std::u32::MAX/2) {
                        self.dups += 1;
                    } else {
                        if expected < seq {
                            self.missing.push((expected.0, (seq-one).0));
                        } else {
                            /* 
                             * split intervals for the wrapping case to simplify
                             * lookup
                             */
                            self.missing.push((expected.0, std::u32::MAX));
                            if seq.0 != 0 {
                                self.missing.push((0, (seq-one).0));
                            }
                        }
                        self.last_seq = Some(seq.0);
                    }
                },
            }
        }
    }

    pub struct Sequencer<T> {
        next_seq: u32,
        mark_data: fn(&mut T, u32),
    }

    impl<T> Sequencer<T> {
        pub fn new(mark_data: fn(&mut T, u32)) -> Sequencer<T> {
            return Sequencer{ next_seq: std::u32::MAX, mark_data };
        }
        pub fn mark(&mut self, data: &mut T) {
           (self.mark_data)(data, self.next_seq); 
           self.next_seq = self.step(self.next_seq);
        }
        fn step(& self, s: u32) -> u32 {
            if s == std::u32::MAX {
                0
            } else {
                s + 1
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::sequence::*;
    use std;
    
    fn mark(d: &mut u32, v: u32) {
        *d = v;
    }
    fn read_seq(d: & u32) -> u32 {
        *d
    }
    #[test]
    fn seq_instance() {
        let _seq = Sequencer::new(mark);
    }
    #[test]
    fn seq_wrap() {
        let mut seq = Sequencer::new(mark);
        let mut s: u32 = 0;
        seq.mark(&mut s);
        assert_eq!(s, std::u32::MAX);
        seq.mark(&mut s);
        assert_eq!(s, 0);
    }
    
    #[test]
    fn reseq_instance() {
        let _reseq = ReSequencer::new(read_seq);
    }
    
    #[test]
    fn reseq_missing() {
        let mut reseq = ReSequencer::new(read_seq);
        reseq.track(& 0u32);
        reseq.track(& 2u32);
        assert_eq!(reseq.missing[0], (1, 1));
    }
    
    #[test]
    fn reseq_missing_wrapping() {
        let mut reseq = ReSequencer::new(read_seq);
        reseq.track(& std::u32::MAX);
        reseq.track(& 1u32);
        assert_eq!(reseq.missing[0], (0, 0));
    }
    
    #[test]
    fn reseq_missing_wrapping_split() {
        let mut reseq = ReSequencer::new(read_seq);
        reseq.track(& (std::u32::MAX - 1));
        reseq.track(& 1u32);
        assert_eq!(reseq.missing[0], (std::u32::MAX, std::u32::MAX));
        assert_eq!(reseq.missing[1], (0, 0));
    }
    
    #[test]
    fn reseq_dup_cur() {
        let mut reseq = ReSequencer::new(read_seq);
        reseq.track(& 0u32);
        reseq.track(& 0u32);
        assert_eq!(reseq.missing, []);
        assert_eq!(reseq.dups, 1);
    }
    
    #[test]
    fn reseq_dup_old() {
        let mut reseq = ReSequencer::new(read_seq);
        reseq.track(& 2u32);
        reseq.track(& 0u32);
        assert_eq!(reseq.missing, []);
        assert_eq!(reseq.dups, 1);
    }
    
    #[test]
    fn reseq_dup_multiple() {
        let mut reseq = ReSequencer::new(read_seq);
        reseq.track(& 8u32);
        reseq.track(& 0u32);
        reseq.track(& 1u32);
        reseq.track(& 3u32);
        reseq.track(& 8u32);
        assert_eq!(reseq.missing, []);
        assert_eq!(reseq.dups, 4);
    }
}
