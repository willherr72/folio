//! Preserve the next show cursor without changing the PDF text line matrix.
use super::*;

pub(super) fn following_depends(operations: &[Operation]) -> bool {
    for op in operations {
        match op.operator.as_str() {
            "BT" | "ET" | "Tm" | "Td" | "TD" | "T*" | "'" | "\"" => return false,
            "Tj" | "TJ" if glyph_show(op) => return true,
            _ => {}
        }
    }
    false
}

fn gaps(show: &Operation) -> EngineResult<f64> {
    let mut total = 0.;
    if let ("TJ", [Object::Array(values)]) = (show.operator.as_str(), show.operands.as_slice()) {
        for value in values {
            if value.as_str().is_err() {
                total += f64::from(numbers(std::slice::from_ref(value), 1)?[0]);
            }
        }
    }
    Ok(total)
}

fn width(run: &RunSnapshot, text: &str, substitute: Option<&FontAsset>) -> EngineResult<f64> {
    if let Some(asset) = substitute {
        return text
            .chars()
            .try_fold(0., |sum, ch| Ok(sum + f64::from(asset.advance(ch)?)));
    }
    if let Some(proof) = &run.proof {
        return Ok(f64::from(proof.advance(text)?));
    }
    if let Some(proof) = &run.standard {
        return Ok(f64::from(proof.advance(text)?));
    }
    let font: TextFont = serde_json::from_value(serde_json::Value::String(run.font.clone()))
        .map_err(|_| invalid("The form font has no verified advance metrics."))?;
    Ok(f64::from(persistence::text_advance(text, font)?))
}

pub(super) fn correction(
    run: &RunSnapshot,
    original: &Occurrence,
    changed: &Operation,
    replacement: &str,
    substitute: Option<&FontAsset>,
) -> EngineResult<f32> {
    let size = f64::from(numbers(std::slice::from_ref(&original.font.size), 1)?[0]);
    if size <= 0. {
        return Err(invalid(
            "The form font size cannot preserve a dependent cursor.",
        ));
    }
    let old_words = if run.proof.as_ref().is_none_or(|proof| proof.single_byte()) {
        original.encoded.iter().filter(|b| **b == 32).count()
    } else {
        0
    };
    let new_words =
        if substitute.is_none() && run.proof.as_ref().is_none_or(|proof| proof.single_byte()) {
            text_show::encoded_bytes(changed)?
                .iter()
                .filter(|b| **b == 32)
                .count()
        } else {
            0
        };
    let characters = replacement.chars().count() as f64 - run.text.chars().count() as f64;
    let spacing = characters * f64::from(original.text_state.character_spacing)
        + (new_words as f64 - old_words as f64) * f64::from(original.text_state.word_spacing);
    let adjustment = (width(run, replacement, substitute)? - width(run, &run.text, None)?
        + spacing / size)
        * 1000.
        - gaps(changed)?
        + gaps(&original.show)?;
    if !adjustment.is_finite() || adjustment.abs() > 1_000_000. {
        return Err(invalid(
            "The dependent text cursor adjustment exceeds safe limits.",
        ));
    }
    Ok(adjustment as f32)
}

/// Merge into the immediately following numeric-only TJ, so repeat edits do not
/// add a new operator each time. Other original adjustment slots remain intact.
pub(super) fn append(ops: &mut Vec<Operation>, index: usize, adjustment: f32) -> EngineResult<()> {
    if adjustment == 0. {
        return Ok(());
    }
    if let Some(next) = ops
        .get_mut(index + 1)
        .filter(|op| positioned_text::cursor_only(op))
    {
        let [Object::Array(values)] = next.operands.as_mut_slice() else {
            unreachable!()
        };
        if let Some(first) = values.first_mut() {
            let value = f64::from(first.as_float().map_err(err)?) + f64::from(adjustment);
            if !value.is_finite() || value.abs() > 1_000_000. {
                return Err(invalid(
                    "The retained text cursor adjustment exceeds safe limits.",
                ));
            }
            *first = Object::Real(value as f32);
        } else {
            values.push(Object::Real(adjustment));
        }
    } else {
        ops.insert(
            index + 1,
            Operation::new("TJ", vec![Object::Array(vec![Object::Real(adjustment)])]),
        );
    }
    Ok(())
}

/// Consecutive Tf operations have only their final selection as an effect. Drop
/// superseded aliases only when neither this scope nor an inheriting child uses
/// them; explicit child resource dictionaries resolve their own names.
pub(super) fn compact_font_selections(
    ops: &mut Vec<Operation>,
    resources: &mut Dictionary,
    node: &Node,
) -> EngineResult<()> {
    let mut compacted: Vec<Operation> = Vec::with_capacity(ops.len());
    let mut removed = HashSet::new();
    for op in ops.drain(..) {
        if op.operator == "Tf"
            && compacted
                .last()
                .is_some_and(|previous| previous.operator == "Tf")
        {
            let previous = compacted.pop().unwrap();
            removed.insert(previous.operands[0].as_name().map_err(err)?.to_vec());
        }
        compacted.push(op);
    }
    *ops = compacted;
    if removed.is_empty() {
        return Ok(());
    }
    fn names(operations: &[Operation], used: &mut HashSet<Vec<u8>>) -> EngineResult<()> {
        for op in operations.iter().filter(|op| op.operator == "Tf") {
            used.insert(op.operands[0].as_name().map_err(err)?.to_vec());
        }
        Ok(())
    }
    fn inherited_names(node: &Node, used: &mut HashSet<Vec<u8>>) -> EngineResult<()> {
        for (_, child) in &node.children {
            if child
                .stream
                .as_ref()
                .is_some_and(|stream| stream.dict.has(b"Resources"))
            {
                continue;
            }
            names(&child.operations, used)?;
            inherited_names(child, used)?;
        }
        Ok(())
    }
    let mut used = HashSet::new();
    names(ops, &mut used)?;
    inherited_names(node, &mut used)?;
    let fonts = resources
        .get_mut(b"Font")
        .and_then(Object::as_dict_mut)
        .map_err(err)?;
    for name in removed {
        if !used.contains(&name) {
            fonts.remove(&name);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parsed(value: &str) -> Vec<Operation> {
        Content::decode(value.as_bytes()).unwrap().operations
    }

    #[test]
    fn compacted_alias_stays_available_to_inheriting_child() {
        for explicit in [false, true] {
            let mut child_dict = Dictionary::new();
            if explicit {
                child_dict.set("Resources", Dictionary::new());
            }
            let child = Node {
                operations: parsed("/Old 16 Tf (Child) Tj"),
                resources: Dictionary::new(),
                stream: Some(Stream::new(child_dict, vec![])),
                children: vec![],
            };
            let node = Node {
                operations: vec![],
                resources: Dictionary::new(),
                stream: None,
                children: vec![(0, child)],
            };
            let mut ops = parsed("/Old 16 Tf /New 16 Tf (Parent) Tj /Old 12 Tf [20] TJ /New 12 Tf");
            let mut resources =
                dictionary! {"Font"=>dictionary!{"Old"=>Object::Null,"New"=>Object::Null}};
            compact_font_selections(&mut ops, &mut resources, &node).unwrap();
            // A later real source use retains Old even when a child resolves it
            // independently; numeric cursor moves separate distinct selections.
            assert_eq!(ops.len(), 5);
            assert!(resources
                .get(b"Font")
                .unwrap()
                .as_dict()
                .unwrap()
                .has(b"Old"));
            let mut ops = parsed("/Old 16 Tf /New 16 Tf (Parent) Tj");
            compact_font_selections(&mut ops, &mut resources, &node).unwrap();
            assert_eq!(ops.len(), 2);
            assert_eq!(
                resources
                    .get(b"Font")
                    .unwrap()
                    .as_dict()
                    .unwrap()
                    .has(b"Old"),
                !explicit
            );
        }
    }

    #[test]
    fn dependency_ends_at_line_position_but_not_spacing_or_font_changes() {
        for tail in ["/F2 12 Tf (Next) Tj", "[12 -3] TJ 0.2 Tc 90 Tz [(Next)] TJ"] {
            assert!(following_depends(&parsed(tail)), "{tail}");
        }
        for tail in [
            "T* (Next) Tj",
            "0 -20 Td (Next) Tj",
            "0 -20 TD (Next) Tj",
            "1 0 0 1 20 30 Tm (Next) Tj",
            "(Next) '",
            "1 2 (Next) \"",
            "ET BT (Next) Tj",
        ] {
            assert!(!following_depends(&parsed(tail)), "{tail}");
        }
    }

    #[test]
    fn repeated_adjustments_keep_slots_bounded_and_do_not_touch_text() {
        let original = parsed("(Old) Tj [-1800 12 -3] TJ (Next) Tj");
        let mut ops = original.clone();
        for _ in 0..100 {
            append(&mut ops, 0, -123.25).unwrap();
            append(&mut ops, 0, 123.25).unwrap();
        }
        assert_eq!(ops.len(), original.len());
        assert_eq!(
            (&ops[0].operator, &ops[0].operands),
            (&original[0].operator, &original[0].operands)
        );
        assert_eq!(
            (&ops[2].operator, &ops[2].operands),
            (&original[2].operator, &original[2].operands)
        );
        assert_eq!(gaps(&ops[1]).unwrap(), -1791.);
        let Object::Array(values) = &ops[1].operands[0] else {
            panic!()
        };
        assert_eq!(&values[1..], &[Object::Integer(12), Object::Integer(-3)]);
        assert!(append(&mut ops, 0, -1_000_000.).is_err());
        let mut no_slot = parsed("(Old) Tj (Next) Tj");
        append(&mut no_slot, 0, 0.).unwrap();
        assert_eq!(no_slot.len(), 2);
        append(&mut no_slot, 0, 20.).unwrap();
        append(&mut no_slot, 0, -20.).unwrap();
        assert_eq!(no_slot.len(), 3);
        assert_eq!(gaps(&no_slot[1]).unwrap(), 0.);
    }
}
