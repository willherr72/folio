//! Effective PDF text parameters; BT/ET do not reset these graphics-state values.
use super::{invalid, numbers};
use crate::EngineResult;
use lopdf::{content::Operation, Object};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct TextState {
    pub character_spacing: f32,
    pub word_spacing: f32,
    pub horizontal_scale: f32,
    pub rise: f32,
    pub leading: f32,
}

#[cfg(test)]
mod tests {
    use super::super::{save, Model};
    use super::*;
    use lopdf::{dictionary, Document, Stream};

    #[test]
    fn form_text_state_requires_bounded_exact_operands() {
        for name in ["Tc", "Tw", "Tz", "Ts", "TL"] {
            for operands in [
                vec![],
                vec![0.into(), 1.into()],
                vec![Object::Null],
                vec![Object::Real(f32::NAN)],
                vec![Object::Real(f32::INFINITY)],
                vec![Object::Real(-f32::INFINITY)],
                vec![1_000_001.into()],
                vec![(-1_000_001).into()],
            ] {
                assert!(
                    TextState::default()
                        .apply(&Operation::new(name, operands))
                        .is_err(),
                    "{name}"
                );
            }
        }
        for value in [0., 0.009, -0.009, 10_001., -10_001.] {
            assert!(TextState::default()
                .apply(&Operation::new("Tz", vec![Object::Real(value)]))
                .is_err());
        }
        for value in [0.01, -0.01, 10_000., -10_000.] {
            assert!(TextState::default()
                .apply(&Operation::new("Tz", vec![Object::Real(value)]))
                .unwrap());
        }
        for operands in [
            vec![],
            vec![0.into(), 0.into()],
            vec![Object::Real(0.)],
            vec![(-1).into()],
            vec![1.into()],
            vec![3.into()],
            vec![7.into()],
        ] {
            assert!(TextState::default()
                .apply(&Operation::new("Tr", operands))
                .is_err());
        }
        assert!(TextState::default()
            .apply(&Operation::new("Tr", vec![0.into()]))
            .unwrap());
        assert!(!TextState::default()
            .apply(&Operation::new("BT", vec![]))
            .unwrap());
    }

    #[test]
    fn form_text_state_scopes_persist_across_bt_and_restore_after_forms_and_q() {
        let mut pdf = Document::with_version("1.7");
        let pages = pdf.new_object_id();
        let font = pdf
            .add_object(dictionary! {"Type"=>"Font", "Subtype"=>"Type1", "BaseFont"=>"Helvetica"});
        let fonts = dictionary! {"Font"=>dictionary!{"F"=>font}};
        let text = "BT /F 12 Tf 1 0 0 1 20 20 Tm (Old) Tj ET";
        let leaf = pdf.add_object(Stream::new(dictionary! {"Type"=>"XObject", "Subtype"=>"Form", "BBox"=>vec![0.into(),0.into(),400.into(),400.into()], "Resources"=>fonts.clone()},
            format!("{text} 4 Tw {text} 9 Tw").into_bytes()));
        let wrapper = pdf.add_object(Stream::new(dictionary! {"Type"=>"XObject", "Subtype"=>"Form", "BBox"=>vec![0.into(),0.into(),400.into(),400.into()], "Resources"=>dictionary!{"XObject"=>dictionary!{"Leaf"=>leaf}}},
            b"q 2 Tw /Leaf Do Q /Leaf Do".to_vec()));
        let content = pdf.add_object(Stream::new(
            dictionary! {},
            format!("0.5 Tc 80 Tz 3 Ts 14 TL 1 Tw /Wrapper Do {text} q 6 Tw {text} Q {text}")
                .into_bytes(),
        ));
        let page = pdf.add_object(dictionary! {"Type"=>"Page", "Parent"=>pages, "MediaBox"=>vec![0.into(),0.into(),600.into(),600.into()], "Resources"=>dictionary!{"Font"=>dictionary!{"F"=>font}, "XObject"=>dictionary!{"Wrapper"=>wrapper}}, "Contents"=>content});
        pdf.objects.insert(
            pages,
            dictionary! {"Type"=>"Pages", "Kids"=>vec![page.into()], "Count"=>1}.into(),
        );
        let root = pdf.add_object(dictionary! {"Type"=>"Catalog", "Pages"=>pages});
        pdf.trailer.set("Root", root);
        let model = Model::read(&save(&mut pdf).unwrap()).unwrap();
        let expected = [2., 4., 1., 4., 1., 6., 1.];
        assert_eq!(model.occurrences.len(), expected.len());
        for (occurrence, word_spacing) in model.occurrences.iter().zip(expected) {
            assert_eq!(
                occurrence.text_state,
                TextState {
                    character_spacing: 0.5,
                    word_spacing,
                    horizontal_scale: 80.,
                    rise: 3.,
                    leading: 14.
                }
            );
        }
        let changed = Model::read(&model.replace(&model.occurrences[0].path,
            &Operation::new("Tj", vec![Object::string_literal("New")])).unwrap()).unwrap();
        assert_eq!(
            model
                .occurrences
                .iter()
                .map(|o| o.text_state)
                .collect::<Vec<_>>(),
            changed
                .occurrences
                .iter()
                .map(|o| o.text_state)
                .collect::<Vec<_>>()
        );
    }
}
impl Default for TextState {
    fn default() -> Self {
        Self {
            character_spacing: 0.,
            word_spacing: 0.,
            horizontal_scale: 100.,
            rise: 0.,
            leading: 0.,
        }
    }
}
impl TextState {
    pub fn apply(&mut self, operation: &Operation) -> EngineResult<bool> {
        if operation.operator == "Tr" {
            if !matches!(operation.operands.as_slice(), [Object::Integer(0)]) {
                return Err(invalid(
                    "Form text editing requires filled rendering mode (0 Tr).",
                ));
            }
            return Ok(true);
        }
        if !matches!(
            operation.operator.as_str(),
            "Tc" | "Tw" | "Tz" | "Ts" | "TL"
        ) {
            return Ok(false);
        }
        let value = numbers(&operation.operands, 1)?[0];
        match operation.operator.as_str() {
            "Tc" => self.character_spacing = value,
            "Tw" => self.word_spacing = value,
            "Tz" => {
                if !(0.01..=10_000.).contains(&value.abs()) {
                    return Err(invalid(
                        "Form text horizontal scale is singular or exceeds safe bounds.",
                    ));
                }
                self.horizontal_scale = value;
            }
            "Ts" => self.rise = value,
            "TL" => self.leading = value,
            _ => unreachable!(),
        }
        Ok(true)
    }
}
