pub mod sequence {
    use std;
    use std::num::Wrapping;
    pub struct ReSequencer<T>
    where
        T: ?Sized,
    {
        pub last_seq: Option<u32>,
        pub missing: Vec<(u32, u32)>,
        pub dups: u32,
        // TODO return Result()
        read_seq: fn(&T) -> u32,
    }

    impl<T> ReSequencer<T>
    where
        T: ?Sized,
    {
        pub fn new(read_seq: fn(&T) -> u32) -> ReSequencer<T> {
            ReSequencer {
                last_seq: Option::None,
                missing: vec![],
                dups: 0,
                read_seq,
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
                if v.1 >= seq.0 && v.0 <= seq.0 {
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
                        self.missing.insert(idx + 1, ((seq + one).0, tmp));
                    }
                }
                None => {
                    if (seq - expected).0 > (std::u32::MAX / 2) {
                        self.dups += 1;
                    } else {
                        if expected < seq {
                            self.missing.push((expected.0, (seq - one).0));
                        } else {
                            // split intervals for the wrapping case to
                            // simplify lookup
                            // 
                            self.missing.push((expected.0, std::u32::MAX));
                            if seq.0 != 0 {
                                self.missing.push((0, (seq - one).0));
                            }
                        }
                        self.last_seq = Some(seq.0);
                    }
                }
            }
        }
    }

    pub struct Sequencer<T> {
        pub next_seq: u32,
        mark_data: fn(T, u32) -> T,
    }

    impl<T> Sequencer<T>
    {
        pub fn new(mark_data: fn(T, u32) -> T) -> Sequencer<T> {
            return Sequencer {
                next_seq: std::u32::MAX,
                mark_data,
            };
        }
        pub fn mark(&mut self, data: T) -> T {
            let data_ret = (self.mark_data)(data, self.next_seq);
            self.next_seq = self.step(self.next_seq);
            data_ret
        }
        fn step(&self, s: u32) -> u32 {
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
    use std::boxed::Box;

    fn mark(mut d: Box<u32>, v: u32) -> Box<u32> {
        *d = v;
        d
    }
    fn read_seq(d: &u32) -> u32 {
        *d
    }
    #[test]
    fn seq_instance() {
        let _seq = Sequencer::new(mark);
    }
    #[test]
    fn seq_wrap() {
        let mut s: Box<u32> = Box::new(0);
        let mut seq = Sequencer::new(mark);
        s = seq.mark(s);
        assert_eq!(*s, std::u32::MAX);
        s = seq.mark(s);
        assert_eq!(*s, 0);
    }

    #[test]
    fn reseq_instance() {
        let _reseq = ReSequencer::new(read_seq);
    }

    #[test]
    fn reseq_missing() {
        let mut reseq = ReSequencer::new(read_seq);
        reseq.track(&0u32);
        reseq.track(&2u32);
        assert_eq!(reseq.missing[0], (1, 1));
    }

    #[test]
    fn reseq_missing_wrapping() {
        let mut reseq = ReSequencer::new(read_seq);
        reseq.track(&std::u32::MAX);
        reseq.track(&1u32);
        assert_eq!(reseq.missing[0], (0, 0));
    }

    #[test]
    fn reseq_missing_wrapping_split() {
        let mut reseq = ReSequencer::new(read_seq);
        reseq.track(&(std::u32::MAX - 1));
        reseq.track(&1u32);
        assert_eq!(reseq.missing[0], (std::u32::MAX, std::u32::MAX));
        assert_eq!(reseq.missing[1], (0, 0));
    }

    #[test]
    fn reseq_dup_cur() {
        let mut reseq = ReSequencer::new(read_seq);
        reseq.track(&0u32);
        reseq.track(&0u32);
        assert_eq!(reseq.missing, []);
        assert_eq!(reseq.dups, 1);
    }

    #[test]
    fn reseq_dup_old() {
        let mut reseq = ReSequencer::new(read_seq);
        reseq.track(&2u32);
        reseq.track(&0u32);
        assert_eq!(reseq.missing, []);
        assert_eq!(reseq.dups, 1);
    }

    #[test]
    fn reseq_dup_multiple() {
        let mut reseq = ReSequencer::new(read_seq);
        reseq.track(&8u32);
        reseq.track(&0u32);
        reseq.track(&1u32);
        reseq.track(&3u32);
        reseq.track(&8u32);
        assert_eq!(reseq.missing, []);
        assert_eq!(reseq.dups, 4);
    }

    #[test]
    fn seq_reseq() {
        let mut s: Box<u32> = Box::new(0);
        let mut seq = Sequencer::new(mark);
        let mut reseq = ReSequencer::new(read_seq);

        s = seq.mark(s);
        reseq.track(&s);
        assert_eq!(reseq.missing, []);
        assert_eq!(reseq.dups, 0);

        s = seq.mark(s);
        s = seq.mark(s);
        reseq.track(&s);
        assert_eq!(reseq.missing, [(0, 0)]);
        assert_eq!(reseq.dups, 0);
        assert_eq!(*s, 1);

        s = seq.mark(s);
        reseq.track(&s);
        reseq.track(&s);
        assert_eq!(reseq.missing, [(0, 0)]);
        assert_eq!(reseq.dups, 1);
        assert_eq!(*s, 2);

        s = seq.mark(s);
        s = seq.mark(s);
        s = seq.mark(s);
        s = seq.mark(s);
        s = seq.mark(s);
        s = seq.mark(s);
        reseq.track(&s);
        assert_eq!(reseq.missing, [(0, 0), (3, 7)]);
        assert_eq!(reseq.dups, 1);
        assert_eq!(*s, 8);

        reseq.track(&4u32);
        assert_eq!(reseq.missing, [(0, 0), (3, 3), (5, 7)]);

        reseq.track(&3u32);
        assert_eq!(reseq.missing, [(0, 0), (5, 7)]);

        reseq.track(&5u32);
        assert_eq!(reseq.missing, [(0, 0), (6, 7)]);

        reseq.track(&7u32);
        assert_eq!(reseq.missing, [(0, 0), (6, 6)]);

        reseq.track(&0u32);
        assert_eq!(reseq.missing, [(6, 6)]);

        reseq.track(&6u32);
        assert_eq!(reseq.missing, []);
    }
}
