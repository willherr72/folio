//! Admission for raw imported normals. Font/image data are not content programs.
use super::{bad, preflight_cost_mode, MAX_BYTES};
use crate::EngineResult;
use lopdf::{Dictionary, Document, Object};
use std::collections::HashSet;

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
enum Role {
    Graph,
    Program,
    States,
}
pub(crate) struct NormalAppearanceAdmission<'a> {
    doc: &'a Document,
    seen: HashSet<(usize, Role)>,
    programs: HashSet<usize>,
    remaining: usize,
    visits: usize,
    failed: bool,
}
impl<'a> NormalAppearanceAdmission<'a> {
    pub(crate) fn new(doc: &'a Document) -> Self {
        Self {
            doc,
            seen: HashSet::new(),
            programs: HashSet::new(),
            remaining: 64 * 1024 * 1024,
            visits: 0,
            failed: false,
        }
    }
    pub(crate) fn admit(&mut self, value: &Object) -> EngineResult<()> {
        if self.failed {
            return Err(bad("an earlier normal appearance failed admission"));
        }
        let result = self.walk(value, Role::States, 0);
        self.failed = result.is_err();
        result
    }
    fn charge(&mut self, bytes: usize) -> EngineResult<()> {
        self.remaining = self
            .remaining
            .checked_sub(bytes)
            .ok_or_else(|| bad("normal appearance admission exceeds 64 MiB"))?;
        Ok(())
    }
    fn walk(&mut self, value: &Object, role: Role, depth: usize) -> EngineResult<()> {
        self.visits += 1;
        if depth > 64 || self.visits > 100_000 {
            return Err(bad("normal appearance graph exceeds its traversal bound"));
        }
        if let Object::Reference(id) = value {
            let value = self
                .doc
                .objects
                .get(id)
                .ok_or_else(|| bad("missing normal appearance dependency"))?;
            return self.walk(value, role, depth + 1);
        }
        let key = value as *const Object as usize;
        if !self.seen.insert((key, role)) {
            return Ok(());
        }
        self.charge(128)?;
        match value {
            Object::Stream(stream) => {
                // A stream declaring a pattern is a content program even if
                // its type uses a permissive/malformed numeric representation.
                // Do not let metadata spelling bypass expansion limits.
                let pattern = stream.dict.has(b"PatternType");
                let program = role != Role::Graph
                    || self.name(&stream.dict, b"Subtype")? == Some(b"Form")
                    || pattern;
                if program && self.programs.insert(key) {
                    let bytes = stream
                        .get_plain_content_with_limit(MAX_BYTES.min(self.remaining))
                        .map_err(|_| bad("normal appearance exceeds its decoding limit"))?;
                    self.charge(preflight_cost_mode(&bytes, true)?)?;
                }
                self.dictionary(&stream.dict, depth + 1)
            }
            Object::Dictionary(dict) if role == Role::States => {
                for (name, value) in dict.iter() {
                    self.charge(name.len())?;
                    self.walk(value, Role::Program, depth + 1)?;
                }
                Ok(())
            }
            Object::Dictionary(dict) if role == Role::Graph => self.dictionary(dict, depth + 1),
            Object::Array(array) if role == Role::Graph => {
                for value in array {
                    self.walk(value, Role::Graph, depth + 1)?;
                }
                Ok(())
            }
            _ if role != Role::Graph => Err(bad("normal appearance state is not a stream")),
            _ => Ok(()),
        }
    }
    fn resolved<'b>(&'b self, mut value: &'b Object) -> EngineResult<&'b Object> {
        let mut seen = HashSet::new();
        while let Object::Reference(id) = value {
            if seen.len() >= 64 || !seen.insert(*id) {
                return Err(bad("cyclic appearance metadata"));
            }
            value = self
                .doc
                .objects
                .get(id)
                .ok_or_else(|| bad("missing appearance metadata"))?;
        }
        Ok(value)
    }
    fn name<'b>(&'b self, dict: &'b Dictionary, key: &[u8]) -> EngineResult<Option<&'b [u8]>> {
        match dict.get(key).ok().map(|v| self.resolved(v)).transpose()? {
            None => Ok(None),
            Some(Object::Name(name)) => Ok(Some(name)),
            Some(_) => Err(bad("malformed appearance subtype")),
        }
    }
    fn dictionary(&mut self, dict: &Dictionary, depth: usize) -> EngineResult<()> {
        let type3 = self.name(dict, b"Subtype")? == Some(b"Type3");
        let mask = matches!(
            dict.get(b"S")
                .ok()
                .map(|v| self.resolved(v))
                .transpose()?
                .and_then(|v| v.as_name().ok()),
            Some(b"Alpha" | b"Luminosity")
        );
        for (name, value) in dict.iter() {
            self.charge(name.len())?;
            let role = if type3 && name == b"CharProcs" {
                Role::States
            } else if mask && name == b"G" {
                Role::Program
            } else {
                Role::Graph
            };
            self.walk(value, role, depth + 1)?;
        }
        Ok(())
    }
}
