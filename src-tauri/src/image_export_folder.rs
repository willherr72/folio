//! Exclusive output ownership and nonrecursive, handle-bound Windows rollback.
use crate::page_image_export::io_error;
use crate::{EngineError, EngineResult};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
const MARKER: &str = "EXPORT-IN-PROGRESS.txt";
const MARKER_TEXT: &[u8] =
    b"This export is incomplete. Do not treat these files as a completed export.\n";
enum Purpose{Images,Split}
pub(crate) struct OwnedImageFolder {
    path: PathBuf,
    identity: Option<same_file::Handle>,
    lock: Option<File>,
    files: Vec<(String, same_file::Handle)>,
}
impl OwnedImageFolder {
    pub(crate) fn create(parent: &Path) -> EngineResult<Self> {
        Self::create_for(parent,Purpose::Images)
    }
    pub(crate) fn create_split(parent: &Path) -> EngineResult<Self> {
        Self::create_for(parent,Purpose::Split)
    }
    fn create_for(parent: &Path,purpose:Purpose) -> EngineResult<Self> {
        // Other platforms need an equally strong deletion protocol before enabling publication.
        if !cfg!(windows) {
            return Err(io_error("Batch export currently requires Windows."));
        }
        let parent = parent.canonicalize().map_err(io_error)?;
        if !parent.is_dir() {
            return Err(io_error("Choose an existing destination folder."));
        }
        let prefix=match purpose{Purpose::Images=>"Folio images",Purpose::Split=>"Folio split"};
        let path = parent.join(format!("{prefix}-{}", uuid::Uuid::new_v4()));
        // Creation and replacement exclusion are one operation, not create-then-open.
        let lock = create_directory_handle(&path)?;
        let mut folder = Self {
            path,
            identity: None,
            lock: Some(lock),
            files: vec![],
        };
        let setup = (|| {
            #[cfg(all(test, windows))]
            tests::hook("created", &folder.path)?;
            #[cfg(all(test, windows))]
            tests::hook("opened", &folder.path)?;
            folder.identity = Some(
                same_file::Handle::from_file(
                    folder
                        .lock
                        .as_ref()
                        .unwrap()
                        .try_clone()
                        .map_err(io_error)?,
                )
                .map_err(io_error)?,
            );
            folder.write(MARKER, MARKER_TEXT)
        })();
        if let Err(error) = setup {
            return Err(folder.rollback_error(error));
        }
        Ok(folder)
    }
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
    fn check(&self) -> EngineResult<()> {
        if self.identity.is_none() {
            return Err(io_error(
                "Export folder ownership could not be established; refusing cleanup.",
            ));
        }
        let metadata = fs::symlink_metadata(&self.path).map_err(io_error)?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || Some(&same_file::Handle::from_path(&self.path).map_err(io_error)?)
                != self.identity.as_ref()
        {
            return Err(io_error(
                "Export folder identity changed; refusing cleanup.",
            ));
        }
        Ok(())
    }
    pub(crate) fn write(&mut self, name: &str, bytes: &[u8]) -> EngineResult<()> {
        self.check()?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            use windows_sys::Win32::{
                Foundation::GENERIC_WRITE,
                Storage::FileSystem::{DELETE, FILE_SHARE_DELETE, FILE_SHARE_READ},
            };
            options
                .access_mode(GENERIC_WRITE | DELETE)
                .share_mode(FILE_SHARE_READ | FILE_SHARE_DELETE);
        }
        let file = options.open(self.path.join(name)).map_err(io_error)?;
        let identity = same_file::Handle::from_file(file).map_err(io_error)?;
        self.files.push((name.into(), identity));
        let file = self.files.last_mut().unwrap().1.as_file_mut();
        file.write_all(bytes).map_err(io_error)?;
        file.sync_all().map_err(io_error)
    }
    fn remove_owned(&mut self, index: usize) -> EngineResult<()> {
        self.check()?;
        let (name, identity) = &self.files[index];
        let path = self.path.join(name);
        let metadata = fs::symlink_metadata(&path).map_err(io_error)?;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || &same_file::Handle::from_path(&path).map_err(io_error)? != identity
        {
            return Err(io_error("Export file identity changed; refusing cleanup."));
        }
        #[cfg(all(test, windows))]
        tests::hook("checked_file", &path)?;
        delete_by_handle(identity.as_file())?;
        self.files.remove(index);
        if path.try_exists().map_err(io_error)? {
            return Err(io_error("An export filename is still present after owned-file deletion; refusing further cleanup."));
        }
        Ok(())
    }
    pub(crate) fn finish(&mut self) -> EngineResult<PathBuf> {
        let index = self
            .files
            .iter()
            .position(|(name, _)| name == MARKER)
            .ok_or_else(|| io_error("Export completion marker is missing."))?;
        self.remove_owned(index)?;
        Ok(self.path.clone())
    }
    pub(crate) fn rollback(mut self) -> EngineResult<()> {
        let result = self.rollback_owned();
        if let Err(error) = result {
            // The directory stays locked through failure and marker restoration. Never overwrite.
            if self.identity.is_some() {
                if let Err(marker) = self.restore_marker() {
                    return Err(io_error(format!(
                        "{error} Could not restore the incomplete marker: {marker}"
                    )));
                }
            }
            return Err(error);
        }
        Ok(())
    }
    fn restore_marker(&mut self) -> EngineResult<()> {
        self.check()?;
        let previous = self
            .files
            .iter()
            .position(|(name, _)| name == MARKER)
            .map(|index| self.files.remove(index).1);
        if let Some(identity) = previous.as_ref() {
            let path = self.path.join(MARKER);
            if fs::symlink_metadata(&path).is_ok_and(|m| m.is_file() && !m.file_type().is_symlink())
                && same_file::Handle::from_path(&path).is_ok_and(|current| &current == identity)
            {
                return Ok(());
            }
        }
        // Close stale tracking (including delete-pending handles), then create without overwrite.
        drop(previous);
        self.write(MARKER, MARKER_TEXT)
    }
    fn rollback_owned(&mut self) -> EngineResult<()> {
        self.check()?;
        for entry in fs::read_dir(&self.path).map_err(io_error)? {
            let name = entry.map_err(io_error)?.file_name();
            if !self
                .files
                .iter()
                .any(|(owned, _)| std::ffi::OsStr::new(owned) == name)
            {
                return Err(io_error(
                    "Unexpected files are present; the incomplete export was preserved.",
                ));
            }
        }
        for index in (0..self.files.len()).rev() {
            self.remove_owned(index)?;
        }
        self.check()?;
        #[cfg(all(test, windows))]
        tests::hook("remove_directory", &self.path)?;
        delete_by_handle(
            self.lock
                .as_ref()
                .ok_or_else(|| io_error("Export folder ownership lock is missing."))?,
        )?;
        self.identity.take();
        self.lock.take();
        Ok(())
    }
    pub(crate) fn rollback_error(self, error: EngineError) -> EngineError {
        let path = self.path.clone();
        match self.rollback() {
            Ok(()) => error,
            Err(cleanup) => io_error(format!(
                "{error} Incomplete export remains at {}: {cleanup}",
                path.display()
            )),
        }
    }
}

#[cfg(windows)]
fn create_directory_handle(path: &Path) -> EngineResult<File> {
    use std::os::windows::{ffi::OsStrExt, io::FromRawHandle};
    use windows_sys::{
        Wdk::{
            Foundation::OBJECT_ATTRIBUTES,
            Storage::FileSystem::{
                NtCreateFile, FILE_CREATE, FILE_DIRECTORY_FILE, FILE_SYNCHRONOUS_IO_NONALERT,
            },
        },
        Win32::{
            Foundation::{RtlNtStatusToDosError, OBJ_CASE_INSENSITIVE, UNICODE_STRING},
            Storage::FileSystem::{
                DELETE, FILE_ATTRIBUTE_DIRECTORY, FILE_LIST_DIRECTORY, FILE_READ_ATTRIBUTES,
                FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_TRAVERSE, SYNCHRONIZE,
            },
            System::IO::IO_STATUS_BLOCK,
        },
    };
    let mut name: Vec<u16> = path.as_os_str().encode_wide().collect();
    // canonicalize() supplies a verbatim DOS/UNC path. Convert its prefix to the NT namespace.
    if !name.starts_with(&[92, 92, 63, 92]) {
        return Err(io_error("Destination is not a canonical Windows path."));
    }
    name[1] = 63;
    let length = u16::try_from(
        name.len()
            .checked_mul(2)
            .ok_or_else(|| io_error("Destination path is too long."))?,
    )
    .map_err(io_error)?;
    let unicode = UNICODE_STRING {
        Length: length,
        MaximumLength: length,
        Buffer: name.as_mut_ptr(),
    };
    let attributes = OBJECT_ATTRIBUTES {
        Length: std::mem::size_of::<OBJECT_ATTRIBUTES>() as u32,
        ObjectName: &unicode,
        Attributes: OBJ_CASE_INSENSITIVE,
        ..Default::default()
    };
    let mut handle = std::ptr::null_mut();
    let mut status = IO_STATUS_BLOCK::default();
    // FILE_CREATE rejects existing objects. The returned handle already excludes delete sharing.
    // All input buffers outlive this synchronous call; successful HANDLE ownership moves to File.
    let result = unsafe {
        NtCreateFile(
            &mut handle,
            DELETE | FILE_READ_ATTRIBUTES | FILE_LIST_DIRECTORY | FILE_TRAVERSE | SYNCHRONIZE,
            &attributes,
            &mut status,
            std::ptr::null(),
            FILE_ATTRIBUTE_DIRECTORY,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            FILE_CREATE,
            FILE_DIRECTORY_FILE | FILE_SYNCHRONOUS_IO_NONALERT,
            std::ptr::null(),
            0,
        )
    };
    if result < 0 {
        return Err(io_error(std::io::Error::from_raw_os_error(unsafe {
            RtlNtStatusToDosError(result)
        }
            as i32)));
    }
    Ok(unsafe { File::from_raw_handle(handle) })
}
#[cfg(not(windows))]
fn create_directory_handle(_path: &Path) -> EngineResult<File> {
    Err(io_error("Page image export currently requires Windows."))
}

#[cfg(windows)]
fn delete_by_handle(file: &File) -> EngineResult<()> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FileDispositionInfo, SetFileInformationByHandle, FILE_DISPOSITION_INFO,
    };
    let info = FILE_DISPOSITION_INFO { DeleteFile: true };
    // The live handle was opened with DELETE access and still identifies the created object.
    let result = unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle(),
            FileDispositionInfo,
            &info as *const _ as *const _,
            std::mem::size_of_val(&info) as u32,
        )
    };
    if result == 0 {
        Err(io_error(std::io::Error::last_os_error()))
    } else {
        Ok(())
    }
}
#[cfg(not(windows))]
fn delete_by_handle(_file: &File) -> EngineResult<()> {
    Err(io_error(
        "Handle-bound image export cleanup is unavailable on this platform.",
    ))
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};
    type Hook = Box<dyn FnMut(&str, &Path) -> EngineResult<()>>;
    thread_local! {static HOOK:RefCell<Option<Hook>>=RefCell::new(None);}
    pub(super) fn hook(stage: &str, path: &Path) -> EngineResult<()> {
        HOOK.with(|h| match h.borrow_mut().as_mut() {
            Some(f) => f(stage, path),
            None => Ok(()),
        })
    }
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            HOOK.with(|h| *h.borrow_mut() = None);
        }
    }
    fn inject(f: impl FnMut(&str, &Path) -> EngineResult<()> + 'static) -> Reset {
        HOOK.with(|h| *h.borrow_mut() = Some(Box::new(f)));
        Reset
    }

    #[test]
    fn replacement_after_identity_check_is_never_deleted() {
        let parent = tempfile::tempdir().unwrap();
        let mut folder = OwnedImageFolder::create(parent.path()).unwrap();
        folder.write("page-0001.png", b"owned").unwrap();
        let path = folder.path.join("page-0001.png");
        let _reset = inject(|stage, path| {
            if stage == "checked_file" && path.file_name().unwrap() == "page-0001.png" {
                fs::rename(path, path.with_extension("moved")).unwrap();
                fs::write(path, b"unrelated sentinel").unwrap();
            }
            Ok(())
        });
        assert!(folder.remove_owned(1).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"unrelated sentinel");
        assert!(
            !path.with_extension("moved").exists(),
            "only the retained original is deleted"
        );
    }
    #[test]
    fn late_unexpected_entry_keeps_incomplete_marker() {
        let parent = tempfile::tempdir().unwrap();
        let folder = OwnedImageFolder::create(parent.path()).unwrap();
        let path = folder.path.clone();
        let _reset = inject(|stage, path| {
            if stage == "remove_directory" {
                fs::write(path.join("unexpected.txt"), b"keep").unwrap();
            }
            Ok(())
        });
        assert!(folder.rollback().is_err());
        assert_eq!(fs::read(path.join("unexpected.txt")).unwrap(), b"keep");
        assert!(
            path.join(MARKER).is_file(),
            "failed cleanup must leave its marker"
        );
    }
    #[test]
    fn setup_failures_report_leftover_path_and_ownership_error() {
        for failure_stage in ["created", "opened"] {
            let parent = tempfile::tempdir().unwrap();
            let created = Rc::new(RefCell::new(PathBuf::new()));
            let capture = created.clone();
            let _reset = inject(move |stage, path| {
                if stage == failure_stage {
                    *capture.borrow_mut() = path.to_path_buf();
                    return Err(io_error("injected setup failure"));
                }
                Ok(())
            });
            let error = OwnedImageFolder::create(parent.path())
                .err()
                .unwrap()
                .to_string();
            assert!(
                error.contains(&created.borrow().display().to_string()),
                "{error}"
            );
            assert!(error.contains("ownership"), "{error}");
            assert!(created.borrow().is_dir());
        }
    }
    #[test]
    fn a_late_marker_replacement_is_preserved_and_reported() {
        let parent = tempfile::tempdir().unwrap();
        let folder = OwnedImageFolder::create(parent.path()).unwrap();
        let path = folder.path.clone();
        let _reset = inject(|stage, path| {
            if stage == "remove_directory" {
                fs::write(path.join(MARKER), b"unrelated marker").unwrap();
            }
            Ok(())
        });
        let error = folder.rollback_error(io_error("cancelled")).to_string();
        assert!(
            error.contains("Could not restore the incomplete marker"),
            "{error}"
        );
        assert!(error.contains(&path.display().to_string()), "{error}");
        assert_eq!(fs::read(path.join(MARKER)).unwrap(), b"unrelated marker");
    }
    #[test]
    fn directory_lock_excludes_handles_that_would_block_cleanup() {
        use std::os::windows::fs::OpenOptionsExt;
        let parent = tempfile::tempdir().unwrap();
        let folder = OwnedImageFolder::create(parent.path()).unwrap();
        let path = folder.path.clone();
        assert!(OpenOptions::new()
            .read(true)
            .share_mode(3)
            .custom_flags(0x02000000)
            .open(&path)
            .is_err());
        folder.rollback().unwrap();
        assert!(!path.exists());
    }
    #[test]
    fn creation_never_adopts_a_replacement_directory() {
        let parent = tempfile::tempdir().unwrap();
        let sentinel = parent.path().join("sentinel");
        fs::create_dir(&sentinel).unwrap();
        let sentinel_identity = same_file::Handle::from_path(&sentinel).unwrap();
        let swapped = Rc::new(RefCell::new(false));
        let capture = swapped.clone();
        let sentinel_path = sentinel.clone();
        let _reset = inject(move |stage, path| {
            if stage == "created" && fs::rename(path, path.with_extension("moved")).is_ok() {
                fs::rename(&sentinel_path, path).unwrap();
                *capture.borrow_mut() = true;
            }
            Ok(())
        });
        let folder = OwnedImageFolder::create(parent.path()).unwrap();
        folder.rollback().unwrap();
        assert!(
            !*swapped.borrow(),
            "exclusive creation must already hold replacement exclusion"
        );
        assert_eq!(
            same_file::Handle::from_path(&sentinel).unwrap(),
            sentinel_identity
        );
    }
    #[test]
    fn missing_tracked_marker_is_restored_after_failed_rollback() {
        let parent = tempfile::tempdir().unwrap();
        let mut folder = OwnedImageFolder::create(parent.path()).unwrap();
        let path = folder.path.clone();
        folder.write("page-0001.png", b"image").unwrap();
        fs::rename(path.join(MARKER), parent.path().join("moved-marker")).unwrap();
        assert!(folder.rollback().is_err());
        assert_eq!(fs::read(path.join(MARKER)).unwrap(), MARKER_TEXT);
    }
    #[test]
    fn exclusive_native_creation_rejects_existing_directories() {
        let parent = tempfile::tempdir().unwrap();
        let existing = parent.path().canonicalize().unwrap().join("existing");
        fs::create_dir(&existing).unwrap();
        fs::write(existing.join("keep.txt"), b"sentinel").unwrap();
        assert!(create_directory_handle(&existing).is_err());
        assert_eq!(fs::read(existing.join("keep.txt")).unwrap(), b"sentinel");
    }
    #[test]
    fn unicode_destination_publishes_and_releases_the_directory_lock() {
        let parent = tempfile::tempdir().unwrap();
        let destination = parent.path().join("Images café 日本語");
        fs::create_dir(&destination).unwrap();
        let mut folder = OwnedImageFolder::create(&destination).unwrap();
        folder.write("page-0001.png", b"image").unwrap();
        let path = folder.finish().unwrap();
        drop(folder);
        assert_eq!(fs::read(path.join("page-0001.png")).unwrap(), b"image");
        assert!(!path.join(MARKER).exists());
        let moved = destination.join("published");
        fs::rename(path, moved).unwrap();
    }
    #[test]
    fn delete_pending_marker_is_closed_before_restoration() {
        let parent = tempfile::tempdir().unwrap();
        let folder = OwnedImageFolder::create(parent.path()).unwrap();
        let path = folder.path.clone();
        fs::remove_file(path.join(MARKER)).unwrap();
        assert!(folder.rollback().is_err());
        assert_eq!(fs::read(path.join(MARKER)).unwrap(), MARKER_TEXT);
    }
}
