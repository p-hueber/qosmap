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

