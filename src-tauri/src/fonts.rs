use crate::{EngineError, EngineResult};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

const MAX_FONT_BYTES: usize = 16 * 1024 * 1024;
const MAX_TOTAL_BYTES: usize = 128 * 1024 * 1024;
const MAX_FONTS: usize = 64;

fn invalid(message: impl Into<String>) -> EngineError {
    EngineError::InvalidRequest(message.into())
}

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

// Deliberately unshaped BMP text. Combining marks, bidi controls, joiners,
// variation selectors and scripts requiring shaping are excluded even when a
// font provides their glyphs. Newlines are handled separately as line breaks.
fn allowed_character(character: char) -> bool {
    matches!(character as u32,
        0x20..=0x7e | 0xa0..=0xac | 0xae..=0x2ff | 0x370..=0x482 | 0x48a..=0x52f |
        0x1d00..=0x1dbf | 0x1e00..=0x1fff | 0x2000..=0x200a |
        0x2010..=0x2027 | 0x202f..=0x205e | 0x2070..=0x20cf |
        0x2100..=0x2bff | 0x2c60..=0x2c7f | 0xa640..=0xa66e |
        0xa673 | 0xa67e..=0xa69d | 0xa720..=0xa7ff | 0xab30..=0xab6f)
}

fn checked_face(bytes: &[u8]) -> EngineResult<ttf_parser::Face<'_>> {
    if bytes.len() > MAX_FONT_BYTES {
        return Err(invalid("A font file cannot exceed 16 MiB."));
    }
    if bytes.starts_with(b"ttcf") {
        return Err(invalid("Font collections (TTC/OTC) are not supported. Choose a standalone TTF or TrueType-outline OTF font."));
    }
    let raw = ttf_parser::RawFace::parse(bytes, 0)
        .map_err(|_| invalid("This file is not a valid standalone TrueType or OpenType font."))?;
    let table = |tag| raw.table(ttf_parser::Tag::from_bytes(tag));
    if table(b"CFF ").is_some() || table(b"CFF2").is_some() {
        return Err(invalid(
            "CFF-outline fonts are not supported. Choose a TrueType-outline font.",
        ));
    }
    if table(b"fvar").is_some() || table(b"gvar").is_some() {
        return Err(invalid(
            "Variable fonts are not supported. Choose a static font face.",
        ));
    }
    if [b"COLR", b"CPAL", b"CBDT", b"CBLC", b"sbix", b"SVG "]
        .iter()
        .any(|tag| table(*tag).is_some())
    {
        return Err(invalid(
            "Color fonts are not supported. Choose a static monochrome TrueType font.",
        ));
    }
    let face = ttf_parser::Face::parse(bytes, 0)
        .map_err(|_| invalid("The font has malformed or missing required tables."))?;
    if face.tables().glyf.is_none() || face.tables().hmtx.is_none() || face.tables().cmap.is_none()
    {
        return Err(invalid("The font must contain valid TrueType outlines, character mappings, and horizontal metrics."));
    }
    // Interpret every permission bit conservatively, including old OS/2
    // versions whose parsers sometimes ignore bitmap-only/no-subsetting bits.
    let os2 = table(b"OS/2")
        .filter(|table| table.len() >= 10)
        .ok_or_else(|| invalid("Font embedding permissions could not be verified."))?;
    let flags = u16::from_be_bytes([os2[8], os2[9]]);
    if flags & 0x0002 != 0 {
        return Err(invalid(
            "This font prohibits embedding (restricted license permissions).",
        ));
    }
    if flags & 0x0004 != 0 {
        return Err(invalid(
            "This font permits preview/print embedding only; editable embedding is required.",
        ));
    }
    if flags & 0x0200 != 0 {
        return Err(invalid(
            "This font permits bitmap-only embedding; outline embedding is required.",
        ));
    }
    if flags & !0x030e != 0 || face.tables().os2.is_none() {
        return Err(invalid(
            "Font embedding permissions are malformed or unsupported.",
        ));
    }
    // Installable/editable faces with no-subsetting are accepted because the
    // complete immutable program is retained and embedded by the PDF writer.
    Ok(face)
}

fn display_name(face: &ttf_parser::Face<'_>) -> String {
    for english in [true, false] {
        for name in face.names() {
            if name.name_id == ttf_parser::name_id::FULL_NAME
                && (!english || name.language_id == 0x0409)
            {
                if let Some(value) = name.to_string().filter(|value| !value.trim().is_empty()) {
                    return value;
                }
            }
        }
    }
    "Imported font".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FontInfo {
    pub id: String,
    pub name: String,
    pub weight: u16,
    pub italic: bool,
    pub coverage: Vec<[u32; 2]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledFont {
    pub id: String,
    pub name: String,
    pub supported: bool,
    pub reason: Option<String>,
}

#[derive(Debug)]
pub struct FontAsset {
    pub info: FontInfo,
    pub bytes: Arc<[u8]>,
}

impl FontAsset {
    pub fn parse(bytes: Vec<u8>) -> EngineResult<Self> {
        let face = checked_face(&bytes)?;
        let mut coverage: Vec<[u32; 2]> = Vec::new();
        for codepoint in 0x20..=0xab6f {
            let Some(character) = char::from_u32(codepoint) else {
                continue;
            };
            if !allowed_character(character)
                || face
                    .glyph_index(character)
                    .is_none_or(|glyph| glyph.0 == 0 || face.glyph_hor_advance(glyph).is_none())
            {
                continue;
            }
            match coverage.last_mut() {
                Some(range) if range[1] + 1 == codepoint => range[1] = codepoint,
                _ => coverage.push([codepoint, codepoint]),
            }
        }
        if coverage.is_empty() {
            return Err(invalid("This font has no supported Latin, Greek, Cyrillic, punctuation, or symbol characters."));
        }
        let info = FontInfo {
            id: digest(&bytes),
            name: display_name(&face),
            weight: face.weight().to_number(),
            italic: face.is_italic() || face.is_oblique(),
            coverage,
        };
        Ok(Self {
            info,
            bytes: bytes.into(),
        })
    }

    pub fn face(&self) -> EngineResult<ttf_parser::Face<'_>> {
        ttf_parser::Face::parse(&self.bytes, 0)
            .map_err(|_| invalid("The registered font could not be parsed."))
    }

    pub fn validate_text(&self, text: &str) -> EngineResult<()> {
        let face = self.face()?;
        for character in text.chars().filter(|&character| character != '\n') {
            self.checked_glyph(&face, character)?;
        }
        Ok(())
    }

    fn checked_glyph(
        &self,
        face: &ttf_parser::Face<'_>,
        character: char,
    ) -> EngineResult<ttf_parser::GlyphId> {
        if !allowed_character(character) {
            return Err(invalid(format!("Character U+{:04X} requires unsupported shaping or is a control/combining character. Use unshaped BMP Latin, Greek, Cyrillic, punctuation, or symbols.", character as u32)));
        }
        face.glyph_index(character)
            .filter(|glyph| glyph.0 != 0 && face.glyph_hor_advance(*glyph).is_some())
            .ok_or_else(|| {
                invalid(format!(
                    "{} has no glyph for U+{:04X} ({character}).",
                    self.info.name, character as u32
                ))
            })
    }

    pub fn glyph_id(&self, character: char) -> EngineResult<u16> {
        Ok(self.checked_glyph(&self.face()?, character)?.0)
    }

    pub fn advance(&self, character: char) -> EngineResult<f32> {
        let face = self.face()?;
        let glyph = self.checked_glyph(&face, character)?;
        Ok(face
            .glyph_hor_advance(glyph)
            .ok_or_else(|| invalid("The glyph has no horizontal metrics."))? as f32
            / face.units_per_em() as f32)
    }
}

#[derive(Debug, Default)]
pub struct FontRegistry {
    state: Mutex<RegistryState>,
}

#[derive(Debug, Default)]
struct RegistryState {
    assets: BTreeMap<String, Arc<FontAsset>>,
    bytes: usize,
    catalog: Option<BTreeMap<String, CatalogEntry>>,
}

#[derive(Debug)]
struct CatalogEntry {
    path: PathBuf,
    info: InstalledFont,
}

impl FontRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> EngineResult<MutexGuard<'_, RegistryState>> {
        self.state
            .lock()
            .map_err(|_| invalid("The font registry is unavailable."))
    }

    pub fn register(&self, bytes: Vec<u8>) -> EngineResult<FontInfo> {
        if bytes.len() > MAX_FONT_BYTES {
            return Err(invalid("A font file cannot exceed 16 MiB."));
        }
        let id = digest(&bytes);
        if let Some(asset) = self.lock()?.assets.get(&id) {
            return Ok(asset.info.clone());
        }
        let asset = Arc::new(FontAsset::parse(bytes)?);
        self.register_assets(std::slice::from_ref(&asset))?;
        Ok(asset.info.clone())
    }

    pub fn get(&self, id: &str) -> EngineResult<Arc<FontAsset>> {
        self.lock()?
            .assets
            .get(id)
            .cloned()
            .ok_or_else(|| invalid("The requested font is not loaded. Select or import it again."))
    }

    pub fn ids(&self) -> EngineResult<BTreeSet<String>> {
        Ok(self.lock()?.assets.keys().cloned().collect())
    }

    pub fn remove(&self, id: &str) -> EngineResult<()> {
        let mut state = self.lock()?;
        if let Some(asset) = state.assets.remove(id) {
            state.bytes -= asset.bytes.len();
        }
        Ok(())
    }

    // Caller stages only parsed, verified assets. Check the entire batch before
    // inserting anything so failed PDF imports cannot leave partial resources.
    pub(crate) fn register_assets(&self, assets: &[Arc<FontAsset>]) -> EngineResult<()> {
        let mut state = self.lock()?;
        let added: BTreeMap<_, _> = assets
            .iter()
            .filter(|asset| !state.assets.contains_key(&asset.info.id))
            .map(|asset| (asset.info.id.clone(), asset.clone()))
            .collect();
        if state.assets.len() + added.len() > MAX_FONTS {
            return Err(invalid(
                "At most 64 fonts can be loaded at once. Close documents using other fonts first.",
            ));
        }
        let bytes = added.values().try_fold(state.bytes, |total, asset| {
            if asset.bytes.len() > MAX_FONT_BYTES { return Err(invalid("A font file cannot exceed 16 MiB.")); }
            total.checked_add(asset.bytes.len()).filter(|&total| total <= MAX_TOTAL_BYTES)
                .ok_or_else(|| invalid("Loaded font resources cannot exceed 128 MiB. Close documents using other fonts first."))
        })?;
        state.assets.extend(added);
        state.bytes = bytes;
        Ok(())
    }

    pub fn list_installed(&self) -> EngineResult<Vec<InstalledFont>> {
        if self.lock()?.catalog.is_none() {
            let catalog = discover_fonts();
            self.lock()?.catalog.get_or_insert(catalog);
        }
        let mut result: Vec<_> = self
            .lock()?
            .catalog
            .as_ref()
            .unwrap()
            .values()
            .map(|entry| entry.info.clone())
            .collect();
        result.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then(a.id.cmp(&b.id))
        });
        Ok(result)
    }

    pub fn load_installed(&self, id: &str) -> EngineResult<FontInfo> {
        self.list_installed()?;
        let path = self
            .lock()?
            .catalog
            .as_ref()
            .unwrap()
            .get(id)
            .map(|entry| entry.path.clone())
            .ok_or_else(|| {
                invalid("The selected installed font was not found in the font catalog.")
            })?;
        self.register(read_font(&path)?)
    }
}

fn read_font(path: &Path) -> EngineResult<Vec<u8>> {
    let file = std::fs::File::open(path).map_err(|error| EngineError::Io(error.to_string()))?;
    if file
        .metadata()
        .map_err(|error| EngineError::Io(error.to_string()))?
        .len()
        > MAX_FONT_BYTES as u64
    {
        return Err(invalid("A font file cannot exceed 16 MiB."));
    }
    let mut bytes = Vec::new();
    file.take(MAX_FONT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| EngineError::Io(error.to_string()))?;
    if bytes.len() > MAX_FONT_BYTES {
        return Err(invalid("A font file cannot exceed 16 MiB."));
    }
    Ok(bytes)
}

fn discover_fonts() -> BTreeMap<String, CatalogEntry> {
    let windows = PathBuf::from(std::env::var_os("WINDIR").unwrap_or_else(|| "C:/Windows".into()));
    let system_fonts = windows.join("Fonts");
    let mut paths = BTreeSet::new();
    let mut directories = vec![system_fonts.clone()];
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        directories.push(PathBuf::from(local).join("Microsoft/Windows/Fonts"));
    }
    for directory in directories {
        if let Ok(entries) = std::fs::read_dir(directory) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file()
                    && path.extension().is_some_and(|extension| {
                        matches!(
                            extension.to_string_lossy().to_ascii_lowercase().as_str(),
                            "ttf" | "otf" | "ttc" | "otc" | "fon" | "fnt"
                        )
                    })
                {
                    paths.insert(path);
                }
            }
        }
    }
    #[cfg(windows)]
    registered_font_paths(&system_fonts, &mut paths);
    paths
        .into_iter()
        .map(|path| {
            let id = format!("installed-{}", digest(path.to_string_lossy().as_bytes()));
            let inspected = read_font(&path)
                .and_then(|bytes| checked_face(&bytes).map(|face| display_name(&face)));
            let (name, reason) = match inspected {
                Ok(name) => (name, None),
                Err(error) => (
                    path.file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    Some(error.to_string()),
                ),
            };
            let info = InstalledFont {
                id: id.clone(),
                name,
                supported: reason.is_none(),
                reason,
            };
            (id, CatalogEntry { path, info })
        })
        .collect()
}

#[cfg(windows)]
fn registered_font_paths(system_fonts: &Path, paths: &mut BTreeSet<PathBuf>) {
    use windows_sys::Win32::System::Registry::*;
    let key_name: Vec<u16> = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Fonts\0"
        .encode_utf16()
        .collect();
    for hive in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
        let mut key = std::ptr::null_mut();
        // Values are read only. Buffers have explicit byte/code-unit bounds;
        // oversized or inaccessible entries are skipped, never interpreted.
        if unsafe { RegOpenKeyExW(hive, key_name.as_ptr(), 0, KEY_READ, &mut key) } != 0 {
            continue;
        }
        for index in 0..16384 {
            let mut name = [0_u16; 16384];
            let mut name_len = name.len() as u32;
            let mut data = [0_u16; 32768];
            let mut data_len = (data.len() * 2) as u32;
            let mut kind = 0;
            let result = unsafe {
                RegEnumValueW(
                    key,
                    index,
                    name.as_mut_ptr(),
                    &mut name_len,
                    std::ptr::null(),
                    &mut kind,
                    data.as_mut_ptr().cast(),
                    &mut data_len,
                )
            };
            if result == windows_sys::Win32::Foundation::ERROR_NO_MORE_ITEMS {
                break;
            }
            if result != 0
                || !matches!(kind, REG_SZ | REG_EXPAND_SZ)
                || data_len as usize > data.len() * 2
                || data_len % 2 != 0
            {
                continue;
            }
            let value = String::from_utf16_lossy(&data[..data_len as usize / 2]);
            let mut value = value.trim_end_matches('\0').to_string();
            if kind == REG_EXPAND_SZ {
                for (name, replacement) in std::env::vars_os() {
                    let token = format!("%{}%", name.to_string_lossy());
                    if let Some(index) = value
                        .as_bytes()
                        .windows(token.len())
                        .position(|part| part.eq_ignore_ascii_case(token.as_bytes()))
                    {
                        value.replace_range(
                            index..index + token.len(),
                            &replacement.to_string_lossy(),
                        );
                        // Environment values are not recursively expanded.
                    }
                }
            }
            if value.is_empty() {
                continue;
            }
            let path = PathBuf::from(value);
            paths.insert(if path.is_absolute() {
                path
            } else {
                system_fonts.join(path)
            });
        }
        unsafe {
            RegCloseKey(key);
        }
    }
}
