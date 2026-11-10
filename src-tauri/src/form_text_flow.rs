//! Admit bounded text objects with a known initial line position.
use super::{err, invalid, matrix, numbers, text_show, text_state::TextState};
use crate::EngineResult;
use lopdf::{content::Operation, Object};

pub(super) struct SelectedFont {
    pub resource: Object,
    pub size: Object,
}

#[derive(Default)]
pub(super) struct TextFlow {
    in_text: bool,
    positioned: bool,
}
impl TextFlow {
    pub fn in_text(&self) -> bool {
        self.in_text
    }
    pub fn finish(&self) -> EngineResult<()> {
        if self.in_text {
            Err(invalid("Unbalanced form text objects."))
        } else {
            Ok(())
        }
    }
    pub fn position(&mut self, op: &Operation, state: &mut TextState) -> EngineResult<bool> {
        match op.operator.as_str() {
            "BT" => {
                if self.in_text || !op.operands.is_empty() {
                    return Err(invalid("Malformed or nested form text objects."));
                }
                self.in_text = true;
                self.positioned = false;
            }
            "ET" => {
                if !self.in_text || !op.operands.is_empty() {
                    return Err(invalid("Malformed or unbalanced form text objects."));
                }
                self.in_text = false;
                self.positioned = false;
            }
            "Tm" | "Td" | "TD" | "T*" => {
                if !self.in_text {
                    return Err(invalid("Form text positioning requires a text object."));
                }
                match op.operator.as_str() {
                    "Tm" => {
                        matrix(&op.operands)?;
                    }
                    "Td" | "TD" => {
                        let values = numbers(&op.operands, 2)?;
                        if op.operator == "TD" {
                            state.leading = -values[1];
                        }
                    }
                    _ if !op.operands.is_empty() => return Err(err("T* operands")),
                    _ => {}
                }
                self.positioned = true;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
    pub fn show(&mut self, op: &Operation, state: &mut TextState) -> EngineResult<Option<Vec<u8>>> {
        if !self.in_text {
            return Err(invalid("Form text-show operators require a text object."));
        }
        let encoded = match op.operator.as_str() {
            "Tj" | "TJ" => text_show::encoded_bytes(op)?,
            "'" | "\"" => {
                let value = match (op.operator.as_str(), op.operands.as_slice()) {
                    ("'", [value]) => value,
                    ("\"", [word, character, value]) => {
                        state.word_spacing = numbers(std::slice::from_ref(word), 1)?[0];
                        state.character_spacing = numbers(std::slice::from_ref(character), 1)?[0];
                        value
                    }
                    _ => return Err(err("quote operands")),
                };
                let bytes = text_show::encoded_bytes(&Operation::new("Tj", vec![value.clone()]))?;
                self.positioned = true;
                bytes
            }
            _ => return Err(err("text show")),
        };
        if super::positioned_text::cursor_only(op) {
            return Ok(None);
        }
        if encoded.is_empty() {
            return Err(invalid("Empty form text-show strings are unsupported."));
        }
        if !self.positioned {
            return Err(invalid("Form text requires a known initial line position."));
        }
        // Consecutive shows retain a known cursor. Publication separately
        // preserves the selected show's end cursor when another piece uses it.
        Ok(Some(encoded))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn op(name: &str, operands: Vec<Object>) -> Operation {
        Operation::new(name, operands)
    }
    fn text() -> Operation {
        op("Tj", vec![Object::string_literal("Old")])
    }
    #[test]
    fn independent_line_flow_and_empty_objects() {
        let mut flow = TextFlow::default();
        let mut state = TextState::default();
        for name in ["BT", "ET", "BT"] {
            assert!(flow.position(&op(name, vec![]), &mut state).unwrap());
        }
        assert!(flow.show(&text(), &mut state).is_err());
        flow.position(&op("TD", vec![2.into(), (-14).into()]), &mut state)
            .unwrap();
        assert_eq!(state.leading, 14.);
        assert_eq!(
            flow.show(&text(), &mut state).unwrap(),
            Some(b"Old".to_vec())
        );
        assert!(flow
            .show(&op("TJ", vec![Object::Array(vec![20.into()])]), &mut state)
            .unwrap()
            .is_none());
        assert_eq!(
            flow.show(&text(), &mut state).unwrap(),
            Some(b"Old".to_vec())
        );
        flow.position(&op("T*", vec![]), &mut state).unwrap();
        flow.show(&text(), &mut state).unwrap();
        flow.position(&op("ET", vec![]), &mut state).unwrap();
        flow.finish().unwrap();
        assert_eq!(state.leading, 14.);
    }
    #[test]
    fn quotes_reset_position_and_retain_spacing() {
        let mut flow = TextFlow::default();
        let mut state = TextState::default();
        flow.position(&op("BT", vec![]), &mut state).unwrap();
        flow.show(&op("'", vec![Object::string_literal("First")]), &mut state)
            .unwrap();
        flow.show(
            &op(
                "\"",
                vec![2.into(), 0.5.into(), Object::string_literal("Next")],
            ),
            &mut state,
        )
        .unwrap();
        assert_eq!(state.word_spacing, 2.);
        assert_eq!(state.character_spacing, 0.5);
        assert_eq!(
            flow.show(&text(), &mut state).unwrap(),
            Some(b"Old".to_vec())
        );
    }
    #[test]
    fn malformed_and_outside_text_flow_is_refused() {
        for name in ["Tm", "Td", "TD", "T*"] {
            assert!(TextFlow::default()
                .position(&op(name, vec![]), &mut TextState::default())
                .is_err());
        }
        for bad in [
            op("BT", vec![1.into()]),
            op("ET", vec![1.into()]),
            op("T*", vec![0.into()]),
            op("TD", vec![0.into()]),
            op("Tm", vec![0.into()]),
        ] {
            let mut flow = TextFlow::default();
            let mut state = TextState::default();
            flow.position(&op("BT", vec![]), &mut state).unwrap();
            assert!(flow.position(&bad, &mut state).is_err());
        }
        for bad in [
            text(),
            op("'", vec![]),
            op(
                "\"",
                vec![
                    0.into(),
                    Object::Real(f32::INFINITY),
                    Object::string_literal("Old"),
                ],
            ),
        ] {
            let mut flow = TextFlow::default();
            let mut state = TextState::default();
            assert!(flow.show(&bad, &mut state).is_err());
            flow.position(&op("BT", vec![]), &mut state).unwrap();
            assert!(flow.show(&bad, &mut state).is_err());
            assert!(flow.finish().is_err());
        }
    }
}
