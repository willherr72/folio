//! Opaque, expiring challenges retain chosen bytes. No retry accepts a file path.
use super::*;
use crate::{
    private_storage::Sensitive,
    protection::{unlock_pdf, ProtectionError},
};
use std::time::{Duration, Instant};

const MAX_PENDING: usize = 4;
const LIFETIME: Duration = Duration::from_secs(10 * 60);
pub(super) struct PendingOpen {
    bytes: Vec<u8>,
    path: PathBuf,
    created: Instant,
}

fn secret(password: &str) -> EngineResult<Sensitive> {
    if password.len() > 1024 {
        return Err(ProtectionError::Limit("password exceeds 1024 bytes").into());
    }
    Ok(Sensitive(password.as_bytes().to_vec()))
}
pub(super) fn password_str(password: &Sensitive) -> &str {
    std::str::from_utf8(&password.0).expect("password originates as UTF-8")
}
fn expired() -> EngineError {
    EngineError::InvalidRequest(
        "This password request expired or was cancelled. Open the PDF again.".into(),
    )
}
fn name(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("document.pdf")
        .into()
}

impl PdfEngine {
    pub fn begin_open_document(&self, path: impl AsRef<Path>) -> EngineResult<OpenOutcome> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::BeginOpen {
            path: path.as_ref().to_path_buf(),
            reply,
        })?;
        self.outcome_in_points(receive.recv().map_err(|_| EngineError::WorkerStopped)?)
    }
    pub fn submit_open_password(
        &self,
        challenge_id: &str,
        password: &str,
    ) -> EngineResult<OpenOutcome> {
        let password = secret(password)?;
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::SubmitOpenPassword {
            challenge_id: challenge_id.into(),
            password,
            reply,
        })?;
        self.outcome_in_points(receive.recv().map_err(|_| EngineError::WorkerStopped)?)
    }
    pub fn cancel_open_document(&self, challenge_id: &str) -> EngineResult<()> {
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::CancelOpen {
            challenge_id: challenge_id.into(),
            reply,
        })?;
        receive.recv().map_err(|_| EngineError::WorkerStopped)?
    }
    pub fn unlock_document_sources(
        &self,
        source_ids: &[String],
        password: &str,
    ) -> EngineResult<Vec<SourceReplacement>> {
        let password = secret(password)?;
        let (reply, receive) = mpsc::channel();
        self.send(WorkerRequest::UnlockSources {
            source_ids: source_ids.to_vec(),
            password,
            reply,
        })?;
        self.replacements_in_points(receive.recv().map_err(|_| EngineError::WorkerStopped)?)
    }
}

impl WorkerRuntime {
    pub(super) fn expire_opens(&mut self) {
        self.pending_opens
            .retain(|_, p| p.created.elapsed() < LIFETIME);
    }

    pub(super) fn begin_open_document(&mut self, path: PathBuf) -> EngineResult<OpenOutcome> {
        self.expire_opens();
        let (path, bytes) = Self::read_source_file(&path)?;
        if self.is_unprotected(&bytes) {
            return self
                .open_bytes(
                    bytes,
                    path.clone(),
                    path,
                    true,
                    true,
                    false,
                    OpenSecurity::Authenticated(None),
                )
                .map(|document| OpenOutcome::Opened { document });
        }
        match unlock_pdf(&bytes, None) {
            Ok(unlocked) => self
                .open_bytes(
                    unlocked.bytes,
                    path.clone(),
                    path,
                    true,
                    true,
                    false,
                    OpenSecurity::Authenticated(unlocked.protection),
                )
                .map(|document| OpenOutcome::Opened { document }),
            Err(ProtectionError::PasswordRequired) => {
                let pending_bytes = self
                    .pending_opens
                    .values()
                    .map(|p| p.bytes.len() as u64)
                    .sum::<u64>();
                if self.pending_opens.len() >= MAX_PENDING
                    || pending_bytes + bytes.len() as u64 > MAX_SOURCE_BYTES
                {
                    return Err(EngineError::InvalidRequest(
                        "Close another password dialog before opening this PDF.".into(),
                    ));
                }
                let challenge_id = Uuid::new_v4().to_string();
                let result = OpenOutcome::PasswordRequired {
                    challenge_id: challenge_id.clone(),
                    name: name(&path),
                    incorrect: false,
                };
                self.pending_opens.insert(
                    challenge_id,
                    PendingOpen {
                        bytes,
                        path,
                        created: Instant::now(),
                    },
                );
                Ok(result)
            }
            Err(error) => Err(error.into()),
        }
    }

    pub(super) fn submit_open_password(
        &mut self,
        id: &str,
        password: &str,
    ) -> EngineResult<OpenOutcome> {
        self.expire_opens();
        let pending = self.pending_opens.get(id).ok_or_else(expired)?;
        let result = unlock_pdf(&pending.bytes, Some(password));
        if matches!(result, Err(ProtectionError::IncorrectPassword)) {
            return Ok(OpenOutcome::PasswordRequired {
                challenge_id: id.into(),
                name: name(&pending.path),
                incorrect: true,
            });
        }
        let pending = self.pending_opens.remove(id).ok_or_else(expired)?;
        let unlocked = result?;
        self.open_bytes(
            unlocked.bytes,
            pending.path.clone(),
            pending.path,
            true,
            true,
            false,
            OpenSecurity::Authenticated(unlocked.protection),
        )
        .map(|document| OpenOutcome::Opened { document })
    }

    pub(super) fn reply_open(
        &mut self,
        reply: mpsc::Sender<EngineResult<OpenOutcome>>,
        result: EngineResult<OpenOutcome>,
    ) {
        if let Err(mpsc::SendError(Ok(outcome))) = reply.send(result) {
            match outcome {
                OpenOutcome::Opened { document } => {
                    self.discard_unpublished_source(&document.id);
                }
                OpenOutcome::PasswordRequired { challenge_id, .. } => {
                    self.pending_opens.remove(&challenge_id);
                }
            }
        }
    }

    pub(super) fn unlock_document_sources(
        &mut self,
        ids: &[String],
        password: &str,
    ) -> EngineResult<Vec<SourceReplacement>> {
        if ids.is_empty()
            || ids.len() > 1000
            || ids.iter().collect::<HashSet<_>>().len() != ids.len()
        {
            return Err(EngineError::InvalidRequest(
                "Select the protected document to unlock.".into(),
            ));
        }
        let first = self.document(&ids[0])?;
        let policy = first
            .policy
            .as_ref()
            .ok_or_else(|| EngineError::InvalidRequest("This PDF is not protected.".into()))?;
        let owner = Arc::new(policy.protection.authenticate_owner(password)?);
        let mut candidates = Vec::with_capacity(ids.len());
        let mut total = 0u64;
        // Validate the whole set before registering any new source. Old versions
        // (and cached recovery contexts) never change their owner capability.
        for id in ids {
            let source = self.document(id)?;
            let p = source
                .policy
                .as_ref()
                .filter(|p| p.same_origin(policy))
                .ok_or_else(|| {
                    EngineError::InvalidRequest("Unlock one protected document at a time.".into())
                })?;
            if source.for_printing {
                return Err(EngineError::PermissionDenied("unlocking a print document"));
            }
            total = total
                .checked_add(source.source_bytes.identity().len() as u64)
                .filter(|n| *n <= MAX_SOURCE_BYTES)
                .ok_or_else(|| {
                    EngineError::InvalidRequest(
                        "Unlocking these source versions exceeds the 512 MiB limit.".into(),
                    )
                })?;
            candidates.push((
                id.clone(),
                source.bytes()?,
                source.path.clone(),
                source.original_path.clone(),
                p.with_owner(owner.clone()),
                source.legacy_comment_import,
            ));
        }
        let mut result: Vec<SourceReplacement> = Vec::with_capacity(ids.len());
        for (source_id, bytes, path, original_path, policy, legacy_comment_import) in candidates {
            let opened = self.open_bytes(
                bytes.to_vec(),
                path,
                original_path,
                true,
                false,
                legacy_comment_import,
                OpenSecurity::Restored(Some(policy)),
            ).and_then(|document| {
                let candidate = document.id.clone();
                let result = self.retain_unlock_image_leases(&source_id, &candidate)
                    .and_then(|()| self.retain_annotation_sources(&source_id, document));
                if result.is_err() { self.discard_unpublished_source(&candidate); }
                result
            });
            match opened {
                Ok(document) => result.push(SourceReplacement {
                    source_id,
                    document,
                }),
                Err(error) => {
                    for replacement in result {
                        self.discard_unpublished_source(&replacement.document.id);
                    }
                    return Err(error);
                }
            }
        }
        Ok(result)
    }

    fn retain_unlock_image_leases(&mut self, original: &str, candidate: &str) -> EngineResult<()> {
        // Byte-identical replacements need independent ownership of every source
        // image, including recovery drafts not embedded in the native PDF.
        // Acquire preserves each asset's access restrictions; a failed partial
        // acquisition drops this guard and releases all completed acquisitions.
        let mut leases = ImageSourceLeases { registry: self.images.clone(), registrations: vec![] };
        if let Some(original) = &self.document(original)?.image_leases {
            if original.registrations.len().checked_mul(512).is_none_or(|n| n > 16 * 1024 * 1024) {
                return Err(EngineError::InvalidRequest("Unlocking image ownership exceeds the 16 MiB metadata limit.".into()));
            }
            leases.registrations.reserve_exact(original.registrations.len());
            for registration in &original.registrations {
                leases.registrations.push(self.images.acquire(&registration.image.id)?);
            }
        }
        self.documents.get_mut(candidate).ok_or_else(expired)?.image_leases = Some(leases);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn imported_overlay_sources_unlock_failure_and_abandoned_reply_release_image_leases() {
        for mode in ["late-source", "partial-acquire", "abandoned"] {
            let root=PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let mut runtime=WorkerRuntime::new(&root.join("resources/pdfium/pdfium.dll"),Arc::new(FontRegistry::new())).unwrap();
            let OpenOutcome::PasswordRequired{challenge_id,..}=runtime.begin_open_document(root.join("../tests/fixtures/protection/aes_256.pdf")).unwrap() else {panic!()};
            let OpenOutcome::Opened{document}=runtime.submit_open_password(&challenge_id,"reader").unwrap() else {panic!()};
            let mut png=std::io::Cursor::new(Vec::new());
            image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(2,2,image::Rgba([25,155,245,255])))
                .write_to(&mut png,image::ImageFormat::Png).unwrap();
            let origin=runtime.document(&document.id).unwrap().policy.as_ref().unwrap().image_origin().to_owned();
            let asset=Arc::new(crate::ImageAsset::parse(png.into_inner()).unwrap());
            let mut registrations=runtime.images.register_source_assets(&[asset],Some(&origin)).unwrap();
            let image_id=registrations[0].image.id.clone();
            if mode == "partial-acquire" {
                let mut missing=registrations[0].clone();missing.image.id="missing-image".into();missing.token="missing-token".into();
                registrations.push(missing);
            }
            runtime.documents.get_mut(&document.id).unwrap().image_leases=Some(ImageSourceLeases{registry:runtime.images.clone(),registrations});
            let mut ids=vec![document.id.clone()];
            if mode == "late-source" {
                let source=runtime.document(&document.id).unwrap();
                let (bytes,path,original,policy)=(source.bytes().unwrap().to_vec(),source.path.clone(),source.original_path.clone(),source.policy.clone());
                let other=runtime.open_bytes(bytes,path,original,true,false,false,OpenSecurity::Restored(policy)).unwrap();
                runtime.documents.get_mut(&other.id).unwrap().source_bytes=crate::source_storage::SourceStore::new().capture(b"invalid second candidate".to_vec().into()).unwrap();
                ids.push(other.id);
            }
            let original_ids:HashSet<_>=runtime.documents.keys().cloned().collect();
            if mode == "abandoned" {
                let (send,receive)=mpsc::channel();let (reply,receiver)=mpsc::channel();drop(receiver);
                send.send(WorkerRequest::UnlockSources{source_ids:ids.clone(),password:secret("owner").unwrap(),reply}).unwrap();
                drop(send);runtime.run(receive);
            } else {
                assert!(runtime.unlock_document_sources(&ids,"owner").is_err(),"{mode}");
            }
            assert_eq!(runtime.documents.keys().cloned().collect::<HashSet<_>>(),original_ids,"{mode}");
            assert!(runtime.images.get(&image_id).is_ok());
            assert!(runtime.images.authorize(&image_id,None).is_err(),"access must not widen on failed/abandoned unlock");
            assert!(!runtime.document(&document.id).unwrap().policy.as_ref().unwrap().info().owner);
            for id in ids { runtime.close_document(&id).unwrap(); }
            assert!(runtime.images.get(&image_id).is_err(),"{mode}: unpublished replacement leaked its lease");
        }
    }
    #[test]
    fn abandoned_open_reclaims_imported_fonts_but_preserves_other_registrations() {
        let root=PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut runtime=WorkerRuntime::new(&root.join("resources/pdfium/pdfium.dll"),Arc::new(FontRegistry::new())).unwrap();
        let font_bytes=std::fs::read(root.join("../tests/fixtures/corpus/fonts/DejaVuSerif.ttf")).unwrap();
        let font=runtime.fonts.register(font_bytes.clone()).unwrap();
        let OpenOutcome::PasswordRequired {challenge_id,..}=runtime.begin_open_document(root.join("../tests/fixtures/protection/aes_256.pdf")).unwrap() else {panic!()};
        let OpenOutcome::Opened {document}=runtime.submit_open_password(&challenge_id,"reader").unwrap() else {panic!()};
        let temp=tempfile::tempdir().unwrap();let output=temp.path().join("annotated.pdf");
        let request=serde_json::from_value(serde_json::json!({"pages":[{"id":"p","sourceId":document.id,"pageIndex":0,"width":360,"height":480,"rotation":0,"overlays":[{"id":"t","type":"text","fontId":font.id,"text":"Annotation","x":35,"y":220,"fontSize":12,"color":"#000000"}]}],"flatten":false})).unwrap();
        runtime.export_pdf_with_bookmarks(request,None,output.clone()).unwrap();
        runtime.close_document(&document.id).unwrap();runtime.fonts.remove(&font.id).unwrap();
        for claim in ["none","before","after"] {
            if claim=="before" {runtime.fonts.register(font_bytes.clone()).unwrap();}
            let OpenOutcome::PasswordRequired {challenge_id,..}=runtime.begin_open_document(output.clone()).unwrap() else {panic!()};
            let opened=runtime.submit_open_password(&challenge_id,"reader").unwrap();
            assert!(runtime.fonts.get(&font.id).is_ok());
            if claim=="after" {runtime.fonts.register(font_bytes.clone()).unwrap();}
            let (send,receive)=mpsc::channel();drop(receive);runtime.reply_open(send,Ok(opened));
            assert!(runtime.documents.is_empty());
            assert_eq!(runtime.fonts.get(&font.id).is_ok(),claim!="none","font owner: {claim}");
            runtime.fonts.remove(&font.id).unwrap();
        }
    }
    #[test]
    fn expired_challenges_and_abandoned_replies_do_not_publish_sources() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut runtime = WorkerRuntime::new(
            &root.join("resources/pdfium/pdfium.dll"),
            Arc::new(FontRegistry::new()),
        )
        .unwrap();
        let path = root.join("../tests/fixtures/protection/aes_256.pdf");
        let outcome = runtime.begin_open_document(path.clone()).unwrap();
        let OpenOutcome::PasswordRequired { challenge_id, .. } = outcome else {
            panic!()
        };
        runtime
            .pending_opens
            .get_mut(&challenge_id)
            .unwrap()
            .created = Instant::now() - LIFETIME;
        assert!(runtime
            .submit_open_password(&challenge_id, "owner")
            .is_err());
        assert!(runtime.pending_opens.is_empty() && runtime.documents.is_empty());
        for protected in [true, false] {
            let outcome = runtime
                .begin_open_document(if protected {
                    path.clone()
                } else {
                    root.join("../tests/fixtures/protection/plain.pdf")
                })
                .unwrap();
            let (sender, receiver) = mpsc::channel();
            drop(receiver);
            runtime.reply_open(sender, Ok(outcome));
            assert!(runtime.pending_opens.is_empty() && runtime.documents.is_empty());
        }
    }
}
