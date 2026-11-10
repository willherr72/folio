//! Bounded, reference-free opaque painting settings for text removal.
use crate::{
    redaction_content::{invalid, number, require},
    EngineResult,
};
use lopdf::{Dictionary, Document, Object, ObjectId};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

const KEYS: &[&[u8]] = &[
    b"Type", b"OP", b"op", b"OPM", b"SA", b"SM", b"CA", b"ca", b"BM", b"SMask",
];
const DOCUMENT_STATES: usize = 256;
const PAGE_STATES: usize = 64;

fn resolve<'a>(pdf: &'a Document, mut value: &'a Object) -> EngineResult<&'a Object> {
    let mut visited = HashSet::new();
    while let Object::Reference(id) = value {
        require(visited.len() < 64 && visited.insert(*id))?;
        value = pdf.objects.get(id).ok_or_else(invalid)?;
    }
    Ok(value)
}
pub(super) struct State {
    dictionary: Dictionary,
    pub digest: [u8; 32],
}
fn normalize(pdf: &Document, value: &Object) -> EngineResult<State> {
    let source = resolve(pdf, value)?.as_dict().map_err(|_| invalid())?;
    require(
        source.len() <= KEYS.len() && source.iter().all(|(key, _)| KEYS.contains(&key.as_slice())),
    )?;
    let mut dictionary = Dictionary::new();
    let mut hash = Sha256::new();
    hash.update(b"Folio opaque graphics state v1");
    for key in KEYS {
        let Ok(value) = source.get(key) else { continue };
        let value = resolve(pdf, value)?;
        require(match *key {
            b"Type" => value.as_name().ok() == Some(b"ExtGState"),
            b"OP" | b"op" | b"SA" => matches!(value, Object::Boolean(_)),
            b"OPM" => matches!(value, Object::Integer(0..=1)),
            b"SM" => (0.0..=1.0).contains(&number(value)?),
            b"CA" | b"ca" => number(value)? == 1.0,
            b"BM" => value.as_name().ok() == Some(b"Normal"),
            b"SMask" => value.as_name().ok() == Some(b"None"),
            _ => false,
        })?;
        hash.update([key.len() as u8]);
        hash.update(key);
        match value {
            Object::Name(name) => {
                hash.update([b'n', name.len() as u8]);
                hash.update(name);
            }
            Object::Boolean(value) => hash.update([b'b', u8::from(*value)]),
            Object::Integer(value) => {
                hash.update(b"i");
                hash.update(value.to_be_bytes());
            }
            Object::Real(value) => {
                hash.update(b"r");
                hash.update(value.to_bits().to_be_bytes());
            }
            _ => return Err(invalid()),
        }
        dictionary.set(key.to_vec(), value.clone());
    }
    Ok(State {
        dictionary,
        digest: hash.finalize().into(),
    })
}
#[derive(Default)]
pub(super) struct Registry {
    states: HashMap<[u8; 32], Arc<State>>,
}
impl Registry {
    fn load(&mut self, pdf: &Document, value: &Object) -> EngineResult<Arc<State>> {
        let state = normalize(pdf, value)?;
        if let Some(existing) = self.states.get(&state.digest) {
            return Ok(existing.clone());
        }
        require(self.states.len() < DOCUMENT_STATES)?;
        let state = Arc::new(state);
        self.states.insert(state.digest, state.clone());
        Ok(state)
    }
}
pub(super) struct PageStates<'a, 'b> {
    pdf: &'a Document,
    resources: Option<&'a Dictionary>,
    registry: &'b mut Registry,
    pub states: Vec<Arc<State>>,
}
impl<'a, 'b> PageStates<'a, 'b> {
    pub fn new(
        pdf: &'a Document,
        resources: Option<&'a Dictionary>,
        registry: &'b mut Registry,
    ) -> Self {
        Self {
            pdf,
            resources,
            registry,
            states: vec![],
        }
    }
    pub fn select(&mut self, alias: &[u8]) -> EngineResult<usize> {
        let states = resolve(
            self.pdf,
            self.resources
                .ok_or_else(invalid)?
                .get(b"ExtGState")
                .map_err(|_| invalid())?,
        )?
        .as_dict()
        .map_err(|_| invalid())?;
        let state = self
            .registry
            .load(self.pdf, states.get(alias).map_err(|_| invalid())?)?;
        if let Some(index) = self.states.iter().position(|s| s.digest == state.digest) {
            return Ok(index);
        }
        require(self.states.len() < PAGE_STATES)?;
        self.states.push(state);
        Ok(self.states.len() - 1)
    }
}
#[derive(Default)]
pub(super) struct Publisher {
    states: HashMap<[u8; 32], ObjectId>,
}
impl Publisher {
    pub fn publish(&mut self, state: &State, pdf: &mut Document) -> ObjectId {
        *self
            .states
            .entry(state.digest)
            .or_insert_with(|| pdf.add_object(state.dictionary.clone()))
    }
}
