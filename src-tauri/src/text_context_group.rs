//! Supported page transparency context, compared by meaning across private copies.
use super::*;
use lopdf::{Document, Object, ObjectId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Group {
    space: Vec<u8>,
    isolated: bool,
    knockout: bool,
}
fn bad() -> EngineError {
    EngineError::InvalidRequest("The page transparency group cannot be preserved safely.".into())
}
fn resolve<'a>(pdf: &'a Document, mut value: &'a Object) -> EngineResult<&'a Object> {
    let mut seen = HashSet::new();
    while let Object::Reference(id) = value {
        if seen.len() >= 64 || !seen.insert(*id) {
            return Err(bad());
        }
        value = pdf.objects.get(id).ok_or_else(bad)?;
    }
    Ok(value)
}
pub(super) fn descriptor(pdf: &Document, page: ObjectId) -> EngineResult<Option<Group>> {
    let page = pdf.get_dictionary(page).map_err(|_| bad())?;
    let Ok(value) = page.get(b"Group") else {
        return Ok(None);
    };
    let group = resolve(pdf, value)?.as_dict().map_err(|_| bad())?;
    if group
        .iter()
        .any(|(key, _)| !matches!(key.as_slice(), b"Type" | b"S" | b"CS" | b"I" | b"K"))
    {
        return Err(bad());
    }
    let name = |key: &[u8]| -> EngineResult<&[u8]> {
        resolve(pdf, group.get(key).map_err(|_| bad())?)?
            .as_name()
            .map_err(|_| bad())
    };
    if group.has(b"Type") && name(b"Type")? != b"Group" {
        return Err(bad());
    }
    if name(b"S")? != b"Transparency" {
        return Err(bad());
    }
    let space = name(b"CS")?;
    if !matches!(space, b"DeviceRGB" | b"DeviceGray" | b"DeviceCMYK") {
        return Err(bad());
    }
    let boolean = |key: &[u8]| -> EngineResult<bool> {
        match group.get(key) {
            Ok(value) => resolve(pdf, value)?.as_bool().map_err(|_| bad()),
            Err(_) => Ok(false),
        }
    };
    Ok(Some(Group {
        space: space.to_vec(),
        isolated: boolean(b"I")?,
        knockout: boolean(b"K")?,
    }))
}

pub(super) fn read(bytes: &[u8], page_index: usize) -> EngineResult<Option<Group>> {
    let pdf = crate::request_parse::read(bytes,crate::request_parse::Policy::Compatible16)
    .map_err(|_| bad())?;
    let page = pdf
        .get_pages()
        .values()
        .nth(page_index)
        .copied()
        .ok_or_else(bad)?;
    descriptor(&pdf, page)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary, Object};
    #[test]
    fn group_defaults_and_indirect_values_compare_semantically() {
        let mut pdf = Document::new();
        let plain = pdf.add_object(dictionary! {"Type"=>"Page"});
        assert_eq!(descriptor(&pdf, plain).unwrap(), None);
        let group = pdf.add_object(dictionary! {"S"=>"Transparency","CS"=>"DeviceRGB"});
        let a = pdf.add_object(dictionary! {"Type"=>"Page","Group"=>group});
        let cs = pdf.add_object(Object::Name(b"DeviceRGB".to_vec()));
        let isolation = pdf.add_object(Object::Boolean(false));
        let b=pdf.add_object(dictionary! {"Type"=>"Page","Group"=>dictionary! {"Type"=>"Group","S"=>"Transparency","CS"=>cs,"I"=>isolation,"K"=>false}});
        assert_eq!(descriptor(&pdf, a).unwrap(), descriptor(&pdf, b).unwrap());
        assert_ne!(
            descriptor(&pdf, a).unwrap(),
            descriptor(&pdf, plain).unwrap()
        );
        pdf.get_dictionary_mut(group).unwrap().set("I", true);
        assert_ne!(descriptor(&pdf, a).unwrap(), descriptor(&pdf, b).unwrap());
    }
    #[test]
    fn unsupported_and_cyclic_group_descriptors_refuse() {
        for group in [
            dictionary! {"S"=>"Transparency"},
            dictionary! {"S"=>"Transparency","CS"=>"NamedSpace"},
            dictionary! {"S"=>"Transparency","CS"=>"DeviceRGB","I"=>1},
            dictionary! {"S"=>"Transparency","CS"=>"DeviceRGB","K"=>Object::Null},
            dictionary! {"S"=>"Transparency","CS"=>"DeviceRGB","Unknown"=>true},
            dictionary! {"S"=>"Other","CS"=>"DeviceRGB"},
        ] {
            let mut pdf = Document::new();
            let page = pdf.add_object(dictionary! {"Group"=>group});
            assert!(descriptor(&pdf, page).is_err());
        }
        let mut pdf = Document::new();
        let cyclic = pdf.new_object_id();
        pdf.objects.insert(cyclic, Object::Reference(cyclic));
        let page = pdf.add_object(dictionary! {"Group"=>cyclic});
        assert!(descriptor(&pdf, page).is_err());
    }
}
