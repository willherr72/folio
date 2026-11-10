use crate::form_appearance::Copier;
use lopdf::{dictionary, Dictionary, Document, Object, Stream};

#[test]
fn shared_large_font_is_copied_once_across_linked_appearances() {
    let mut source = Document::new();
    let font = source.add_object(Stream::new(
        dictionary! {"Length1"=>20*1024*1024},
        vec![42; 20 * 1024 * 1024],
    ));
    let mut output = Document::new();
    let mut copy = Copier::with_byte_limit(&source, &mut output, 32 * 1024 * 1024);
    let first = copy.copy(&font.into()).unwrap();
    for _ in 0..100 {
        assert_eq!(copy.copy(&font.into()).unwrap(), first);
    }
    drop(copy);
    assert_eq!(output.objects.len(), 1);
    assert_eq!(
        output
            .get_object(first.as_reference().unwrap())
            .unwrap()
            .as_stream()
            .unwrap()
            .content
            .len(),
        20 * 1024 * 1024
    );
}

#[test]
fn imported_and_local_appearances_share_one_retained_budget() {
    let mut source = Document::new();
    let font = source.add_object(Stream::new(Dictionary::new(), vec![42; 6000]));
    let mut output = Document::new();
    let mut copy = Copier::with_byte_limit(&source, &mut output, 10_000);
    copy.copy(&font.into()).unwrap();
    assert!(copy
        .retain_local(Object::Stream(Stream::new(
            Dictionary::new(),
            vec![1; 6000]
        )))
        .is_err());
    drop(copy);
    assert_eq!(output.objects.len(), 1, "failed local object was retained");
}

#[test]
fn failed_import_cannot_reuse_a_partial_cached_reference() {
    let mut source = Document::new();
    let page = source.add_object(dictionary! {"Type"=>"Page"});
    let mut output = Document::new();
    let mut copy = Copier::new(&source, &mut output);
    assert!(copy.copy(&page.into()).is_err());
    assert!(
        copy.copy(&page.into()).is_err(),
        "failed reference was silently reused"
    );
    assert!(copy.retain_local(Object::Integer(1)).is_err());
}

#[test]
fn local_resource_dictionary_is_charged_before_clone() {
    let source = Document::new();
    let mut output = Document::new();
    let mut copy = Copier::with_byte_limit(&source, &mut output, 1024);
    assert!(copy
        .reserve_dictionary(&dictionary! {"Untrusted"=>Object::string_literal(vec![1;2048])})
        .is_err());
    assert!(
        copy.copy(&Object::Integer(1)).is_err(),
        "failed budget was reset"
    );
}

#[test]
fn local_objects_and_imports_keep_distinct_object_ids() {
    let mut source = Document::new();
    let font = source.add_object(dictionary! {"Type"=>"Font","BaseFont"=>"Helvetica"});
    let mut output = Document::new();
    output.max_id = 500;
    let mut copy = Copier::with_byte_limit(&source, &mut output, 10000);
    let imported = copy.copy(&font.into()).unwrap().as_reference().unwrap();
    let local = copy
        .retain_local(Object::Stream(Stream::new(
            dictionary! {"Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>imported}}},
            b"BT /F1 12 Tf (Answer) Tj ET".to_vec(),
        )))
        .unwrap();
    assert!(imported.0 > 500 && local.0 > 500);
    assert_ne!(imported, local);
    drop(copy);
    assert_eq!(output.objects.len(), 2);
}
