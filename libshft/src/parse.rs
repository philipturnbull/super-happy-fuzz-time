use grammar::{Grammar, GrammarDef};
use std::fmt;

pub type NodeRef = usize;
pub type RangeRef = usize;

#[derive(Debug)]
enum Match<'buf> {
    Break(&'buf [u8], &'buf [u8]),
    Tokenizer(&'buf [u8], &'buf [u8], &'buf [u8]),
    DelimStart(&'buf [u8], &'buf [u8], Vec<u8>, &'buf [u8]),
    DelimEnd(&'buf [u8], &'buf [u8], &'buf [u8]),
}

#[derive(Clone)]
pub enum Node<'buf> {
    Delim(&'buf [u8], RangeRef, &'buf [u8]),
    Range(RangeRef),
    Token(&'buf [u8]),
}

fn fmt_token(f: &mut fmt::Write, token: &[u8]) -> fmt::Result {
    write!(f, "\"")?;
    for b in token {
        if *b > 0x1f && *b < 0x7f {
            write!(f, "{}", *b as char)?
        } else if *b == 0x09 {
            write!(f, "\\t")?
        } else if *b == 0x0a {
            write!(f, "\\n")?
        } else if *b == 0x0d {
            write!(f, "\\r")?
        } else {
            write!(f, "\\x{:02x}", *b)?
        }
    }
    write!(f, "\"")
}

impl<'buf> fmt::Debug for Node<'buf> {
    fn fmt(self: &Self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Node::Delim(prefix, rangeref, postfix) => {
                write!(f, "Delim(")?;
                fmt_token(f, prefix)?;
                write!(f, ", {}, ", rangeref)?;
                fmt_token(f, postfix)?;
                write!(f, ")")
            },
            Node::Range(rangeref) => {
                write!(f, "Range({})", rangeref)
            },
            Node::Token(token) => {
                write!(f, "Token(")?;
                fmt_token(f, token)?;
                write!(f, ")")
            },
        }
    }
}

pub struct ParsedFile<'buf> {
    pub root: Vec<NodeRef>,
    pub nodes: Vec<Node<'buf>>,
    pub ranges: Vec<Vec<NodeRef>>,
}

impl<'buf> ParsedFile<'buf> {
    fn dump_noderef(self: &Self, indent: usize, noderef: NodeRef, f: &mut fmt::Write) -> fmt::Result {
        match self.nodes[noderef] {
            Node::Delim(prefix, rangeref, postfix) => {
                write!(f, "{:indent$}", "", indent=indent)?;
                fmt_token(f, prefix)?;
                writeln!(f, " {{")?;
                for noderef in &self.ranges[rangeref] {
                    self.dump_noderef(indent + 4, *noderef, f)?
                }
                write!(f, "{:indent$}}} ", "", indent=indent)?;
                fmt_token(f, postfix)?;
                writeln!(f, "")
            },
            Node::Range(rangeref) => {
                writeln!(f, "{:indent$}{{", "", indent=indent)?;
                for noderef in &self.ranges[rangeref] {
                    self.dump_noderef(indent + 4, *noderef, f)?
                }
                writeln!(f, "{:indent$}}} ", "", indent=indent)
            },
            Node::Token(token) => {
                write!(f, "{:indent$}", "", indent=indent)?;
                fmt_token(f, token)?;
                writeln!(f, "")
            },
        }
    }

    pub fn dump(self: &Self, f: &mut fmt::Write) -> fmt::Result {
        for noderef in &self.root {
            self.dump_noderef(0, *noderef, f)?
        }
        Ok(())
    }
}

struct SlurpState<'buf> {
    start_pattern: &'buf [u8],
    end_pattern: Vec<u8>,
    range: Vec<NodeRef>,
}

impl<'buf> SlurpState<'buf> {
    fn new(start_pattern: &'buf [u8], end_pattern: Vec<u8>) -> Self {
        SlurpState {
            start_pattern: start_pattern,
            end_pattern: end_pattern,
            range: Vec::new(),
        }
    }

    fn add_node_ref(self: &mut Self, noderef: NodeRef) {
        self.range.push(noderef)
    }
}

struct TreeBuilder<'buf> {
    root: Vec<NodeRef>,
    nodes: Vec<Node<'buf>>,
    ranges: Vec<Vec<NodeRef>>,

    stack: Vec<SlurpState<'buf>>,
}

impl<'buf> TreeBuilder<'buf> {
    fn new() -> Self {
        TreeBuilder {
            root: Vec::new(),
            nodes: Vec::new(),
            ranges: Vec::new(),
            stack: Vec::new(),
        }
    }

    fn add_node_ref(self: &mut Self, noderef: NodeRef) {
        if self.stack.is_empty() {
            self.root.push(noderef)
        } else {
            let index = self.stack.len() - 1;
            self.stack[index].add_node_ref(noderef)
        }
    }

    fn push_node(self: &mut Self, node: Node<'buf>) -> NodeRef {
        let noderef = self.nodes.len();
        self.nodes.push(node);
        noderef
    }

    fn push_token(self: &mut Self, buf: &'buf [u8]) {
        if !buf.is_empty() {
            let noderef = self.push_node(Node::Token(buf));

            if !self.stack.is_empty() {
                let index = self.stack.len() - 1;
                self.stack[index].add_node_ref(noderef)
            } else {
                self.root.push(noderef)
            }
        }
    }

    fn push_range(self: &mut Self, range: Vec<NodeRef>) -> RangeRef {
        let index = self.ranges.len();
        self.ranges.push(range);
        index
    }

    fn start_recurse(self: &mut Self, start_pattern: &'buf [u8], end_pattern: Vec<u8>) {
        self.stack.push(SlurpState::new(start_pattern, end_pattern));
    }

    fn state_with_end_pattern(self: &mut Self, end_pattern: &'buf [u8]) -> Option<SlurpState<'buf>> {
        if self.stack.is_empty() {
            None
        } else {
            let index = self.stack.len() - 1;
            if self.stack[index].end_pattern == end_pattern {
                self.stack.pop()
            } else {
                None
            }
        }
    }

    fn end_recurse(self: &mut Self, end_pattern: &'buf [u8]) {
        match self.state_with_end_pattern(end_pattern) {
            Some(state) => {
                let rangeref = self.push_range(state.range);
                let noderef = self.push_node(Node::Delim(state.start_pattern, rangeref, end_pattern));
                self.add_node_ref(noderef)
            },
            None => {
                self.push_token(end_pattern)
            },
        }
    }

    fn finish(self: &mut Self) {
        while let Some(state) = self.stack.pop() {
            self.push_token(state.start_pattern);
            for noderef in &state.range {
                self.add_node_ref(*noderef)
            }
        }
    }
}

fn scan_next<'buf, 'cfg>(grammar: &'cfg Grammar, buf: &'buf [u8]) -> Match<'buf> {
    for (i, _) in buf.iter().enumerate() {
        for def in &grammar.defs {
            match *def {
                GrammarDef::Delim(ref start_pattern, ref end_pattern) => {
                    if buf[i..].starts_with(start_pattern) {
                        return Match::DelimStart(&buf[..i], &buf[i..i+start_pattern.len()], end_pattern.clone(), &buf[i+start_pattern.len()..])
                    } else if buf[i..].starts_with(end_pattern) {
                        return Match::DelimEnd(&buf[..i], &buf[i..i+end_pattern.len()], &buf[i+end_pattern.len()..])
                    }
                },
                GrammarDef::Tokenizer(ref pattern) => {
                    if buf[i..].starts_with(pattern) {
                        return Match::Tokenizer(&buf[..i], &buf[i..i+pattern.len()], &buf[i+pattern.len()..])
                    }
                },
                GrammarDef::Breaker(ref pattern) => {
                    if i != 0 && buf[i..].starts_with(pattern) {
                        return Match::Break(&buf[..i], &buf[i..])
                    }
                },
            }
        }
    }

    Match::Break(buf, &buf[buf.len()..])
}

pub fn slurp<'buf>(grammar: &Grammar, buf: &'buf [u8]) -> ParsedFile<'buf> {
    let mut builder = TreeBuilder::new();

    let mut remainder = buf;
    while !remainder.is_empty() {
        let token_match = scan_next(grammar, remainder);
        remainder = match token_match {
            Match::Tokenizer(prefix, token, remainder) => {
                builder.push_token(prefix);
                builder.push_token(token);
                remainder
            },
            Match::DelimStart(prefix, start_pattern, end_pattern, remainder) => {
                builder.push_token(prefix);
                builder.start_recurse(start_pattern, end_pattern);
                remainder
            },
            Match::DelimEnd(prefix, end_pattern, remainder) => {
                builder.push_token(prefix);
                builder.end_recurse(end_pattern);
                remainder
            },
            Match::Break(token, remainder) => {
                builder.push_token(token);
                remainder
            },
        }
    }

    builder.finish();

    ParsedFile {
        root: builder.root,
        nodes: builder.nodes,
        ranges: builder.ranges,
    }
}
