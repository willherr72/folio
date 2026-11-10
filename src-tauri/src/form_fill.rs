//! Native interactive form editing on a private immutable-source copy.
use crate::{EngineError, EngineResult};
use pdfium_render::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    ffi::c_void,
    os::raw::{c_int, c_ulong},
    ptr,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FormFieldValue {
    Text { text: String },
    Toggle { checked: bool },
    Choice { indices: Vec<usize> },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormField {
    pub annotation_index: usize,
    pub name: String,
    pub label: String,
    pub kind: String,
    pub value: FormFieldValue,
    pub options: Vec<String>,
    pub selected_indices: Vec<usize>,
    pub read_only: bool,
    pub required: bool,
    pub multiline: bool,
    pub password: bool,
    pub multi_select: bool,
    pub editable_choice: bool,
    pub reason: Option<String>,
    /// Raw PDF coordinates internally; the worker converts these to source display coordinates.
    pub bounds: Option<crate::AnnotationRect>,
    pub visible: bool,
    pub no_rotate: bool,
}
pub(crate) struct FieldEligibility {
    pub supported:bool,pub password:bool,pub visible:bool,
    pub bounds:Option<crate::AnnotationRect>,pub no_rotate:bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FillFormFieldRequest {
    pub page_index: usize,
    pub annotation_index: usize,
    pub expected_value: FormFieldValue,
    pub value: FormFieldValue,
}
/// Shared request semantics for raw generation and faithful publication.
pub(crate) struct ValidatedFill {
    pub(crate) field: FormField,
    pub(crate) wanted: FormFieldValue,
}
fn bad(message: impl std::fmt::Display) -> EngineError {
    EngineError::InvalidRequest(format!("PDF form filling: {message}"))
}
const MAX_TEXT: usize = 32768;
// Bounds the retained Rust inspection model, not PDFium or source parsing.
const INSPECTION_BUDGET: usize = 8 * 1024 * 1024;
fn charge(budget: &mut usize, bytes: usize) -> EngineResult<()> {
    *budget = budget.checked_sub(bytes).ok_or_else(|| bad("field inspection exceeds its memory budget"))?;
    Ok(())
}
fn reserve<T>(values: &mut Vec<T>, additional: usize, budget: &mut usize) -> EngineResult<()> {
    let needed = values.len().checked_add(additional).ok_or_else(|| bad("field inspection exceeds its memory budget"))?;
    if needed > values.capacity() {
        let capacity = needed.max(values.capacity().saturating_mul(2)).max(4);
        let bytes = capacity.checked_sub(values.capacity()).and_then(|n| n.checked_mul(std::mem::size_of::<T>())).ok_or_else(|| bad("field inspection exceeds its memory budget"))?;
        charge(budget, bytes)?;
        values.try_reserve_exact(capacity - values.len()).map_err(bad)?;
        // Account for allocator-provided capacity beyond the requested amount.
        charge(budget, values.capacity().saturating_sub(capacity).saturating_mul(std::mem::size_of::<T>()))?;
    }
    Ok(())
}
fn owned_string(value: &str, budget: &mut usize) -> EngineResult<String> {
    charge(budget, value.len() + std::mem::size_of::<String>())?;
    Ok(value.to_owned())
}
#[cfg(test)]
mod inspection_budget_tests {
    use super::*;
    use std::cell::Cell;
    fn read(units:&[u16],budget:&mut usize,payloads:&Cell<usize>)->EngineResult<String>{
        limited_string(|out,size|{
            if !out.is_null() {
                payloads.set(payloads.get()+1);
                assert_eq!(size as usize,units.len()*2);
                unsafe {std::ptr::copy_nonoverlapping(units.as_ptr(),out,units.len());}
            }
            (units.len()*2) as c_ulong
        },budget)
    }
    #[test]
    fn exhausted_budget_refuses_before_reading_or_allocating_string_payload(){
        let payloads=Cell::new(0);let mut budget=1;
        assert!(read(&[65,66,67,0],&mut budget,&payloads).is_err());
        assert_eq!(payloads.get(),0,"budget check must precede native payload allocation/read");
    }
    #[test]
    fn exact_unicode_budget_and_malformed_native_strings(){
        let text="Aé𝄞\r\nB\rC";let units=text.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
        let mut budget=text.len()+std::mem::size_of::<String>();let payloads=Cell::new(0);
        assert_eq!(read(&units,&mut budget,&payloads).unwrap(),text);assert_eq!(budget,0);assert_eq!(payloads.get(),1);
        assert!(read(&[0xd800,0],&mut 1024,&Cell::new(0)).is_err());
        assert!(read(&[65,66],&mut 1024,&Cell::new(0)).is_err());
        assert!(limited_string(|out,_|if out.is_null(){4}else{6},&mut 1024).is_err());
        let calls=Cell::new(0);assert!(limited_string(|_,_|{calls.set(calls.get()+1);3},&mut 1024).is_err());assert_eq!(calls.get(),1);
    }
    #[test]
    fn bounded_capacity_and_overflow_refuse_before_growth() {
        let mut values = Vec::<u64>::new();
        let mut budget = 31;
        assert!(reserve(&mut values, 1, &mut budget).is_err());
        assert_eq!(values.capacity(), 0);
        budget = 32;
        reserve(&mut values, 1, &mut budget).unwrap();
        assert_eq!(budget, 0);
        values.push(1);
        reserve(&mut values, 3, &mut budget).unwrap();
        let capacity = values.capacity();
        assert!(reserve(&mut values, 4, &mut budget).is_err());
        assert!(reserve(&mut values, usize::MAX, &mut budget).is_err());
        assert_eq!(values.capacity(), capacity);
    }
    #[test]
    fn individual_string_limit_and_owned_normalization() {
        let units = vec![b'x' as u16; MAX_TEXT].into_iter().chain(Some(0)).collect::<Vec<_>>();
        let mut budget = MAX_TEXT + std::mem::size_of::<String>();
        assert_eq!(read(&units, &mut budget, &Cell::new(0)).unwrap().len(), MAX_TEXT);
        let mut excessive = units.clone(); excessive.insert(0, 120);
        let payloads = Cell::new(0);
        assert!(read(&excessive, &mut INSPECTION_BUDGET.clone(), &payloads).is_err());
        assert_eq!(payloads.get(), 0);
        let original = String::from("é\r\n𝄞\rB\nC\r");
        let pointer = original.as_ptr(); let capacity = original.capacity();
        let normalized = normalize_owned(original);
        assert_eq!(normalized, "é\n𝄞\nB\nC\n");
        assert_eq!(normalized.as_ptr(), pointer);
        assert_eq!(normalized.capacity(), capacity);
    }
    #[test]
    fn normalized_native_values_charge_normalized_bytes_and_keep_the_utf16_limit() {
        for (text, accepted) in [("\r\n".repeat(MAX_TEXT),true),("é".repeat(MAX_TEXT+1),false)] {
            let units=text.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
            let mut budget=MAX_TEXT+std::mem::size_of::<String>();
            let value=limited_native_string(|out,_| {
                if !out.is_null() { unsafe { std::ptr::copy_nonoverlapping(units.as_ptr(),out,units.len()); } }
                (units.len()*2) as c_ulong
            }, &mut budget,true);
            if accepted {assert_eq!(value.unwrap(),"\n".repeat(MAX_TEXT));assert_eq!(budget,0);}else{assert!(value.is_err());}
        }
        let calls=Cell::new(0);
        let mut budget=usize::MAX;
        assert!(limited_native_string(|_,_|{calls.set(calls.get()+1);(MAX_TEXT*4+4) as c_ulong},&mut budget,true).is_err());
        assert_eq!(calls.get(),1,"oversized scratch must refuse before reading the payload");
    }
}
fn limited_string(get: impl FnMut(*mut u16, c_ulong) -> c_ulong, budget: &mut usize) -> EngineResult<String> {
    limited_native_string(get, budget, false)
}
fn limited_native_string(mut get: impl FnMut(*mut u16, c_ulong) -> c_ulong, budget: &mut usize, normalize_lines: bool) -> EngineResult<String> {
    let size = get(ptr::null_mut(), 0) as usize;
    if size == 0 {
        charge(budget, std::mem::size_of::<String>())?;
        return Ok(String::new());
    }
    // PDFium expands multiline LF to CRLF. Bound that scratch representation,
    // then enforce the public limit on the normalized value before retaining it.
    let raw_limit = if normalize_lines { MAX_TEXT * 2 } else { MAX_TEXT };
    if size % 2 != 0 || size > raw_limit * 2 + 2 {
        return Err(bad("field text exceeds the supported length"));
    }
    // Every valid UTF-16 unit needs at least one UTF-8 byte. Reject impossible
    // fits before allocating scratch space or asking PDFium for the payload.
    let minimum = if normalize_lines { (size / 2 - 1).div_ceil(2) } else { size / 2 - 1 };
    if minimum + std::mem::size_of::<String>() > *budget {
        return Err(bad("field inspection exceeds its memory budget"));
    }
    let mut data = vec![0u16; size / 2];
    if get(data.as_mut_ptr(), size as c_ulong) as usize != size || data.last() != Some(&0) {
        return Err(bad("could not read field text"));
    }
    data.pop();
    if normalize_lines {
        let (mut read, mut write) = (0, 0);
        while read < data.len() {
            let unit = data[read]; read += 1;
            data[write] = if unit == 13 {
                if data.get(read) == Some(&10) { read += 1; }
                10
            } else { unit };
            write += 1;
        }
        data.truncate(write);
        if data.len() > MAX_TEXT { return Err(bad("field text exceeds the supported length")); }
    }
    let mut bytes = 0usize;
    for character in char::decode_utf16(data.iter().copied()) {
        bytes = bytes.checked_add(character.map_err(bad)?.len_utf8()).ok_or_else(|| bad("field inspection exceeds its memory budget"))?;
    }
    charge(budget, bytes + std::mem::size_of::<String>())?;
    let mut value = String::with_capacity(bytes);
    for character in char::decode_utf16(data.iter().copied()) {
        value.push(character.map_err(bad)?);
    }
    Ok(value)
}
#[cfg(test)]
fn normalize_owned(value: String) -> String {
    let mut bytes = value.into_bytes();
    let (mut read, mut write) = (0, 0);
    while read < bytes.len() {
        let byte = bytes[read];
        read += 1;
        bytes[write] = if byte == b'\r' {
            if bytes.get(read) == Some(&b'\n') { read += 1; }
            b'\n'
        } else { byte };
        write += 1;
    }
    bytes.truncate(write);
    String::from_utf8(bytes).expect("replacing ASCII line endings preserves UTF-8")
}
pub(crate) fn normalized(value: &FormFieldValue) -> FormFieldValue {
    match value {
        FormFieldValue::Text { text } => FormFieldValue::Text {
            text: text.replace("\r\n", "\n").replace('\r', "\n"),
        },
        _ => value.clone(),
    }
}

/// All handles belong to a separate document; bytes and the heap-allocated info
/// outlive it. Callbacks are null except the bounded, non-reentrant save callback.
pub(crate) struct NativeForm<'a> {
    bindings: &'a dyn PdfiumLibraryBindings,
    bytes: Cow<'a, [u8]>,
    actions: crate::acroforms::ActionMask,
    info: Box<FPDF_FORMFILLINFO>,
    document: FPDF_DOCUMENT,
    form: FPDF_FORMHANDLE,
    pages: Vec<(usize, FPDF_PAGE)>,
}
struct Annotation<'a> {
    bindings: &'a dyn PdfiumLibraryBindings,
    handle: FPDF_ANNOTATION,
}
impl Drop for Annotation<'_> {
    fn drop(&mut self) {
        unsafe { self.bindings.FPDFPage_CloseAnnot(self.handle) }
    }
}
impl Drop for NativeForm<'_> {
    fn drop(&mut self) {
        unsafe {
            for (_, page) in self.pages.drain(..) {
                if !self.form.is_null() {
                    self.bindings.FORM_OnBeforeClosePage(page, self.form);
                }
                self.bindings.FPDF_ClosePage(page);
            }
            if !self.form.is_null() {
                self.bindings.FPDFDOC_ExitFormFillEnvironment(self.form);
            }
            if !self.document.is_null() {
                self.bindings.FPDF_CloseDocument(self.document);
            }
        }
    }
}
impl<'a> NativeForm<'a> {
    pub(crate) fn open(
        bindings: &'a dyn PdfiumLibraryBindings,
        bytes: &'a [u8],
    ) -> EngineResult<Self> {
        let (mut source, indices) = crate::acroforms::filling_document(bytes)?;
        let actions = crate::acroforms::ActionMask::apply(&mut source)?;
        let bytes = if actions.changed() {
            let mut sanitized = vec![];
            source.save_to(&mut sanitized).map_err(bad)?;
            Cow::Owned(sanitized)
        } else {
            Cow::Borrowed(bytes)
        };
        // C struct consists of nullable callbacks, pointers and integers. Keeping
        // callbacks unset disables scripts/external actions. Internal actions are
        // separately masked above, including ResetForm and Hide.
        let mut info: Box<FPDF_FORMFILLINFO> = Box::new(unsafe { std::mem::zeroed() });
        info.version = 2;
        info.xfa_disabled = 1;
        let mut this = Self {
            bindings,
            bytes,
            actions,
            info,
            document: ptr::null_mut(),
            form: ptr::null_mut(),
            pages: vec![],
        };
        unsafe {
            this.document = bindings.FPDF_LoadMemDocument64(&this.bytes, None);
            if this.document.is_null() {
                return Err(bad("could not open the source"));
            }
            this.form = bindings.FPDFDOC_InitFormFillEnvironment(this.document, this.info.as_mut());
            if this.form.is_null() {
                return Err(bad("could not initialize the form"));
            }
            for index in indices {
                let page =
                    bindings.FPDF_LoadPage(this.document, i32::try_from(index).map_err(bad)?);
                if page.is_null() {
                    return Err(bad("could not load a form page"));
                }
                this.pages.push((index, page));
                bindings.FORM_OnAfterLoadPage(page, this.form);
            }
        }
        Ok(this)
    }
    fn page(&self, index: usize) -> EngineResult<FPDF_PAGE> {
        self.pages
            .iter()
            .find(|(i, _)| *i == index)
            .map(|(_, p)| *p)
            .ok_or_else(|| bad("page has no interactive fields"))
    }
    fn annotation(&self, page: usize, index: usize) -> EngineResult<Annotation<'_>> {
        let page = self.page(page)?;
        let handle = unsafe {
            self.bindings
                .FPDFPage_GetAnnot(page, i32::try_from(index).map_err(bad)?)
        };
        if handle.is_null() {
            Err(bad("field no longer exists"))
        } else {
            Ok(Annotation {
                bindings: self.bindings,
                handle,
            })
        }
    }
    pub(crate) fn fields(&self, page: usize) -> EngineResult<Vec<FormField>> {
        self.fields_bounded(page,INSPECTION_BUDGET)
    }
    /// Inspect a known widget without reading unrelated names, values or options.
    pub(crate) fn field_bounded(&self,page:usize,index:usize,mut budget:usize)->EngineResult<FormField>{
        let annot=self.annotation(page,index)?;
        if unsafe {self.bindings.FPDFAnnot_GetSubtype(annot.handle)}!=20 {return Err(bad("annotation is not a form widget"));}
        budget = budget.min(INSPECTION_BUDGET);
        charge(&mut budget, std::mem::size_of::<FormField>())?;
        self.inspect_bounded(index,annot.handle,&mut budget)
    }
    pub(crate) fn fields_bounded(&self, page:usize,budget:usize)->EngineResult<Vec<FormField>> {
        self.fields_filtered(page,budget,|_|true)
    }
    pub(crate) fn fields_filtered(&self,page:usize,mut budget:usize,mut include:impl FnMut(&FieldEligibility)->bool)->EngineResult<Vec<FormField>> {
        budget = budget.min(INSPECTION_BUDGET);
        let Ok(handle) = self.page(page) else {
            if page >= unsafe { self.bindings.FPDF_GetPageCount(self.document) } as usize {
                return Err(bad("page does not exist"));
            }
            return Ok(vec![]);
        };
        let count = unsafe { self.bindings.FPDFPage_GetAnnotCount(handle) };
        if !(0..=100_000).contains(&count) {
            return Err(bad("page annotation count exceeds the supported limit"));
        }
        let mut fields = vec![];
        for i in 0..count as usize {
            let annot = self.annotation(page, i)?;
            if unsafe { self.bindings.FPDFAnnot_GetSubtype(annot.handle) } == 20 {
                // Inspect fixed-size eligibility before reading any names, values,
                // or option strings. Excluded fields consume no value budget.
                let eligibility=unsafe {
                    let flags=self.bindings.FPDFAnnot_GetFormFieldFlags(self.form,annot.handle);
                    let kind=self.bindings.FPDFAnnot_GetFormFieldType(self.form,annot.handle);
                    let annotation_flags=self.bindings.FPDFAnnot_GetFlags(annot.handle);
                    let mut rect=FS_RECTF{left:0.,top:0.,right:0.,bottom:0.};
                    let bounds=if self.bindings.FPDFAnnot_GetRect(annot.handle,&mut rect)!=0
                        &&[rect.left,rect.top,rect.right,rect.bottom].iter().all(|v|v.is_finite())&&rect.right>rect.left&&rect.top>rect.bottom {
                        Some(crate::AnnotationRect{x:rect.left,y:rect.bottom,width:rect.right-rect.left,height:rect.top-rect.bottom})
                    }else{None};
                    FieldEligibility{supported:(2..=6).contains(&kind),password:kind==6&&flags&(1<<13)!=0,visible:annotation_flags&(1|2|32)==0,bounds,no_rotate:annotation_flags&16!=0}
                };
                if !include(&eligibility){continue;}
                reserve(&mut fields, 1, &mut budget)?;
                fields.push(self.inspect_bounded(i, annot.handle,&mut budget)?);
            }
        }
        Ok(fields)
    }
    fn inspect(&self, index: usize, annot: FPDF_ANNOTATION) -> EngineResult<FormField> {
        let mut budget = INSPECTION_BUDGET;
        charge(&mut budget, std::mem::size_of::<FormField>())?;
        self.inspect_bounded(index,annot,&mut budget)
    }
    fn inspect_bounded(&self,index:usize,annot:FPDF_ANNOTATION,budget:&mut usize)->EngineResult<FormField> {
        unsafe {
            let b = self.bindings;
            let form = self.form;
            let native = b.FPDFAnnot_GetFormFieldType(form, annot);
            let flags = b.FPDFAnnot_GetFormFieldFlags(form, annot);
            let name = limited_string(|buf, n| b.FPDFAnnot_GetFormFieldName(form, annot, buf, n),budget)?;
            let mut label =
                limited_string(|buf, n| b.FPDFAnnot_GetFormFieldAlternateName(form, annot, buf, n),budget)?;
            if label.is_empty() {
                label = owned_string(&name, budget)?;
            }
            let kind = owned_string(match native {
                2 => "checkbox",
                3 => "radio",
                4 => "combo",
                5 => "list",
                6 => "text",
                7 => "signature",
                _ => "unsupported",
            }, budget)?;
            let editable_choice = native == 4 && flags & (1 << 18) != 0;
            let mut options = vec![];
            let mut selected = vec![];
            if native == 4 || native == 5 {
                let count = b.FPDFAnnot_GetOptionCount(form, annot);
                if !(0..=2000).contains(&count) {
                    return Err(bad("field has an invalid or excessive option count"));
                }
                reserve(&mut options, count as usize, budget)?;
                for i in 0..count {
                    options.push(limited_string(|buf, n| {
                        b.FPDFAnnot_GetOptionLabel(form, annot, i, buf, n)
                    },budget)?);
                    if b.FPDFAnnot_IsOptionSelected(form, annot, i) != 0 {
                        reserve(&mut selected, 1, budget)?;
                        selected.push(i as usize);
                    }
                }
            }
            if native == 2 || native == 3 {
                let export =
                    limited_string(|buf, n| b.FPDFAnnot_GetFormFieldExportValue(form, annot, buf, n),budget)?;
                if !export.is_empty() {
                    reserve(&mut options, 1, budget)?;
                    options.push(export);
                }
            }
            let value = match native {
                2 | 3 => FormFieldValue::Toggle {
                    checked: b.FPDFAnnot_IsChecked(form, annot) != 0,
                },
                4 | 5 if !editable_choice => {
                    charge(budget, selected.len().checked_mul(std::mem::size_of::<usize>()).ok_or_else(|| bad("field inspection exceeds its memory budget"))?)?;
                    FormFieldValue::Choice {
                    indices: selected.clone(),
                }},
                _ => FormFieldValue::Text {
                    text: limited_native_string(|buf, n| b.FPDFAnnot_GetFormFieldValue(form, annot, buf, n),budget,true)?,
                },
            };
            let read_only = flags & 1 != 0;
            let annotation_flags = b.FPDFAnnot_GetFlags(annot);
            let mut rect=FS_RECTF {left:0.,top:0.,right:0.,bottom:0.};
            let bounds=if b.FPDFAnnot_GetRect(annot,&mut rect)!=0
                && [rect.left,rect.top,rect.right,rect.bottom].iter().all(|n|n.is_finite())
                && rect.right>rect.left && rect.top>rect.bottom {
                Some(crate::AnnotationRect {x:rect.left,y:rect.bottom,width:rect.right-rect.left,height:rect.top-rect.bottom})
            }else{None};
            let reason = if read_only {
                Some("This field is read-only.")
            } else if annotation_flags & (1 | 2 | 32 | 64) != 0 {
                Some("This field is hidden or cannot be edited on the page.")
            } else if !(2..=6).contains(&native) {
                Some("This field type cannot be filled here.")
            } else if native == 6 && flags & ((1 << 20) | (1 << 25)) != 0 {
                Some("File-selection and rich-text fields are not supported yet.")
            } else {
                None
            }
            .map(|value| owned_string(value, budget)).transpose()?;
            Ok(FormField {
                annotation_index: index,
                name,
                label,
                kind,
                value,
                options,
                selected_indices: selected,
                read_only,
                required: flags & 2 != 0,
                multiline: native == 6 && flags & (1 << 12) != 0,
                password: native == 6 && flags & (1 << 13) != 0,
                multi_select: native == 5 && flags & (1 << 21) != 0,
                editable_choice,
                reason,
                bounds,
                visible: annotation_flags & (1|2|32)==0,
                no_rotate: annotation_flags & 16!=0,
            })
        }
    }
    pub(crate) fn validate_fill(&self, request: &FillFormFieldRequest) -> EngineResult<ValidatedFill> {
        let annot = self.annotation(request.page_index, request.annotation_index)?;
        let before = self.inspect(request.annotation_index, annot.handle)?;
        if let Some(reason) = &before.reason {
            return Err(bad(reason));
        }
        if before.value != normalized(&request.expected_value) {
            return Err(bad("field changed; reopen the form and try again"));
        }
        let wanted = normalized(&request.value);
        match (&before.value, &wanted) {
            (FormFieldValue::Text { .. }, FormFieldValue::Text { text }) => {
                if text.contains('\0') || text.encode_utf16().count() > MAX_TEXT {
                    return Err(bad("text contains NUL or exceeds 32768 UTF-16 units"));
                }
                if !before.multiline && text.contains('\n') {
                    return Err(bad("this field accepts a single line"));
                }
            }
            (FormFieldValue::Toggle { .. }, FormFieldValue::Toggle { checked }) => {
                if before.kind == "radio" && !checked {
                    return Err(bad("select another radio option to change this group"));
                }
            }
            (_, FormFieldValue::Choice { indices })
                if before.kind == "combo" || before.kind == "list" =>
            {
                if indices.len() > before.options.len()
                    || indices.iter().any(|i| *i >= before.options.len())
                    || indices.windows(2).any(|w| w[0] >= w[1])
                    || (!before.multi_select && indices.len() > 1)
                    || (before.kind == "combo" && indices.is_empty())
                {
                    return Err(bad("invalid choice selection"));
                }
            }
            _ => return Err(bad("value does not match the field type")),
        }
        Ok(ValidatedFill { field: before, wanted })
    }
    pub(crate) fn fill(&self, request: &FillFormFieldRequest) -> EngineResult<Vec<u8>> {
        let ValidatedFill { field: before, wanted } = self.validate_fill(request)?;
        unsafe {
            let annot = self.annotation(request.page_index, request.annotation_index)?;
            let b = self.bindings;
            let page = self.page(request.page_index)?;
            if b.FORM_SetFocusedAnnot(self.form, annot.handle) == 0 {
                return Err(bad("this field could not receive focus"));
            }
            match &wanted {
                FormFieldValue::Text { text } => {
                    if b.FORM_SelectAllText(self.form, page) == 0 {
                        return Err(bad("could not select the field text"));
                    }
                    if before.editable_choice {
                        // ReplaceSelection alone leaves the combo's selected option
                        // active and blur restores its old export value. A character
                        // event clears that selection before replacing the draft.
                        // This private document never executes field scripts.
                        if b.FORM_OnChar(self.form, page, 32, 0) == 0
                            || b.FORM_SelectAllText(self.form, page) == 0
                        {
                            return Err(bad("could not edit this combo box"));
                        }
                    }
                    let units = text
                        .encode_utf16()
                        .chain(std::iter::once(0))
                        .collect::<Vec<_>>();
                    b.FORM_ReplaceSelection(self.form, page, units.as_ptr());
                }
                FormFieldValue::Toggle { .. } => {
                    if before.value != wanted && b.FORM_OnChar(self.form, page, 32, 0) == 0 {
                        return Err(bad("could not change this selection"));
                    }
                }
                FormFieldValue::Choice { indices } => {
                    if before.kind == "list" {
                        for i in 0..before.options.len() {
                            if b.FORM_SetIndexSelected(self.form, page, i as i32, 0) == 0 {
                                return Err(bad("could not clear list selection"));
                            }
                        }
                    }
                    for i in indices {
                        if b.FORM_SetIndexSelected(self.form, page, *i as i32, 1) == 0 {
                            return Err(bad("could not select this option"));
                        }
                    }
                }
            }
            if b.FORM_ForceToKillFocus(self.form) == 0 {
                return Err(bad("could not commit the field"));
            }
            if !self.matches_value(request.page_index, request.annotation_index, &wanted)? {
                return Err(bad("the field could not retain the complete value; check its length and formatting limits"));
            }
            self.actions.restore(self.snapshot_private(512 * 1024 * 1024)?)
        }
    }
    /// Sanitized private document only. The fill publication path restores actions;
    /// isolated appearance probes contain no actions and never publish this document.
    pub(crate) fn snapshot_private(&self, limit: usize) -> EngineResult<Vec<u8>> {
        if limit == 0 || limit > 512 * 1024 * 1024 {
            return Err(bad("invalid private snapshot byte limit"));
        }
        let mut writer = Writer {
            api: FPDF_FILEWRITE { version: 1, WriteBlock: Some(write_block) },
            bytes: crate::pdf_output::BoundedWriter::new(limit),
            limit,
            failed: false,
        };
        let saved = unsafe { self.bindings.FPDF_SaveAsCopy(self.document, &mut writer.api, 0) };
        if saved == 0 || writer.failed {
            return Err(bad("could not save the private form within its byte limit"));
        }
        writer.bytes.finish()
    }
    pub(crate) fn matches_value(
        &self,
        page: usize,
        index: usize,
        wanted: &FormFieldValue,
    ) -> EngineResult<bool> {
        let annotation = self.annotation(page, index)?;
        let actual = self.inspect(index, annotation.handle)?;
        Ok(match wanted {
            FormFieldValue::Choice { indices } => actual.selected_indices == *indices,
            _ => actual.value == normalized(wanted),
        })
    }
}

#[repr(C)]
struct Writer {
    api: FPDF_FILEWRITE,
    bytes: crate::pdf_output::BoundedWriter,
    limit: usize,
    failed: bool,
}
unsafe extern "C" fn write_block(
    writer: *mut FPDF_FILEWRITE,
    data: *const c_void,
    size: c_ulong,
) -> c_int {
    // repr(C) puts api at offset zero. PDFium invokes this synchronously and never
    // retains it. No callbacks into PDFium, panics, or unbounded allocation here.
    let writer = &mut *(writer as *mut Writer);
    let size = size as usize;
    if writer.failed || size > writer.limit || (size > 0 && data.is_null()) {
        writer.failed = true;
        return 0;
    }
    if size > 0 {
        if std::io::Write::write_all(
            &mut writer.bytes,
            std::slice::from_raw_parts(data.cast::<u8>(), size),
        ).is_err() {
            writer.failed = true;
            return 0;
        }
    }
    1
}
