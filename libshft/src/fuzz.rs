extern crate rand;

use std::borrow::Cow;
use self::rand::{Rand, Rng};
use parse::{Node, NodeRef, ParsedFile, RangeRef};

#[derive(Debug)]
pub struct FuzzFile<'buf: 'parse, 'parse> {
    root: Cow<'parse, [NodeRef]>,
    nodes: Cow<'parse, [Node<'buf>]>,
    ranges: Cow<'parse, [Vec<NodeRef>]>,
}

#[derive(Clone)]
pub enum FuzzAction {
    DuplicateRange,
    DuplicateRootNode,
    ShuffleRanges,
    SwapRanges,
}

impl Rand for FuzzAction {
    fn rand<R: Rng>(rng: &mut R) -> Self {
        let actions = vec![
            FuzzAction::DuplicateRange,
            FuzzAction::DuplicateRootNode,
            FuzzAction::ShuffleRanges,
            FuzzAction::SwapRanges,
        ];
        rng.choose(&actions).unwrap().clone()
    }
}

fn rand_indices<R: Rng, T>(mut rng: &mut Rng, x: &[T]) -> (usize, usize) {
    let indices = rand::sample(&mut rng, 0..x.len(), 2);
    (indices[0], indices[1])
}

struct SerializeState {
    have_serialized_range: Vec<bool>,
}

impl SerializeState {
    fn new(ranges: &[Vec<NodeRef>]) -> Self {
        SerializeState {
            have_serialized_range: ranges.iter().map(|_| false).collect()
        }
    }

    fn should_serialize(self: &mut Self, rangeref: RangeRef) -> bool {
        if self.have_serialized_range[rangeref] {
            false
        } else {
            self.have_serialized_range[rangeref] = true;
            true
        }
    }

    fn reset(self: &mut Self, rangeref: RangeRef) {
        self.have_serialized_range[rangeref] = false;
    }
}

impl<'buf, 'parse> FuzzFile<'buf, 'parse> {
    pub fn new(parsed: &'parse ParsedFile<'buf>) -> Self {
        FuzzFile {
            root: Cow::from(parsed.root.as_slice()),
            nodes: Cow::from(parsed.nodes.as_slice()),
            ranges: Cow::from(parsed.ranges.as_slice()),
        }
    }

    fn serialize_noderef(self: &Self, noderef: NodeRef, mut state: &mut SerializeState, mut out: &mut Vec<u8>) {
        match self.nodes[noderef] {
            Node::Delim(prefix, rangeref, postfix) => {
                out.extend(prefix);
                if state.should_serialize(rangeref) {
                    for noderef in &self.ranges[rangeref] {
                        self.serialize_noderef(*noderef, &mut state, &mut out)
                    }
                    state.reset(rangeref);
                }
                out.extend(postfix);
            },
            Node::Range(rangeref) => {
                if state.should_serialize(rangeref) {
                    for noderef in &self.ranges[rangeref] {
                        self.serialize_noderef(*noderef, &mut state, &mut out)
                    }
                    state.reset(rangeref);
                }
            },
            Node::Token(token) => out.extend(token),
        }
    }

    pub fn serialize(self: &Self) -> Vec<u8> {
        let mut out = Vec::new();
        let mut state = SerializeState::new(&self.ranges[..]);

        for noderef in self.root.iter() {
            self.serialize_noderef(*noderef, &mut state, &mut out);
        }

        out
    }

    pub fn swap_ranges<R: Rng>(self: &mut Self, rng: &mut R) {
        if self.ranges.len() > 1 {
            let mut ranges = self.ranges.to_mut();
            let (index0, index1) = rand_indices::<R, _>(rng, ranges);
            ranges.swap(index0, index1);
        }
    }

    pub fn shuffle_range<R: Rng>(self: &mut Self, rng: &mut R) {
        if let Some(range) = rng.choose_mut(self.ranges.to_mut()) {
            rng.shuffle(range)
        }
    }

    pub fn duplicate_range<R: Rng>(self: &mut Self, rng: &mut R) {
        if let Some(range) = rng.choose_mut(self.ranges.to_mut()) {
            let num_duplications = rng.gen_range(1, 4);
            let mut extension = Vec::new();
            for _ in 0..num_duplications {
                extension.extend(&range[..])
            }
            range.extend(&extension[..])
        }
    }

    pub fn duplicate_root_node<R: Rng>(self: &mut Self, rng: &mut R) {
        if self.nodes.len() > 1 {
            let mut nodes = self.nodes.to_mut();
            let (src_index, dst_index) = rand_indices::<R, _>(rng, nodes);

            let dup_node = nodes[dst_index].clone();
            let noderef = nodes.len();
            nodes.push(dup_node);

            let ranges = self.ranges.to_mut();
            let range = vec![noderef, src_index];
            let rangeref = ranges.len();
            ranges.push(range);

            nodes[dst_index] = Node::Range(rangeref);
        }
    }
}

pub fn fuzz_one<'buf, R: Rng>(parsed: &ParsedFile<'buf>, mut rng: &mut R, mutations: usize) -> Vec<u8> {
    let mut ff = FuzzFile::new(parsed);
    for _ in 0..mutations {
        match rng.gen() {
            FuzzAction::DuplicateRange => ff.duplicate_range(&mut rng),
            FuzzAction::DuplicateRootNode => ff.duplicate_root_node(&mut rng),
            FuzzAction::ShuffleRanges => ff.shuffle_range(&mut rng),
            FuzzAction::SwapRanges => ff.swap_ranges(&mut rng),
        }
    }
    ff.serialize()
}
