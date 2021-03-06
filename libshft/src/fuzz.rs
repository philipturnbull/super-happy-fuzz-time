extern crate rand;

use std::borrow::Cow;
use std::cmp;
use self::rand::Rng;
use grammar::Delim;
use parse::{Node, NodeRef, ParsedFile, RangeRef};

#[derive(Debug)]
pub struct FuzzFile<'buf: 'parse, 'parse> {
    root: Cow<'parse, [NodeRef]>,
    nodes: Cow<'parse, [Node<'buf>]>,
    ranges: Cow<'parse, [Vec<NodeRef>]>,
}

#[derive(Clone)]
pub enum Mutation {
    DuplicateRange,
    DuplicateRootNode,
    EmptyDelim,
    NestDelim,
    RandDelim,
    RemoveDelim,
    ShuffleRanges,
    SwapDelim,
    SwapRanges,
}

pub fn default_mutations() -> Vec<Mutation> {
    vec![
        Mutation::DuplicateRange,
        Mutation::DuplicateRootNode,
        Mutation::EmptyDelim,
        Mutation::NestDelim,
        Mutation::RandDelim,
        Mutation::RemoveDelim,
        Mutation::ShuffleRanges,
        Mutation::SwapDelim,
        Mutation::SwapRanges,
    ]
}

pub struct FuzzConfig<'buf> {
    pub max_mutations: usize,
    pub max_duplications: usize,
    pub valid_actions: Vec<Mutation>,
    pub all_delims: Vec<Delim<'buf>>,
}

fn rand_indices<R: Rng, T>(mut rng: &mut Rng, x: &[T]) -> Option<(usize, usize)> {
    if x.len() > 1 {
        let indices = rand::sample(&mut rng, 0..x.len(), 2);
        Some((indices[0], indices[1]))
    } else {
        None
    }
}

fn rand_delim<'buf, R: Rng>(mut rng: &mut R, nodes: &[Node<'buf>]) -> Option<(NodeRef, Delim<'buf>, RangeRef)> {
    let delims: Vec<_> = nodes.iter().enumerate().filter_map(|item| {
        match item {
            (index, &Node::Delim(ref delim, rangeref)) => {
                Some((index, delim.clone(), rangeref))
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

pub struct SliceSerializer<'buf> {
    slice: &'buf mut [u8],
    cur_offset: usize,
}

impl<'buf> SliceSerializer<'buf> {
    pub fn new(slice: &'buf mut [u8]) -> SliceSerializer {
        SliceSerializer {
            slice: slice,
            cur_offset: 0,
        }
    }

    pub fn bytes_written(self: &Self) -> usize {
        self.cur_offset
    }
}

impl<'buf> SerializeInto for SliceSerializer<'buf> {
    fn push(self: &mut Self, token: &[u8]) {
        let remaining = self.slice.len() - self.cur_offset;
        let num_bytes_to_write = cmp::min(remaining, token.len());
        if num_bytes_to_write > 0 {
            self.slice[self.cur_offset..self.cur_offset+num_bytes_to_write].copy_from_slice(&token[..num_bytes_to_write]);
            self.cur_offset += num_bytes_to_write
        }
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
            Node::Delim(ref delim, rangeref) => {
                out.push(delim.start_pattern);
                if state.should_serialize(rangeref) {
                    for noderef in &self.ranges[rangeref] {
                        self.serialize_noderef(*noderef, &mut state, out)
                    }
                    state.reset(rangeref);
                }
                out.push(delim.end_pattern);
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

    pub fn swap_ranges<R: Rng>(self: &mut Self, rng: &mut R) -> bool {
        match rand_indices::<R, _>(rng, &self.ranges[..]) {
            Some((index0, index1)) => {
                let mut ranges = self.ranges.to_mut();
                ranges.swap(index0, index1);
                true
            },
            None => false,
        }
    }

    pub fn shuffle_range<R: Rng>(self: &mut Self, rng: &mut R) -> bool {
        match rng.choose_mut(self.ranges.to_mut()) {
            Some(range) => {
                rng.shuffle(range);
                true
            },
            None => false,
        }
    }

    pub fn duplicate_range<R: Rng>(self: &mut Self, rng: &mut R, max_duplications: usize) -> bool {
        if max_duplications < 1 {
            return false
        }

        match rng.choose_mut(self.ranges.to_mut()) {
            Some(range) => {
                let num_duplications = rng.gen_range(1, max_duplications);
                let mut extension = Vec::new();
                for _ in 0..num_duplications {
                    extension.extend(&range[..])
                }
                range.extend(&extension[..]);
                true
            },
            None => false,
        }
    }

    pub fn duplicate_root_node<R: Rng>(self: &mut Self, rng: &mut R) -> bool {
        match rand_indices::<R, _>(rng, &self.ranges[..]) {
            Some((src_index, dst_index)) => {
                let mut nodes = self.nodes.to_mut();

                let dup_node = nodes[dst_index].clone();
                let noderef = nodes.len();
                nodes.push(dup_node);

                let ranges = self.ranges.to_mut();
                let range = vec![noderef, src_index];
                let rangeref = ranges.len();
                ranges.push(range);

                nodes[dst_index] = Node::Range(rangeref);
                true
            },
            None => false,
        }
    }

    pub fn remove_delim<R: Rng>(self: &mut Self, mut rng: &mut R) -> bool {
        match rand_delim(&mut rng, &self.nodes[..]) {
            Some((index, _, rangeref)) => {
                let mut nodes = self.nodes.to_mut();
                nodes[index] = Node::Range(rangeref);
                true
            },
            None => false,
        }
    }

    pub fn swap_delim<R: Rng>(self: &mut Self, mut rng: &mut R) -> bool {
        match rand_delim(&mut rng, &self.nodes[..]) {
            Some((index, delim, rangeref)) => {
                let mut nodes = self.nodes.to_mut();
                let delim = Delim::new(delim.end_pattern, delim.start_pattern);
                nodes[index] = Node::Delim(delim, rangeref);
                true
            },
            None => false,
        }
    }

    pub fn nest_delim<R: Rng>(self: &mut Self, mut rng: &mut R) -> bool {
        match rand_delim(&mut rng, &self.nodes[..]) {
            Some((index, ref delim, rangeref)) => {
                let mut nodes = self.nodes.to_mut();
                let mut ranges = self.ranges.to_mut();

                let nested_noderef = nodes.len();
                nodes.push(Node::Delim(delim.clone(), rangeref));

                let nested_rangeref = ranges.len();
                ranges.push(vec![nested_noderef]);

                nodes[index] = Node::Delim(delim.clone(), nested_rangeref);
                true
            },
            None => false,
        }
    }

    pub fn empty_delim<R: Rng>(self: &mut Self, mut rng: &mut R) -> bool {
        match rand_delim(&mut rng, &self.nodes[..]) {
            Some((index, delim, _)) => {
                let mut nodes = self.nodes.to_mut();
                let mut ranges = self.ranges.to_mut();

                let rangeref = ranges.len();
                ranges.push(vec![]);

                nodes[index] = Node::Delim(delim, rangeref);
                true
            },
            None => false,
        }
    }

    pub fn rand_delim<R: Rng>(self: &mut Self, mut rng: &mut R, delims: &[Delim<'buf>]) -> bool {
        if delims.is_empty() {
            return false
        }

        match rand_delim(&mut rng, &self.nodes[..]) {
            Some((index, ref delim, rangeref)) => {
                match rng.choose(&delims[..]) {
                    Some(&ref new_delim) => {
                        if new_delim != delim {
                            let mut nodes = self.nodes.to_mut();
                            nodes[index] = Node::Delim(new_delim.clone(), rangeref);
                            true
                        } else {
                            false
                        }
                    },
                    None => {
                        false
                    }
                }
            },
            None => false,
        }
    }
}

pub fn fuzz_one<'buf, 'parse, R: Rng>(parsed: &'parse ParsedFile<'buf>, mut rng: &mut R, config: &'buf FuzzConfig) -> Option<FuzzFile<'buf, 'parse>> {
    let mut ff = FuzzFile::new(parsed);
    let mut did_mutate = false;
    for _ in 0..config.max_mutations {
        did_mutate |= match rng.choose(&config.valid_actions[..]) {
            Some(&Mutation::DuplicateRange) => ff.duplicate_range(&mut rng, config.max_duplications),
            Some(&Mutation::DuplicateRootNode) => ff.duplicate_root_node(&mut rng),
            Some(&Mutation::EmptyDelim) => ff.empty_delim(&mut rng),
            Some(&Mutation::NestDelim) => ff.nest_delim(&mut rng),
            Some(&Mutation::RandDelim) => ff.rand_delim(&mut rng, &config.all_delims[..]),
            Some(&Mutation::RemoveDelim) => ff.remove_delim(&mut rng),
            Some(&Mutation::ShuffleRanges) => ff.shuffle_range(&mut rng),
            Some(&Mutation::SwapDelim) => ff.swap_delim(&mut rng),
            Some(&Mutation::SwapRanges) => ff.swap_ranges(&mut rng),
            None => false,
        }
    }

    if did_mutate {
        Some(ff)
    } else {
        None
    }
}
