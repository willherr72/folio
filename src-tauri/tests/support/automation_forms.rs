use lopdf::{dictionary, Dictionary, Document, Object, Stream};
pub fn fixture() -> Vec<u8> {
    let mut d = Document::with_version("1.7");
    let pages = d.new_object_id();
    let font =
        d.add_object(dictionary! {"Type"=>"Font", "Subtype"=>"Type1", "BaseFont"=>"Helvetica"});
    let ap = d.add_object(Stream::new(dictionary! {"Type"=>"XObject", "Subtype"=>"Form", "BBox"=>vec![0.into(),0.into(),180.into(),30.into()], "Resources"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}}, b"0 0 180 30 re S BT /F1 12 Tf 5 10 Td (Original name) Tj ET".to_vec()));
    let off = d.add_object(Stream::new(dictionary! {"Type"=>"XObject", "Subtype"=>"Form", "BBox"=>vec![0.into(),0.into(),18.into(),18.into()]}, b"0 0 18 18 re S".to_vec()));
    let on = d.add_object(Stream::new(dictionary! {"Type"=>"XObject", "Subtype"=>"Form", "BBox"=>vec![0.into(),0.into(),18.into(),18.into()]}, b"0 0 18 18 re S 2 2 m 16 16 l S".to_vec()));
    let person = d.new_object_id();
    let name = d.new_object_id();
    let mut page_ids = vec![];
    let mut widgets = vec![];
    let mut fields = vec![Object::Reference(person)];
    for index in 0..3 {
        let page = d.new_object_id();
        let mut annotations = vec![];
        if index < 2 {
            let widget = d.add_object(dictionary! {"Type"=>"Annot", "Subtype"=>"Widget", "Parent"=>name, "P"=>page, "Rect"=>vec![40.into(),200.into(),220.into(),230.into()], "F"=>4, "AP"=>dictionary!{"N"=>ap}});
            widgets.push(Object::Reference(widget));
            annotations.push(Object::Reference(widget));
        }
        if index == 0 {
            let widget = d.add_object(dictionary! {"Type"=>"Annot", "Subtype"=>"Widget", "FT"=>"Btn", "T"=>Object::string_literal("consent"), "V"=>"Accepted", "DV"=>"Off", "AS"=>"Accepted", "P"=>page, "Rect"=>vec![40.into(),150.into(),58.into(),168.into()], "F"=>4, "AP"=>dictionary!{"N"=>dictionary!{"Off"=>off,"Accepted"=>on}}});
            fields.push(Object::Reference(widget));
            annotations.push(Object::Reference(widget));
        }
        if index == 2 {
            let widget = d.add_object(dictionary! {"Type"=>"Annot", "Subtype"=>"Widget", "FT"=>"Ch", "T"=>Object::string_literal("choice"), "Opt"=>vec![Object::string_literal("One"),Object::string_literal("Two")], "V"=>Object::string_literal("Two"), "P"=>page, "Rect"=>vec![40.into(),200.into(),220.into(),230.into()], "F"=>4, "AP"=>dictionary!{"N"=>ap}});
            fields.push(Object::Reference(widget));
            annotations.push(Object::Reference(widget));
        }
        d.objects.insert(page, Object::Dictionary(dictionary! {"Type"=>"Page", "Parent"=>pages, "MediaBox"=>vec![0.into(),0.into(),300.into(),400.into()], "Resources"=>dictionary!{}, "Annots"=>annotations}));
        page_ids.push(Object::Reference(page));
    }
    d.objects.insert(name, Object::Dictionary(dictionary! {"T"=>Object::string_literal("name"), "FT"=>"Tx", "Parent"=>person, "V"=>Object::string_literal("Original name"), "DV"=>Object::string_literal("Default name"), "MaxLen"=>80, "Kids"=>widgets}));
    d.objects.insert(person, Object::Dictionary(dictionary! {"T"=>Object::string_literal("person"), "Kids"=>vec![Object::Reference(name)]}));
    d.objects.insert(
        pages,
        Object::Dictionary(dictionary! {"Type"=>"Pages", "Count"=>3, "Kids"=>page_ids}),
    );
    let form = d.add_object(dictionary! {"Fields"=>fields, "DA"=>Object::string_literal("/F1 12 Tf 0 g"), "DR"=>dictionary!{"Font"=>dictionary!{"F1"=>font}}, "CO"=>vec![Object::Reference(name)]});
    let root = d.add_object(dictionary! {"Type"=>"Catalog", "Pages"=>pages, "AcroForm"=>form});
    d.trailer.set("Root", root);
    let mut bytes = vec![];
    d.save_to(&mut bytes).unwrap();
    bytes
}

pub fn controls() -> Vec<u8> {
    let mut d = Document::load_mem(&fixture()).unwrap();
    let page = *d.get_pages().values().next().unwrap();
    let form = d
        .catalog()
        .unwrap()
        .get(b"AcroForm")
        .unwrap()
        .as_reference()
        .unwrap();
    let mut annots = d
        .get_dictionary(page)
        .unwrap()
        .get(b"Annots")
        .unwrap()
        .as_array()
        .unwrap()
        .clone();
    let mut fields = d
        .get_dictionary(form)
        .unwrap()
        .get(b"Fields")
        .unwrap()
        .as_array()
        .unwrap()
        .clone();
    for (name, flags) in [
        ("combo", 1 << 17),
        ("list", 1 << 21),
        ("free", (1 << 17) | (1 << 18)),
        ("multi", 1 << 12),
    ] {
        let field=d.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","FT"=>if name=="multi"{"Tx"}else{"Ch"},"Ff"=>flags,"T"=>Object::string_literal(name),"P"=>page,"F"=>4,"Rect"=>vec![80.into(),40.into(),260.into(),100.into()],"Opt"=>vec![Object::Array(vec![Object::string_literal("a"),Object::string_literal("Alpha")]),Object::Array(vec![Object::string_literal("b"),Object::string_literal("Beta")])],"V"=>Object::string_literal(if name=="multi"{"start"}else{"a"}),"I"=>vec![0.into()]});
        annots.push(field.into());
        fields.push(field.into());
    }
    let parent = d.new_object_id();
    let mut kids = vec![];
    for (name, x) in [("Left", 10), ("Right", 40)] {
        let on=d.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),18.into(),18.into()]},b"0 0 18 18 re f".to_vec()));
        let off=d.add_object(Stream::new(dictionary!{"Type"=>"XObject","Subtype"=>"Form","BBox"=>vec![0.into(),0.into(),18.into(),18.into()]},vec![]));
        let mut states = Dictionary::new();
        states.set("Off", off);
        states.set(name, on);
        let child=d.add_object(dictionary!{"Type"=>"Annot","Subtype"=>"Widget","Parent"=>parent,"P"=>page,"F"=>4,"Rect"=>vec![x.into(),10.into(),(x+18).into(),28.into()],"AS"=>if name=="Left"{"Left"}else{"Off"},"AP"=>dictionary!{"N"=>states}});
        kids.push(Object::Reference(child));
        annots.push(child.into());
    }
    d.objects.insert(parent,Object::Dictionary(dictionary!{"FT"=>"Btn","Ff"=>1<<15,"T"=>Object::string_literal("side"),"V"=>"Left","Kids"=>kids}));
    fields.push(parent.into());
    d.get_dictionary_mut(form).unwrap().set("Fields", fields);
    d.get_dictionary_mut(page).unwrap().set("Annots", annots);
    let mut bytes = vec![];
    d.save_to(&mut bytes).unwrap();
    bytes
}
