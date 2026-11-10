//! Native-validated preview bundles and exact, exclusive publication of their bytes.
use super::form_automation::{self,FormMetadata};
use super::font_inputs::{FontChoice, FontInputs};
use super::original_snapshot::OriginalSnapshot;
use super::paragraph_preview::{self, ParagraphMetadata};
use super::{inspection, nested_inspection, invalid, io, path_result, selected, Source};
use crate::{DocumentInfo, EngineResult, ExportRequest, PageContentReplacement, PdfEngine};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::Path,
};

pub(crate) const MAX_BUNDLE: u64 = 512 * 1024 * 1024;
const MAX_PNG: u64 = 32 * 1024 * 1024;
const MAX_MANIFEST: u64 = 64 * 1024;
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct TextEdit {
    pub page: u32,
    pub selection_id: String,
    pub replacement: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "paragraph_preview::non_null")]
    pub font_id: Option<String>,
}
impl TextEdit {
    pub fn validate(&self) -> EngineResult<()> {
        if !(1..=10000).contains(&self.page)
            || !(valid_selection(&self.selection_id) || nested_inspection::valid_selection(&self.selection_id))
            || self.replacement.is_empty()
            || self.replacement.len() > 4096
            || self.font_id.as_deref().is_some_and(|id| !valid_hash(id))
        {
            return Err(invalid("Invalid text edit: choose an inspected selection and 1 to 4096 UTF-8 bytes of replacement text."));
        }
        Ok(())
    }
    pub fn read(path: &Path) -> EngineResult<Self> {
        let bytes = read_bounded(path, 32 * 1024)?;
        let edit: Self = serde_json::from_slice(&bytes)
            .map_err(|_| invalid("The edit request must match the declared JSON schema."))?;
        edit.validate()?;
        Ok(edit)
    }
}
pub(crate) fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn valid_selection(value: &str) -> bool {
    value.strip_prefix("text-v1-").is_some_and(valid_hash)
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn open_regular(path: &Path) -> EngineResult<File> {
    let meta = fs::symlink_metadata(path).map_err(io)?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err(invalid(
            "A preview file is missing or is not a regular file.",
        ));
    }
    let file = File::open(path).map_err(io)?;
    if !file.metadata().map_err(io)?.is_file() {
        return Err(invalid("A preview file is not regular."));
    }
    Ok(file)
}
pub(super) fn read_bounded(path: &Path, limit: u64) -> EngineResult<Vec<u8>> {
    let file = open_regular(path)?;
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes).map_err(io)?;
    if bytes.len() as u64 > limit {
        return Err(invalid(
            "The preview request or manifest exceeds its byte limit.",
        ));
    }
    Ok(bytes)
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct FileProof {
    pub sha256: String,
    pub bytes: u64,
}
pub(super) fn copy_hash(path: &Path, limit: u64, mut out: impl Write) -> EngineResult<FileProof> {
    let mut file = open_regular(path)?;
    let mut digest = Sha256::new();
    let mut count = 0u64;
    let mut chunk = [0u8; 32768];
    loop {
        let n = file.read(&mut chunk).map_err(io)?;
        if n == 0 {
            break;
        }
        count = count
            .checked_add(n as u64)
            .filter(|v| *v <= limit)
            .ok_or_else(|| invalid("Preview files exceed their byte limit."))?;
        digest.update(&chunk[..n]);
        out.write_all(&chunk[..n]).map_err(io)?;
    }
    Ok(FileProof {
        sha256: hex(&digest.finalize()),
        bytes: count,
    })
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PreviewManifest {
    pub version: u32,
    pub source_fingerprint: String,
    pub page: u32,
    pub selection_id: String,
    pub old_text: String,
    pub replacement: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "paragraph_preview::non_null")]
    pub substitute_font: Option<FontChoice>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "paragraph_preview::non_null"
    )]
    pub paragraph: Option<ParagraphMetadata>,
    #[serde(default,skip_serializing_if="Option::is_none",deserialize_with="paragraph_preview::non_null")]
    pub form: Option<FormMetadata>,
    pub candidate: FileProof,
    pub before: FileProof,
    pub after: FileProof,
}
impl PreviewManifest {
    pub fn manifest_limit(&self)->u64 {if self.version==3 {form_automation::MAX_MANIFEST as u64}else{MAX_MANIFEST}}
    pub fn request_limit(&self)->u64 {if self.version==3 {form_automation::MAX_REQUEST as u64}else{32*1024}}
    pub fn bytes(&self) -> u64 {
        self.candidate.bytes + self.before.bytes + self.after.bytes
    }
    fn validate(&self) -> EngineResult<()> {
        if let Some(font) = &self.substitute_font {
            if !matches!(self.version, 1 | 4) {
                return Err(invalid("Single-run font metadata requires a text preview."));
            }
            font.validate()?;
        }
        match (self.version, &self.paragraph, &self.form) {
            (version @ (1 | 4), None, None) => {
                let valid = if version == 1 {valid_selection(&self.selection_id)} else {nested_inspection::valid_selection(&self.selection_id)};
                if !valid {return Err(invalid("Text preview version and selection kind do not match."));}
                TextEdit {
                page: self.page,
                selection_id: self.selection_id.clone(),
                replacement: self.replacement.clone(),
                font_id: self.substitute_font.as_ref().map(|font| font.font_id.clone()),
            }
            .validate()?;
            },
            (version @ (2 | 5), Some(paragraph), None) => {
                paragraph.validate()?;
                if !(1..=10000).contains(&self.page)
                    || !paragraph_preview::valid_pair(&paragraph.seed_selection_id, &self.selection_id)
                    || (version == 5) != paragraph.occurrence_scope.is_some()
                    || paragraph.text() != self.replacement
                {
                    return Err(invalid("Invalid paragraph preview manifest."));
                }
            }
            (3,None,Some(form))=>{
                form.validate()?;
                if !(1..=10000).contains(&self.page) || !form_automation::valid_field(&self.selection_id) || !self.old_text.is_empty() || !self.replacement.is_empty() {return Err(invalid("Invalid form preview manifest."));}
            }
            _ => return Err(invalid("Invalid preview version or metadata.")),
        }
        if !valid_hash(&self.source_fingerprint)
            || self.old_text.len() > 16 * 1024
            || [&self.candidate, &self.before, &self.after]
                .iter()
                .any(|p| !valid_hash(&p.sha256) || p.bytes == 0 || p.bytes > MAX_BUNDLE)
            || self.before.bytes > MAX_PNG
            || self.after.bytes > MAX_PNG
            || self.bytes() > MAX_BUNDLE
        {
            return Err(invalid(
                "The preview manifest is invalid or exceeds its bounds.",
            ));
        }
        Ok(())
    }
}
pub(crate) fn read_manifest(folder: &Path, expected: &str) -> EngineResult<PreviewManifest> {
    if !valid_hash(expected) {
        return Err(invalid("Invalid preview manifest fingerprint."));
    }
    let bytes = read_bounded(&folder.join("manifest.json"), form_automation::MAX_MANIFEST as u64)?;
    if hex(&Sha256::digest(&bytes)) != expected {
        return Err(invalid(
            "The preview manifest changed. Create a fresh preview.",
        ));
    }
    let manifest: PreviewManifest = serde_json::from_slice(&bytes)
        .map_err(|_| invalid("The preview manifest schema is invalid."))?;
    manifest.validate()?;
    if bytes.len() as u64>manifest.manifest_limit() {return Err(invalid("Preview manifest exceeds its byte limit."));}
    if manifest.bytes() + bytes.len() as u64 > MAX_BUNDLE {
        return Err(invalid("Preview bundle exceeds its byte limit."));
    }
    Ok(manifest)
}
pub(crate) fn validate_bundle(folder: &Path, expected: &str) -> EngineResult<PreviewManifest> {
    let manifest = read_manifest(folder, expected)?;
    for (name, proof, limit) in [
        ("candidate.pdf", &manifest.candidate, MAX_BUNDLE),
        ("before.png", &manifest.before, MAX_PNG),
        ("after.png", &manifest.after, MAX_PNG),
    ] {
        if copy_hash(&folder.join(name), limit, std::io::sink())? != *proof {
            return Err(invalid(
                "The preview files changed. Create a fresh preview.",
            ));
        }
    }
    Ok(manifest)
}

pub(super) fn preview(
    engine: &PdfEngine,
    info: &DocumentInfo,
    edit: TextEdit,
    parent: &Path,
    password: Option<&str>,
    original: &OriginalSnapshot,
    fonts: &FontInputs,
) -> EngineResult<Value> {
    edit.validate()?;
    let (run, source_fingerprint) =
        inspection::resolve_any(engine, info, edit.page, &edit.selection_id)?;
    let substitute_font = edit.font_id.as_deref()
        .map(|id| fonts.get(id).and_then(|asset| FontChoice::from_asset(&asset)))
        .transpose()?;
    let _font_install = fonts.install(&engine.fonts())?;
    let order = (0..info.pages.len()).collect::<Vec<_>>();
    let index = edit.page as usize - 1;
    let (changed,old_text) = match run {
        inspection::ResolvedText::Page(run) => (
            engine.replace_text_occurrence(&info.id,&order,index,run.object_index,&run.text,&edit.replacement,edit.font_id.as_deref())?,
            run.source_text.unwrap_or(run.text)),
        inspection::ResolvedText::Nested(run) => (
            engine.replace_form_text_occurrence(&info.id,&order,index,&run.object_path,&run.text,&edit.replacement,edit.font_id.as_deref())?,
            run.text),
    };
    let unchanged = old_text == edit.replacement && edit.font_id.is_none();
    publish(
        engine,
        info,
        changed,
        PreviewDescription {
            unchanged_input: unchanged.then_some(original),
            source_fingerprint,
            page: edit.page,
            selection_id: edit.selection_id,
            old_text,
            replacement: edit.replacement,
            substitute_font,
            paragraph: None,
            form: None,
        },
        parent,
        password,
    )
}

pub(super) struct PreviewDescription<'a> {
    // Only selected after the request and native replacement have been validated.
    pub unchanged_input: Option<&'a OriginalSnapshot>,
    pub source_fingerprint: String,
    pub page: u32,
    pub selection_id: String,
    pub old_text: String,
    pub replacement: String,
    pub substitute_font: Option<FontChoice>,
    pub paragraph: Option<ParagraphMetadata>,
    pub form: Option<FormMetadata>,
}

pub(super) fn publish(
    engine: &PdfEngine,
    info: &DocumentInfo,
    changed: PageContentReplacement,
    description: PreviewDescription<'_>,
    parent: &Path,
    password: Option<&str>,
) -> EngineResult<Value> {
    let order = (0..info.pages.len()).collect::<Vec<_>>();
    let candidate = Source {
        engine,
        info: changed.document,
    };
    if changed.page_indices != order || candidate.info.pages.len() != info.pages.len() {
        return Err(invalid("The text preview changed the source page order."));
    }
    let parent = parent.canonicalize().map_err(io)?;
    let staging = tempfile::Builder::new()
        .prefix("Folio preview-")
        .tempdir_in(parent)
        .map_err(io)?;
    let folder = staging.path();
    let pdf = folder.join("candidate.pdf");
    let candidate_proof = if let Some(original) = description.unchanged_input {
        original.write_candidate(&pdf)?
    } else {
        engine.export_pdf(
            ExportRequest {
                pages: selected(&candidate.info, None)?,
                flatten: false,
            },
            &pdf,
        )?;
        copy_hash(&pdf, MAX_BUNDLE, std::io::sink())?
    };
    let reopened = match password {
        Some(p) => engine.open_document_with_password(&pdf, p)?,
        None => engine.open_document(&pdf)?,
    };
    let reopened = Source {
        engine,
        info: reopened,
    };
    for (name, source) in [("before.png", info), ("after.png", &reopened.info)] {
        let plan = selected(source, Some(description.page))?.remove(0);
        let png = engine.render_page_plan(plan, 1200)?;
        if png.len() as u64 > MAX_PNG {
            return Err(invalid("Preview image exceeds its byte limit."));
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(folder.join(name))
            .map_err(io)?;
        file.write_all(&png).map_err(io)?;
        file.sync_all().map_err(io)?;
    }
    let manifest = PreviewManifest {
        version: if description.form.is_some() {3} else if description.paragraph.is_some() {
            if paragraph_preview::valid_nested_paragraph(&description.selection_id) {5} else {2}
        } else if nested_inspection::valid_selection(&description.selection_id) {
            4
        } else {
            1
        },
        source_fingerprint: description.source_fingerprint,
        page: description.page,
        selection_id: description.selection_id,
        old_text: description.old_text,
        replacement: description.replacement,
        substitute_font: description.substitute_font,
        paragraph: description.paragraph,
        form: description.form,
        candidate: candidate_proof,
        before: copy_hash(&folder.join("before.png"), MAX_PNG, std::io::sink())?,
        after: copy_hash(&folder.join("after.png"), MAX_PNG, std::io::sink())?,
    };
    manifest.validate()?;
    let bytes =
        serde_json::to_vec(&manifest).map_err(|_| invalid("Cannot encode preview metadata."))?;
    if bytes.len() as u64 > manifest.manifest_limit() || manifest.bytes() + bytes.len() as u64 > MAX_BUNDLE {
        return Err(invalid("Preview bundle exceeds its byte limit."));
    }
    let manifest_hash = hex(&Sha256::digest(&bytes));
    fs::write(folder.join("manifest.json"), bytes).map_err(io)?;
    let mut result = json!({"bundleName":folder.file_name().and_then(|s|s.to_str()).ok_or_else(||invalid("Invalid preview bundle name."))?,
        "manifestSha256":manifest_hash,"page":manifest.page,"sourceFingerprint":manifest.source_fingerprint,
        "oldText":manifest.old_text,"replacement":manifest.replacement,"candidateSha256":manifest.candidate.sha256});
    if let Some(paragraph) = &manifest.paragraph {
        result["paragraphId"] = json!(manifest.selection_id);
        result["paragraph"] = json!(paragraph);
    }
    if let Some(font) = &manifest.substitute_font { result["substituteFont"] = json!(font); }
    if let Some(form)=&manifest.form {result.as_object_mut().unwrap().remove("oldText");result.as_object_mut().unwrap().remove("replacement");result["fieldId"]=json!(manifest.selection_id);result["form"]=json!(form);}
    result = path_result(result, "folder", folder);
    for (field, name) in [
        ("candidate", "candidate.pdf"),
        ("before", "before.png"),
        ("after", "after.png"),
    ] {
        result = path_result(result, field, &folder.join(name));
    }
    let _ = staging.keep();
    Ok(result)
}

pub(super) fn save(folder: &Path, manifest_hash: &str, output: &Path) -> EngineResult<Value> {
    if !output
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
    {
        return Err(invalid("Choose a new .pdf output filename."));
    }
    super::ensure_absent(output)?;
    let manifest = validate_bundle(folder, manifest_hash)?;
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .canonicalize()
        .map_err(io)?;
    let output = parent.join(
        output
            .file_name()
            .ok_or_else(|| invalid("Choose a PDF output filename."))?,
    );
    let mut staged = tempfile::NamedTempFile::new_in(parent).map_err(io)?;
    let copied = copy_hash(
        &folder.join("candidate.pdf"),
        MAX_BUNDLE,
        staged.as_file_mut(),
    )?;
    if copied != manifest.candidate {
        return Err(invalid(
            "The candidate changed while saving. Create a fresh preview.",
        ));
    }
    staged.as_file_mut().sync_all().map_err(io)?;
    let result = path_result(
        json!({"bytes":copied.bytes,"sha256":copied.sha256,"page":manifest.page}),
        "path",
        &output,
    );
    staged.persist_noclobber(&output).map_err(|e| {
        if e.error.kind() == std::io::ErrorKind::AlreadyExists {
            invalid("The output already exists. Choose a new filename.")
        } else {
            io(&e.error)
        }
    })?;
    Ok(result)
}

#[cfg(test)]
mod nested_manifest_tests {
    use super::*;

    #[test]
    fn single_run_font_metadata_is_optional_bounded_and_text_only() {
        let choice=json!({"fontId":"f".repeat(64),"name":"Chosen font","weight":400,"italic":false,"bytes":1024});
        for (version,prefix) in [(1,"text-v1-"),(4,"nested-text-v1-")] {
            let old=manifest(version,format!("{prefix}{}","e".repeat(64)));
            old.validate().unwrap();let mut value=serde_json::to_value(old).unwrap();
            assert!(value.get("substituteFont").is_none());
            value["substituteFont"]=choice.clone();
            serde_json::from_value::<PreviewManifest>(value.clone()).unwrap().validate().unwrap();
            for (field,bad) in [("fontId",json!("invalid")),("name",json!("x".repeat(513))),("bytes",json!(16*1024*1024+1)),("weight",json!(1001))] {
                let mut invalid=value.clone();invalid["substituteFont"][field]=bad;
                assert!(serde_json::from_value::<PreviewManifest>(invalid).unwrap().validate().is_err());
            }
            value["substituteFont"]=Value::Null;
            assert!(serde_json::from_value::<PreviewManifest>(value).is_err());
        }
        let mut paragraph=serde_json::to_value(manifest(1,format!("text-v1-{}","e".repeat(64)))).unwrap();
        paragraph["version"]=json!(2);paragraph["selectionId"]=json!(format!("paragraph-v1-{}","e".repeat(64)));
        paragraph["paragraph"]=json!({"seedSelectionId":format!("text-v1-{}","e".repeat(64)),"runs":[{"text":"New","styleId":0}],"bounds":{"x":10.,"y":10.,"width":100.,"height":50.},"alignment":"left","lineSpacing":1.2});
        serde_json::from_value::<PreviewManifest>(paragraph.clone()).unwrap().validate().unwrap();
        paragraph["substituteFont"]=choice;
        assert!(serde_json::from_value::<PreviewManifest>(paragraph).unwrap().validate().is_err());
    }

    fn manifest(version: u32, selection_id: String) -> PreviewManifest {
        serde_json::from_value(json!({
            "version":version,"sourceFingerprint":"a".repeat(64),"page":1,
            "selectionId":selection_id,"oldText":"Old","replacement":"New",
            "candidate":{"sha256":"b".repeat(64),"bytes":1},
            "before":{"sha256":"c".repeat(64),"bytes":1},
            "after":{"sha256":"d".repeat(64),"bytes":1}
        }))
        .unwrap()
    }

    #[test]
    fn nested_preview_version_cannot_masquerade_as_page_text() {
        let page = format!("text-v1-{}", "e".repeat(64));
        let nested = format!("nested-text-v1-{}", "e".repeat(64));
        assert!(manifest(1, page.clone()).validate().is_ok());
        assert!(manifest(4, nested.clone()).validate().is_ok());
        assert!(manifest(1, nested.clone()).validate().is_err());
        assert!(manifest(4, page).validate().is_err());
        assert!(manifest(2, nested.clone()).validate().is_err());
        assert!(manifest(3, nested).validate().is_err());
    }
}
