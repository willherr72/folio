use folio_engine::{
    resolve_choice_edit, ChoiceDraftOption, ChoiceEdit, ChoiceEditSelection as Edit, ChoiceModel,
    ChoiceOption, ChoiceSelection as Selection,
};

fn option(export: &str, display: &str) -> ChoiceOption {
    ChoiceOption {
        export_value: export.into(),
        display: display.into(),
    }
}
fn source() -> ChoiceModel {
    ChoiceModel {
        options: vec![
            option("a", "Same"),
            option("b", "Same"),
            option("c", "Third"),
        ],
        selection: Selection::Rows(vec![1]),
        defaults: Some(vec!["a".into()]),
        combo: false,
        editable: false,
        multiple: false,
        sorted: false,
        top_index: 1,
    }
}
fn draft(s: &ChoiceModel) -> ChoiceEdit {
    ChoiceEdit {
        options: s
            .options
            .iter()
            .enumerate()
            .map(|(i, o)| ChoiceDraftOption {
                source_index: Some(i),
                export_value: o.export_value.clone(),
                display: o.display.clone(),
            })
            .collect(),
        selection: Edit::Keep,
        defaults: Edit::Keep,
        manual_order: false,
    }
}
#[test]
fn reorder_uses_original_rows_even_when_labels_and_exports_repeat() {
    let mut s = source();
    s.options[2].export_value = "b".into();
    s.selection = Selection::Rows(vec![2]);
    let mut d = draft(&s);
    d.options.swap(0, 2);
    let r = resolve_choice_edit(&s, &d).unwrap();
    assert_eq!(r.selection, Selection::Rows(vec![0]));
    assert_eq!(r.options[0].export_value, "b");
    assert_eq!(r.defaults, Some(vec!["a".into()]));
    assert!(!r.defaults_changed);
}
#[test]
fn selected_export_change_follows_the_row_not_an_equal_label() {
    let s = source();
    let mut d = draft(&s);
    d.options[1].export_value = "new-b".into();
    let r = resolve_choice_edit(&s, &d).unwrap();
    assert_eq!(r.selection, Selection::Rows(vec![1]));
    assert_eq!(r.options[1].export_value, "new-b");
    assert_eq!(s.options[1].export_value, "b");
}
#[test]
fn removing_selected_or_default_rows_requires_explicit_resolution() {
    let s = source();
    let mut d = draft(&s);
    d.options.remove(1);
    assert!(resolve_choice_edit(&s, &d).is_err());
    d.selection = Edit::Clear;
    assert_eq!(
        resolve_choice_edit(&s, &d).unwrap().selection,
        Selection::None
    );
    d.options.remove(0);
    assert!(resolve_choice_edit(&s, &d).is_err());
    d.defaults = Edit::Clear;
    let r = resolve_choice_edit(&s, &d).unwrap();
    assert_eq!(r.defaults, None);
    assert!(r.defaults_changed);
}
#[test]
fn duplicate_default_values_are_preserved_without_inventing_a_unique_row() {
    let mut s = source();
    s.options[2].export_value = "a".into();
    let mut d = draft(&s);
    d.options.remove(0);
    let r = resolve_choice_edit(&s, &d).unwrap();
    assert_eq!(r.defaults, s.defaults);
    assert!(!r.defaults_changed);
    d.options[1].export_value = "other".into();
    assert!(resolve_choice_edit(&s, &d).is_err());
    d.defaults = Edit::Rows(vec![1]);
    assert_eq!(
        resolve_choice_edit(&s, &d).unwrap().defaults,
        Some(vec!["other".into()])
    );
}
#[test]
fn a_new_row_reusing_a_deleted_default_value_does_not_silently_adopt_it() {
    let s = source();
    let mut d = draft(&s);
    d.options.remove(0);
    d.options.push(ChoiceDraftOption {
        source_index: None,
        export_value: "a".into(),
        display: "New meaning".into(),
    });
    assert!(resolve_choice_edit(&s, &d).is_err());
    d.defaults = Edit::Rows(vec![2]);
    assert!(resolve_choice_edit(&s, &d).is_ok());
}
#[test]
fn multiselection_is_canonical_and_defaults_use_export_values() {
    let mut s = source();
    s.multiple = true;
    let mut d = draft(&s);
    d.selection = Edit::Rows(vec![2, 0]);
    d.defaults = Edit::Rows(vec![2, 1]);
    let r = resolve_choice_edit(&s, &d).unwrap();
    assert_eq!(r.selection, Selection::Rows(vec![0, 2]));
    assert_eq!(r.defaults, Some(vec!["b".into(), "c".into()]));
    d.selection = Edit::Rows(vec![1, 1]);
    assert!(resolve_choice_edit(&s, &d).is_err());
    d.selection = Edit::Rows(vec![0, 3]);
    assert!(resolve_choice_edit(&s, &d).is_err());
    d.selection = Edit::Rows(vec![0, 1]);
    s.multiple = false;
    assert!(resolve_choice_edit(&s, &d).is_err());
}
#[test]
fn free_text_remains_distinct_from_a_matching_option() {
    let mut s = source();
    s.combo = true;
    s.editable = true;
    s.top_index = 0;
    s.selection = Selection::Text("Custom".into());
    s.defaults = Some(vec!["Default custom".into()]);
    assert_eq!(
        resolve_choice_edit(&s, &draft(&s)).unwrap().selection,
        s.selection
    );
    let mut d = draft(&s);
    d.selection = Edit::Text("b".into());
    assert!(resolve_choice_edit(&s, &d).is_err());
    d.selection = Edit::Keep;
    d.options.push(ChoiceDraftOption {
        source_index: None,
        export_value: "Custom".into(),
        display: "New".into(),
    });
    assert!(resolve_choice_edit(&s, &d).is_err());
    d.selection = Edit::Rows(vec![3]);
    assert!(resolve_choice_edit(&s, &d).is_ok());
    d.options.push(ChoiceDraftOption {
        source_index: None,
        export_value: "Default custom".into(),
        display: "Default row".into(),
    });
    assert!(resolve_choice_edit(&s, &d).is_err());
    d.defaults = Edit::Rows(vec![4]);
    assert!(resolve_choice_edit(&s, &d).is_ok());
}
#[test]
fn empty_options_and_explicit_clears_are_retained() {
    let s = source();
    let mut d = draft(&s);
    d.options.clear();
    d.selection = Edit::Clear;
    d.defaults = Edit::Clear;
    let r = resolve_choice_edit(&s, &d).unwrap();
    assert!(r.options.is_empty());
    assert_eq!(r.selection, Selection::None);
    assert_eq!(r.defaults, None);
    assert_eq!(r.top_index, 0);
}
#[test]
fn top_visible_row_follows_identity_then_next_surviving_neighbor() {
    let mut s = source();
    s.selection = Selection::None;
    s.defaults = None;
    let mut d = draft(&s);
    d.options.swap(0, 1);
    assert_eq!(resolve_choice_edit(&s, &d).unwrap().top_index, 0);
    d.options.remove(0);
    assert_eq!(resolve_choice_edit(&s, &d).unwrap().top_index, 1);
    d.options.remove(1);
    assert_eq!(resolve_choice_edit(&s, &d).unwrap().top_index, 0);
}
#[test]
fn sorted_sources_require_explicit_manual_order_for_option_changes() {
    let mut s = source();
    s.sorted = true;
    let mut d = draft(&s);
    assert!(resolve_choice_edit(&s, &d).unwrap().sorted);
    d.options[0].display = "Changed".into();
    assert!(resolve_choice_edit(&s, &d).is_err());
    d.manual_order = true;
    assert!(!resolve_choice_edit(&s, &d).unwrap().sorted);
}
#[test]
fn invalid_original_identities_and_source_shapes_fail_without_mutation() {
    let s = source();
    let mut d = draft(&s);
    d.options[0].source_index = Some(3);
    assert!(resolve_choice_edit(&s, &d).is_err());
    d.options[0].source_index = Some(1);
    assert!(resolve_choice_edit(&s, &d).is_err());
    let mut invalid = s.clone();
    invalid.editable = true;
    assert!(resolve_choice_edit(&invalid, &draft(&invalid)).is_err());
    invalid = s.clone();
    invalid.selection = Selection::Rows(vec![99]);
    assert!(resolve_choice_edit(&invalid, &draft(&invalid)).is_err());
    invalid = s.clone();
    invalid.defaults = Some(vec!["missing".into()]);
    assert!(resolve_choice_edit(&invalid, &draft(&invalid)).is_err());
    assert_eq!(s.options.len(), 3);
    assert_eq!(s.selection, Selection::Rows(vec![1]));
}
#[test]
fn count_and_utf8_payload_limits_apply_to_both_inputs() {
    let mut s = source();
    s.selection = Selection::None;
    s.defaults = None;
    s.top_index = 0;
    s.options = vec![option("", ""); 2000];
    let mut d = draft(&s);
    assert!(resolve_choice_edit(&s, &d).is_ok());
    d.options.push(ChoiceDraftOption {
        source_index: None,
        export_value: "".into(),
        display: "".into(),
    });
    assert!(resolve_choice_edit(&s, &d).is_err());
    s.options.push(option("", ""));
    assert!(resolve_choice_edit(&s, &draft(&s)).is_err());
    s = source();
    let mut d = draft(&s);
    d.options[0].display = "é".repeat(2049);
    assert!(resolve_choice_edit(&s, &d).is_err());
    d = draft(&s);
    d.options[0].display = "é".repeat(2048);
    assert!(resolve_choice_edit(&s, &d).is_ok());
    s.options = vec![option(&"x".repeat(4096), &"y".repeat(4096)); 17];
    s.selection = Selection::None;
    s.defaults = None;
    assert!(resolve_choice_edit(&s, &draft(&s)).is_err());
}
#[test]
fn transport_rejects_unknown_fields_and_actions() {
    let d = draft(&source());
    let mut value = serde_json::to_value(&d).unwrap();
    value["surprise"] = serde_json::json!(true);
    assert!(serde_json::from_value::<ChoiceEdit>(value).is_err());
    let mut value = serde_json::to_value(&d).unwrap();
    value["selection"] = serde_json::json!({"kind":"Guess"});
    assert!(serde_json::from_value::<ChoiceEdit>(value).is_err());
}
#[test]
fn combined_output_budget_is_checked_even_when_each_input_fits() {
    let mut s = source();
    s.options = vec![option(&"a".repeat(4096), "")];
    s.selection = Selection::None;
    s.multiple = true;
    s.top_index = 0;
    s.defaults = Some(vec!["a".repeat(4096); 16]);
    let mut d = draft(&s);
    for _ in 0..15 {
        d.options.push(ChoiceDraftOption {
            source_index: None,
            export_value: "".into(),
            display: "x".repeat(4096),
        });
    }
    assert!(resolve_choice_edit(&s, &d).is_ok()); // Exactly128KiB combined output.
    d.options.push(ChoiceDraftOption {
        source_index: None,
        export_value: "".into(),
        display: "x".into(),
    });
    assert!(resolve_choice_edit(&s, &d).is_err());
    assert_eq!(s.defaults.as_ref().unwrap().len(), 16);
}
#[test]
fn empty_payloads_do_not_bypass_value_or_index_count_limits() {
    let mut s = source();
    s.multiple = true;
    s.options[0].export_value = "".into();
    s.defaults = Some(vec![String::new(); 2001]);
    assert!(resolve_choice_edit(&s, &draft(&s)).is_err());
    s.defaults = None;
    s.selection = Selection::Rows(vec![0; 2001]);
    assert!(resolve_choice_edit(&s, &draft(&s)).is_err());
    s.selection = Selection::None;
    let mut d = draft(&s);
    d.defaults = Edit::Rows(vec![0; 2001]);
    assert!(resolve_choice_edit(&s, &d).is_err());
}
#[test]
fn explicit_defaults_use_value_semantics_without_collapsing_current_row_identity() {
    let mut s = source();
    s.multiple = true;
    s.options[1].export_value = "a".into();
    let mut d = draft(&s);
    d.selection = Edit::Rows(vec![1, 0]);
    d.defaults = Edit::Rows(vec![1, 0]);
    let r = resolve_choice_edit(&s, &d).unwrap();
    assert_eq!(r.selection, Selection::Rows(vec![0, 1]));
    assert_eq!(r.defaults, Some(vec!["a".into()]));
    assert!(r.defaults_changed);
    s.defaults = None;
    let r = resolve_choice_edit(&s, &draft(&s)).unwrap();
    assert_eq!(r.defaults, None);
    assert!(!r.defaults_changed);
}
#[test]
fn text_actions_and_unknown_nested_fields_cannot_bypass_admission() {
    let s = source();
    let mut d = draft(&s);
    d.selection = Edit::Text("Custom".into());
    assert!(resolve_choice_edit(&s, &d).is_err());
    d.selection = Edit::Keep;
    d.defaults = Edit::Text("Custom".into());
    assert!(resolve_choice_edit(&s, &d).is_err());
    let mut value = serde_json::to_value(draft(&s)).unwrap();
    value["options"][0]["objectId"] = serde_json::json!(42);
    assert!(serde_json::from_value::<ChoiceEdit>(value).is_err());
    let mut value = serde_json::to_value(draft(&s)).unwrap();
    value["selection"] = serde_json::json!({"kind":"keep","value":[0]});
    assert!(serde_json::from_value::<ChoiceEdit>(value).is_err());
}
