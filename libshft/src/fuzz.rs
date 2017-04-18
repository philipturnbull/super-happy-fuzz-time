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
    RemoveDelim,
    ShuffleRanges,
    SwapDelim,
    SwapRanges,
}

impl Rand for FuzzAction {
    fn rand<R: Rng>(rng: &mut R) -> Self {
        let actions = vec![
            FuzzAction::DuplicateRange,
            FuzzAction::DuplicateRootNode,
            FuzzAction::RemoveDelim,
            FuzzAction::ShuffleRanges,
            FuzzAction::SwapDelim,
            FuzzAction::SwapRanges,
        ];
        rng.choose(&actions).unwrap().clone()
    }
}

fn rand_indices<R: Rng, T>(mut rng: &mut Rng, x: &[T]) -> (usize, usize) {
    let indices = rand::sample(&mut rng, 0..x.len(), 2);
    (indices[0], indices[1])
}

fn rand_delim<'buf, R: Rng>(mut rng: &mut R, nodes: &[Node<'buf>]) -> Option<(usize, &'buf [u8], RangeRef, &'buf [u8])> {
    let delims: Vec<_> = nodes.iter().enumerate().filter_map(|item| {
        match item {
            (index, &Node::Delim(start_pattern, rangeref, end_pattern)) => {
                Some((index, start_pattern, rangeref, end_pattern))
            },
            _ => None,
        }
    }).collect();
    rng.choose(&delims[..]).cloned()
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

pub trait SerializeInto {
    fn push(&mut self, &[u8]);
}

impl SerializeInto for Vec<u8> {
    fn push(self: &mut Self, token: &[u8]) {
        self.extend(token);
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

    fn serialize_noderef<S: SerializeInto>(self: &Self, noderef: NodeRef, mut state: &mut SerializeState, mut out: &mut S) {
        match self.nodes[noderef] {
            Node::Delim(prefix, rangeref, postfix) => {
                out.push(prefix);
                if state.should_serialize(rangeref) {
                    for noderef in &self.ranges[rangeref] {
                        self.serialize_noderef(*noderef, &mut state, out)
                    }
                    state.reset(rangeref);
                }
                out.push(postfix);
            },
            Node::Range(rangeref) => {
                if state.should_serialize(rangeref) {
                    for noderef in &self.ranges[rangeref] {
                        self.serialize_noderef(*noderef, &mut state, out)
                    }
                    state.reset(rangeref);
                }
            },
            Node::Token(token) => out.push(token),
        }
    }

    pub fn serialize<S: SerializeInto>(self: &Self, out: &mut S) {
        let mut state = SerializeState::new(&self.ranges[..]);

        for noderef in self.root.iter() {
            self.serialize_noderef(*noderef, &mut state, out);
        }
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

    pub fn remove_delim<R: Rng>(self: &mut Self, mut rng: &mut R) {
        if let Some((index, _, rangeref, _)) = rand_delim(&mut rng, &self.nodes[..]) {
            let mut nodes = self.nodes.to_mut();
            nodes[index] = Node::Range(rangeref);
        }
    }

    pub fn swap_delim<R: Rng>(self: &mut Self, mut rng: &mut R) {
        if let Some((index, start_pattern, rangeref, end_pattern)) = rand_delim(&mut rng, &self.nodes[..]) {
            let mut nodes = self.nodes.to_mut();
            nodes[index] = Node::Delim(end_pattern, rangeref, start_pattern);
        }
    }
}

pub fn fuzz_one<'buf, 'parse, R: Rng>(parsed: &'parse ParsedFile<'buf>, mut rng: &mut R, mutations: usize) -> FuzzFile<'buf, 'parse> {
    let mut ff = FuzzFile::new(parsed);
    for _ in 0..mutations {
        match rng.gen() {
            FuzzAction::DuplicateRange => ff.duplicate_range(&mut rng),
            FuzzAction::DuplicateRootNode => ff.duplicate_root_node(&mut rng),
            FuzzAction::RemoveDelim => ff.remove_delim(&mut rng),
            FuzzAction::ShuffleRanges => ff.shuffle_range(&mut rng),
            FuzzAction::SwapDelim => ff.swap_delim(&mut rng),
            FuzzAction::SwapRanges => ff.swap_ranges(&mut rng),
        }
    }

    ff
}
