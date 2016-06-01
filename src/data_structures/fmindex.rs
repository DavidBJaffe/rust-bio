// Cloneright 2014 Johannes Köster.
// Licensed under the MIT license (http://opensource.org/licenses/MIT)
// This file may not be copied, modified, or distributed
// except according to those terms.

//! FM-Index and FMD-Index for finding suffix array intervals matching a given pattern in linear time.

use std::iter::DoubleEndedIterator;

use data_structures::bwt::{less, BWT, DerefBWT, DerefOcc, DerefLess};
use data_structures::suffix_array::{SuffixArray, DerefSuffixArray};
use alphabets::dna;
use std::mem::swap;

/// A suffix array interval.
#[derive(Clone, Copy, Debug)]
pub struct Interval {
    pub lower: usize,
    pub upper: usize,
}

impl Interval {
  pub fn occ<SA: SuffixArray, T: DerefSuffixArray<SA>>(&self, sa: T) -> Vec<usize> {
      (self.lower..self.upper)
          .map(|pos| sa.get(pos)
                        .expect("Interval out of range of suffix array"))
          .collect()
  }
}

pub trait FMIndexable {
    /// Get occurrence count of symbol a in BWT[..r+1].
    fn occ(&self, r: usize, a: u8) -> usize;
    /// Also known as
    fn less(&self, a: u8) -> usize;
    fn bwt(&self) -> &BWT;

    /// Perform backward search, yielding suffix array
    /// interval denoting exact occurrences of the given pattern of length m in the text.
    /// Complexity: O(m).
    ///
    /// # Arguments
    ///
    /// * `pattern` - the pattern to search
    ///
    /// # Example
    ///
    /// ```
    /// use bio::data_structures::bwt::{bwt, less, Occ};
    /// use bio::data_structures::fmindex::{FMIndex, FMIndexable};
    /// use bio::data_structures::suffix_array::suffix_array;
    /// use bio::alphabets::dna;
    /// use std::rc::Rc;
    ///
    /// let text = b"GCCTTAACATTATTACGCCTA$";
    /// let alphabet = dna::n_alphabet();
    /// let sa = Rc::new(suffix_array(text));
    /// let bwt = Rc::new(bwt(text, sa.clone()));
    /// let less = Rc::new(less(&bwt, &alphabet));
    /// let occ = Rc::new(Occ::new(&bwt, 3, &alphabet));
    /// let fm = FMIndex::new(bwt, less, occ);
    ///
    /// let pattern = b"TTA";
    /// let sai = fm.backward_search(pattern.iter());
    ///
    /// let occ = sai.occ(sa.clone());
    ///
    /// assert_eq!(occ, [3, 12, 9]);
    /// ```
    fn backward_search<'b, P: Iterator<Item = &'b u8> + DoubleEndedIterator> (&self, pattern: P) -> Interval {
        let (mut l, mut r) = (0, self.bwt().len() - 1);
        for &a in pattern.rev() {
            let less = self.less(a);
            l = less +
                if l > 0 {
                self.occ(l - 1, a)
            } else {
                0
            };
            r = less + self.occ(r, a) - 1;
        }

        Interval {
            lower: l,
            upper: r + 1,
        }
    }


}

/// The Fast Index in Minute space (FM-Index, Ferragina and Manzini, 2000) for finding suffix array
/// intervals matching a given pattern.

#[cfg_attr(feature = "serde_macros", derive(Serialize, Deserialize))]
pub struct FMIndex<
        DBWT: DerefBWT + Clone,
        DLess: DerefLess + Clone,
        DOcc: DerefOcc + Clone> {
    bwt: DBWT,
    less: DLess,
    occ: DOcc,
}

impl<
    DBWT: DerefBWT + Clone,
    DLess: DerefLess + Clone,
    DOcc: DerefOcc + Clone> FMIndexable for FMIndex<DBWT, DLess, DOcc> {

    fn occ(&self, r: usize, a: u8) -> usize {
        self.occ.get(&self.bwt, r, a)
    }
    fn less(&self, a: u8) -> usize {
        self.less[a as usize]
    }
    /// Provide a reference to the underlying BWT.
    fn bwt(&self) -> &BWT {
        &self.bwt
    }
}

impl<
    DBWT: DerefBWT + Clone,
    DLess: DerefLess + Clone,
    DOcc: DerefOcc + Clone> FMIndex<DBWT, DLess, DOcc> {

    /// Construct a new instance of the FM index.
    ///
    /// # Arguments
    ///
    /// * `sa` - the suffix array (or sample)
    /// * `bwt` - the BWT
    /// * `k` - the sampling rate of the occ array: every k-th entry will be stored (higher k means
    ///   less memory usage, but worse performance)
    /// * `alphabet` - the alphabet of the underlying text, omitting the sentinel
    pub fn new(bwt: DBWT, less: DLess, occ: DOcc) -> Self {
        FMIndex {
            bwt: bwt,
            less: less,
            occ: occ,
        }
    }

    /// Construct a new instance of the FMD index (see Heng Li (2012) Bioinformatics).
    /// This expects a BWT that was created from a text over the DNA alphabet with N
    /// (`alphabets::dna::n_alphabet()`) consisting of the
    /// concatenation with its reverse complement, separated by the sentinel symbol `$`.
    /// I.e., let T be the original text and R be its reverse complement.
    /// Then, the expected text is T$R$. Further, multiple concatenated texts are allowed, e.g.
    /// T1$R1$T2$R2$T3$R3$.
    ///
    pub fn as_fmdindex(self) -> FMDIndex<DBWT, DLess, DOcc> {
        let mut alphabet = dna::n_alphabet();
        alphabet.insert(b'$');
        assert!(alphabet.is_word(self.bwt()),
                "Expecting BWT over the DNA alphabet (including N) with the sentinel $.");

        FMDIndex {
            fmindex: self,
        }
    }
}

/// A bi-interval on suffix array of the forward and reverse strand of a DNA text.
#[derive(Clone, Copy, Debug)]
pub struct BiInterval {
    lower: usize,
    lower_rev: usize,
    size: usize,
    match_size: usize,
}

impl BiInterval {
    pub fn forward(&self) -> Interval {
        Interval {
            upper: self.lower + self.size,
            lower: self.lower
        }
    }
    pub fn revcomp(&self) -> Interval {
        Interval {
            upper: self.lower_rev + self.size,
            lower: self.lower_rev
        }
    }

    fn swapped(&self) -> BiInterval {
        BiInterval {
            lower: self.lower_rev,
            lower_rev: self.lower,
            size: self.size,
            match_size: self.match_size,
        }
    }
}


/// The FMD-Index for linear time search of supermaximal exact matches on forward and reverse
/// strand of DNA texts (Li, 2012).
#[cfg_attr(feature = "serde_macros", derive(Serialize, Deserialize))]
pub struct FMDIndex<
    DBWT: DerefBWT + Clone,
    DLess: DerefLess + Clone,
    DOcc: DerefOcc + Clone> {

    fmindex: FMIndex<DBWT, DLess, DOcc>,
}

impl<
    DBWT: DerefBWT + Clone,
    DLess: DerefLess + Clone,
    DOcc: DerefOcc + Clone> FMIndexable for FMDIndex<DBWT, DLess, DOcc> {

    fn occ(&self, r: usize, a: u8) -> usize {
        self.fmindex.occ(r, a)
    }

    fn less(&self, a: u8) -> usize {
        self.fmindex.less(a)
    }

    /// Provide a reference to the underlying BWT.
    fn bwt(&self) -> &BWT {
        self.fmindex.bwt()
    }
}

impl<
    DBWT: DerefBWT + Clone,
    DLess: DerefLess + Clone,
    DOcc: DerefOcc + Clone>  FMDIndex<DBWT, DLess, DOcc> {

    /// Find supermaximal exact matches of given pattern that overlap position i in the pattern.
    /// Complexity O(m) with pattern of length m.
    ///
    /// # Example
    ///
    /// ```
    /// use bio::alphabets::dna;
    /// use bio::data_structures::fmindex::FMIndex;
    /// use bio::data_structures::suffix_array::suffix_array;
    /// use bio::data_structures::bwt::{bwt, less, Occ};
    /// use std::rc::Rc;
    ///
    /// let text = b"ATTC$GAAT$";
    /// let alphabet = dna::n_alphabet();
    /// let sa = Rc::new(suffix_array(text));
    /// let bwt = Rc::new(bwt(text, sa.clone()));
    /// let less = Rc::new(less(&bwt, &alphabet));
    /// let occ = Rc::new(Occ::new(&bwt, 3, &alphabet));
    /// let fm = FMIndex::new(bwt, less, occ);
    /// let fmdindex = fm.as_fmdindex();
    ///
    /// let pattern = b"ATT";
    /// let intervals = fmdindex.smems(pattern, 2);
    ///
    /// let forward_occ = intervals[0].forward().occ(sa.clone());
    ///
    /// let revcomp_occ = intervals[0].revcomp().occ(sa.clone());
    ///
    /// assert_eq!(forward_occ, [0]);
    /// assert_eq!(revcomp_occ, [6]);
    /// ```
    pub fn smems(&self, pattern: &[u8], i: usize) -> Vec<BiInterval> {

        let curr = &mut Vec::new();
        let prev = &mut Vec::new();
        let mut matches = Vec::new();

        let mut interval = self.init_interval(pattern, i);

        for &a in pattern[i + 1..].iter() {
            // forward extend interval
            let forward_interval = self.forward_ext(&interval, a);

            // if size changed, add last interval to list
            if interval.size != forward_interval.size {
                curr.push(interval.clone());
            }
            // if new interval size is zero, stop, as no further forward extension is possible
            if forward_interval.size == 0 {
                break;
            }
            interval = forward_interval;
        }
        // add the last non-zero interval
        curr.push(interval.clone());
        // reverse intervals such that longest comes first
        curr.reverse();

        swap(curr, prev);
        let mut j = pattern.len() as isize;

        for k in (-1..i as isize).rev() {
            let a = if k == -1 {
                b'$'
            } else {
                pattern[k as usize]
            };
            curr.clear();
            // size of the last confirmed interval
            let mut last_size = -1;

            for interval in prev.iter() {
                // backward extend interval
                let forward_interval = self.backward_ext(&interval, a);

                if (forward_interval.size == 0 || k == -1) &&
                        // interval could not be extended further
                        // if no interval has been extended this iteration,
                        // interval is maximal and can be added to the matches
                        curr.is_empty() && k < j {
                    j = k;
                    matches.push((*interval).clone());
                }
                // add _interval to curr (will be further extended next iteration)
                if forward_interval.size != 0 && forward_interval.size as isize != last_size {
                    last_size = forward_interval.size as isize;
                    curr.push(forward_interval);
                }
            }
            if curr.is_empty() {
                break;
            }
            swap(curr, prev);
        }

        matches
    }

    fn init_interval(&self, pattern: &[u8], i: usize) -> BiInterval {
        let a = pattern[i];
        let comp_a = dna::complement(a);
        let lower = self.fmindex.less(a);

        BiInterval {
            lower: lower,
            lower_rev: self.fmindex.less(comp_a),
            size: self.fmindex.less(a + 1) - lower,
            match_size: 1,
        }
    }

    fn backward_ext(&self, interval: &BiInterval, a: u8) -> BiInterval {
        let mut s = 0;
        let mut o = 0;
        let mut l = interval.lower_rev;
        // Interval [l(c(aP)), u(c(aP))] is a subinterval of [l(c(P)), u(c(P))] for each a,
        // starting with the lexicographically smallest ($),
        // then c(T) = A, c(G) = C, c(C) = G, N, c(A) = T, ...
        // Hence, we calculate lower revcomp bounds by iterating over
        // symbols and updating from previous one.
        for &b in b"$TGCNAtgcna".iter() {
            l = l + s;
            o = self.fmindex.occ(interval.lower - 1, b);
            // calculate size
            s = self.fmindex.occ(interval.lower + interval.size - 1, b) - o;
            if b == a {
                break;
            }
        }
        // calculate lower bound
        let k = self.fmindex.less(a) + o;

        BiInterval {
            lower: k,
            lower_rev: l,
            size: s,
            match_size: interval.match_size + 1,
        }
    }


    fn forward_ext(&self, interval: &BiInterval, a: u8) -> BiInterval {
        let comp_a = dna::complement(a);

        self.backward_ext(&interval.swapped(), comp_a)
            .swapped()
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use alphabets::dna;
    use data_structures::suffix_array::suffix_array;
    use data_structures::bwt::{bwt, less, Occ};
    use std::rc::Rc;

    #[test]
    fn test_smems() {
        let orig_text = b"GCCTTAACAT";
        let revcomp_text = dna::revcomp(orig_text);
        let text_builder: Vec<&[u8]> = vec![orig_text, b"$", &revcomp_text[..], b"$"];
        let text = text_builder.concat();

        let alphabet = dna::n_alphabet();
        let sa = Rc::new(suffix_array(&text));
        let bwt = Rc::new(bwt(&text, sa.clone()));
        let less = Rc::new(less(&bwt, &alphabet));
        let occ = Rc::new(Occ::new(&bwt, 3, &alphabet));

        let fmindex = FMIndex::new(bwt, less, occ);
        let fmdindex = fmindex.as_fmdindex();
        {
            let pattern = b"AA";
            let intervals = fmdindex.smems(pattern, 0);
            let forward = intervals[0].forward();
            let revcomp = intervals[0].revcomp();
            assert_eq!(forward.occ(sa.clone()), [5, 16]);
            assert_eq!(revcomp.occ(sa.clone()), [3, 14]);
        }
        {
            let pattern = b"CTTAA";
            let intervals = fmdindex.smems(pattern, 1);
            assert_eq!(intervals[0].forward().occ(sa.clone()), [2]);
            assert_eq!(intervals[0].revcomp().occ(sa.clone()), [14]);
            assert_eq!(intervals[0].match_size, 5)
        }
    }


    #[test]
    fn test_init_interval() {
        let text = b"ACGT$TGCA$";

        let alphabet = dna::n_alphabet();
        let sa = Rc::new(suffix_array(text));
        let bwt = Rc::new(bwt(text, sa.clone()));
        let less = Rc::new(less(&bwt, &alphabet));
        let occ = Rc::new(Occ::new(&bwt, 3, &alphabet));

        let fmindex = FMIndex::new(bwt, less, occ);
        let fmdindex = fmindex.as_fmdindex();
        let pattern = b"T";
        let interval = fmdindex.init_interval(pattern, 0);


        assert_eq!(interval.forward().occ(sa.clone()), [3, 5]);
        assert_eq!(interval.revcomp().occ(sa.clone()), [8, 0]);
    }

    #[test]
    #[cfg(feature = "nightly")]
    fn test_serde() {
        use serde::{Serialize, Deserialize};
        fn impls_serde_traits<S: Serialize + Deserialize>() {}

        impls_serde_traits::<FMIndex>();
        impls_serde_traits::<FMDIndex>();
    }

    #[test]
    fn test_issue39() {
        let reads = b"GGCGTGGTGGCTTATGCCTGTAATCCCAGCACTTTGGGAGGTCGAAGTGGGCGG$CCGC\
                       CCACTTCGACCTCCCAAAGTGCTGGGATTACAGGCATAAGCCACCACGCC$CGAAGTGG\
                       GCGGATCACTTGAGGTCAGGAGTTGGAGACTAGCCTGGCCAACACGATGAAACCCCGTC\
                       TCTAATA$TATTAGAGACGGGGTTTCATCGTGTTGGCCAGGCTAGTCTCCAACTCCTGA\
                       CCTCAAGTGATCCGCCCACTTCG$AGCTCGAAAAATGTTTGCTTATTTTGGTAAAATTA\
                       TTCATTGACTATGCTCAGAAATCAAGCAAACTGTCCATATTTCATTTTTTG$CAAAAAA\
                       TGAAATATGGACAGTTTGCTTGATTTCTGAGCATAGTCAATGAATAATTTTACCAAAAT\
                       AAGCAAACATTTTTCGAGCT$AGCTCGAAAAATGTTTGCTTATTTTGGTAAAATTATTC\
                       ATTGACTATGCTCAGAAATCAAGCAAACTGTCCATATTTCATTTTTTGAAATTACATAT\
                       $ATATGTAATTTCAAAAAATGAAATATGGACAGTTTGCTTGATTTCTGAGCATAGTCAA\
                       TGAATAATTTTACCAAAATAAGCAAACATTTTTCGAGCT$TAAAATTTCCTCTGACAGT\
                       GTAAAAGAGATCTTCATACAAAAATCAGAATTTATATAGTCTCTTTCCAAAAGACCATA\
                       AAACCAATCAGTTAATAGTTGAT$ATCAACTATTAACTGATTGGTTTTATGGTCTTTTG\
                       GAAAGAGACTATATAAATTCTGATTTTTGTATGAAGATCTCTTTTACACTGTCAGAGGA\
                       AATTTTA$CACCTATCTACCCTGAATCTAAGTGCTAACAGGAAAGGATGCCAGATTGCA\
                       TGCCTGCTGATAAAGCCACAGTTTGGACTGTCACTCAATCACCATCGTTC$GAACGATG\
                       GTGATTGAGTGACAGTCCAAACTGTGGCTTTATCAGCAGGCATGCAATCTGGCATCCTT\
                       TCCTGTTAGCACTTAGATTCAGGGTAGATAGGTG$CATCGTTCCTCCTGTGACTCAGTA\
                       TAACAAGATTGGGAGAATACTCTACAGTTCCTGATTCCCCCACAG$CTGTGGGGGAATC\
                       AGGAACTGTAGAGTATTCTCCCAATCTTGTTATACTGAGTCACAGGAGGAACGATG$TG\
                       TAAATTCTGAGAAAAATTTGCAGGTCTTTCTTCAGGAGCATGTAATCTCTTGCTCTCTT\
                       TGTTATCTATCTATAGTACTGTAGGTTATCTGGAGTTGCT$AGCAACTCCAGATAACCT\
                       ACAGTACTATAGATAGATAACAAAGAGAGCAAGAGATTACATGCTCCTGAAGAAAGACC\
                       TGCAAATTTTTCTCAGAATTTACA$CACTTCTCCTTGTCTTTACAGACTGGTTTTGCAC\
                       TGGGAAATCCTTTCACCAGTCAGCCCAGTTAGAGATTCTG$CAGAATCTCTAACTGGGC\
                       TGACTGGTGAAAGGATTTCCCAGTGCAAAACCAGTCTGTAAAGACAAGGAGAAGTG$AA\
                       TGGAGGTATATAAATTATCTGGCAAAGTGACATATCCTGACACATTCTCCAGGATAGAT\
                       CAAATGTTAGGTCACAAAGAGAGTCTTAACAAAATT$AATTTTGTTAAGACTCTCTTTG\
                       TGACCTAACATTTGATCTATCCTGGAGAATGTGTCAGGATATGTCACTTTGCCAGATAA\
                       TTTATATACCTCCATT$TTAATTTTGTTAAGACTCTCTTTGTGACCTAACATTTGATCT\
                       ATCCTGGAGAATGTGTCAGGATATGTCACTTTGCCAGATAATTTATATACCTCCATTTT\
                       $AAAATGGAGGTATATAAATTATCTGGCAAAGTGACATATCCTGACACATTCTCCAGGA\
                       TAGATCAAATGTTAGGTCACAAAGAGAGTCTTAACAAAATTAA$TTCTTCTTTGACTCA\
                       TTGGTTGTTCAATAGTATGTTGTTTAATTTCCATATATTTGTAAATGTTTCCGTTTTCC\
                       TTCTACTATTGAATTTTTGCTTCATC$GATGAAGCAAAAATTCAATAGTAGAAGGAAAA\
                       CGGAAACATTTACAAATATATGGAAATTAAACAACATACTATTGAACAACCAATGAGTC\
                       AAAGAAGAA$AGGAAAACGGAAACATTTACAAATATATGGAAATTAAACAACATACTAT\
                       TGAACAACCAATGAGTCAAAGAAGAAATCAAAAAGAATATTAGAAAAC$GTTTTCTAAT\
                       ATTCTTTTTGATTTCTTCTTTGACTCATTGGTTGTTCAATAGTATGTTGTTTAATTTCC\
                       ATATATTTGTAAATGTTTCCGTTTTCCT$TTAGAAAACAAGCTGACAAAAAAATAAAAA\
                       AACACAACATAGCAAAACTTAGAAATGCAGCAAAGGCAGTACTAAAGAGGGAAATTTAT\
                       AGCAATAAATGC$GCATTTATTGCTATAAATTTCCCTCTTTAGTACTGCCTTTGCTGCA\
                       TTTCTAAGTTTTGCTATGTTGTGTTTTTTTATTTTTTTGTCAGCTTGTTTTCTAA$TTT\
                       ATTGCTATAAATTTCCCTCTTTAGTACTGCCTTTGCTGCATTTCTAAGTTTTGCTATGT\
                       TGTGTTTTTTTATTTTTTTGTCAGCTTGTTTTCTA$TAGAAAACAAGCTGACAAAAAAA\
                       TAAAAAAACACAACATAGCAAAACTTAGAAATGCAGCAAAGGCAGTACTAAAGAGGGAA\
                       ATTTATAGCAATAAA$TCTTTCTTCTTTTTTAAGGTAGGCATTTATTGCTATAAATTTC\
                       CCTCTTTAGTACTGCCTTTG$CAAAGGCAGTACTAAAGAGGGAAATTTATAGCAATAAA\
                       TGCCTACCTTAAAAAAGAAGAAAGA$";

        let alphabet = dna::n_alphabet();
        let sa = Rc::new(suffix_array(reads));
        let bwt = Rc::new(bwt(reads, sa.clone()));
        let less = Rc::new(less(&bwt, &alphabet));
        let occ = Rc::new(Occ::new(&bwt, 3, &alphabet));

        let fmindex = FMIndex::new(bwt, less, occ);
        let fmdindex = fmindex.as_fmdindex();

        let read = b"GGCGTGGTGGCTTATGCCTGTAATCCCAGCACTTTGGGAGGTCGAAGTGGGCGG";
        let read_pos = 0;

        for i in 0..read.len() {
            println!("i {}", i);
            let intervals = fmdindex.smems(read, i);
            println!("{:?}", intervals);
            let matches = intervals.iter()
                                   .flat_map(|interval| interval.forward().occ(sa.clone()))
                                   .collect::<Vec<usize>>();
            //assert_eq!(matches, vec![read_pos]);
        }
    }
}
