pub mod sequence {
    use std::num::Wrapping;
    use std::ops::Add;
    use std::ops::Sub;

    #[derive(Serialize, Deserialize, Debug)]
    pub struct SequenceReport {
        pub last_seq: u32,
        pub missing: Vec<(u32, u32)>,
        pub dups: u32,
        pub cnt: u32,
    }

    pub struct ReSequencer<T>
    where
        Wrapping<T>: Add<Output = Wrapping<T>>,
        T: Default + Copy + From<u8>,
    {
        pub last_seq: Option<T>,
        pub missing: Vec<(T, T)>,
        pub dups: u32,
        pub cnt: u32,
    }

    impl<T> ReSequencer<T>
    where
        Wrapping<T>: Add<Output = Wrapping<T>> + Sub<Output = Wrapping<T>>,
        T: PartialEq + PartialOrd + Default + Copy + From<u8>,
    {
        pub fn new() -> ReSequencer<T> {
            ReSequencer {
                last_seq: Option::None,
                missing: vec![],
                dups: 0,
                cnt: 0,
            }
        }
        pub fn track(&mut self, seq: T) {
            let zero = T::from(0u8);
            let one = T::from(1u8);
            let max = (Wrapping(zero) - Wrapping(one)).0;
            let expected: Wrapping<T>;

            self.cnt += 1;

            if self.last_seq == Option::None {
                self.last_seq = Some(seq);
                return;
            } else {
                expected = Wrapping(self.last_seq.unwrap()) + Wrapping(one);
            }

            if expected.0 == seq {
                self.last_seq = Some(seq);
                return;
            }
            let mut found: Option<usize> = None;
            for (idx, v) in self.missing.iter_mut().enumerate() {
                if v.1 >= seq && v.0 <= seq {
                    found = Some(idx);
                    break;
                }
            }

            match found {
                Some(idx) => {
                    let v = self.missing[idx];
                    if v.0 == v.1 {
                        self.missing.remove(idx);
                    } else if v.0 == seq {
                        self.missing[idx].0 =
                            (Wrapping(seq) + Wrapping(one)).0;
                    } else if v.1 == seq {
                        self.missing[idx].1 =
                            (Wrapping(seq) - Wrapping(one)).0;
                    } else {
                        let tmp = v.1;
                        self.missing[idx].1 =
                            (Wrapping(seq) - Wrapping(one)).0;
                        self.missing.insert(
                            idx + 1,
                            ((Wrapping(seq) + Wrapping(one)).0, tmp),
                        );
                    }
                }
                None => {
                    let distance = Wrapping(seq) - expected;
                    if (distance + distance).0 < distance.0 {
                        self.dups += 1;
                    } else {
                        if expected.0 < seq {
                            self.missing.push((
                                expected.0,
                                (Wrapping(seq) - Wrapping(one)).0,
                            ));
                        } else {
                            // split intervals for the wrapping case to
                            // simplify lookup
                            //
                            self.missing.push((expected.0, max));
                            if seq != zero {
                                self.missing.push((
                                    zero,
                                    (Wrapping(seq) - Wrapping(one)).0,
                                ));
                            }
                        }
                        self.last_seq = Some(seq);
                    }
                }
            }
        }
    }

    pub struct Sequencer<T> {
        seq: T,
    }

    impl<T> Sequencer<T>
    where
        Wrapping<T>: Add<Output = Wrapping<T>>,
        T: Default + Copy + From<u8>,
    {
        pub fn new() -> Sequencer<T> {
            return Sequencer { seq: T::default() };
        }
        pub fn next_seq(&mut self) -> T {
            let ret = self.seq;
            let one: T = T::from(1u8);
            self.seq = (Wrapping(self.seq) + Wrapping(one)).0;
            ret
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
