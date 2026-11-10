//! Transient, explicitly requested security for a newly exported PDF copy.
use super::{load,save,validate_loaded,DocumentPermissions,ProtectionError,Result};
use lopdf::{dictionary,encryption::{crypt_filters::{Aes256CryptFilter,CryptFilter},EncryptionState,EncryptionVersion,Permissions},Object};
use std::{collections::BTreeMap,fmt,sync::Arc};

#[derive(serde::Deserialize)]
#[serde(tag="mode",rename_all="camelCase",deny_unknown_fields)]
pub enum ProtectionChange {
    Remove {},
    Passwords {
        #[serde(rename="userPassword")] user_password:String,
        #[serde(rename="ownerPassword")] owner_password:String,
        permissions:DocumentPermissions,
    },
}
impl fmt::Debug for ProtectionChange {
    fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result{match self{Self::Remove{}=>f.write_str("Remove"),Self::Passwords{permissions,..}=>f.debug_struct("Passwords").field("permissions",permissions).finish_non_exhaustive()}}
}
fn normalized(password:&str)->Result<String>{
    if password.len()>1024{return Err(ProtectionError::Limit("password exceeds 1,024 UTF-8 bytes"));}
    let value=stringprep::saslprep(password).map_err(|_|ProtectionError::Invalid("password contains unsupported Unicode characters"))?;
    if value.len()>127{return Err(ProtectionError::Limit("normalized password exceeds 127 UTF-8 bytes"));}
    Ok(value.into_owned())
}
impl ProtectionChange {
    pub fn validate(&self)->Result<()> {
        if let Self::Passwords{user_password,owner_password,permissions:p}=self {
            let user=normalized(user_password)?;let owner=normalized(owner_password)?;
            if owner.is_empty(){return Err(ProtectionError::Invalid("an owner password is required"));}
            if user==owner{return Err(ProtectionError::Invalid("opening and owner passwords must differ"));}
            if (!p.print&&p.print_high_quality)||(p.annotate&&!p.fill_forms){return Err(ProtectionError::Invalid("inconsistent PDF permission settings"));}
        }
        Ok(())
    }
    pub(crate) fn apply(&self,bytes:&[u8])->Result<Vec<u8>> {
        self.validate()?;
        let Self::Passwords{user_password,owner_password,permissions:p}=self else{return Ok(bytes.to_vec())};
        let user=normalized(user_password)?;let owner=normalized(owner_password)?;
        let mut doc=load(bytes,None,true)?;
        if doc.trailer.has(b"Encrypt")||doc.encryption_state.is_some(){return Err(ProtectionError::Invalid("expected an unencrypted editing document"));}
        validate_loaded(&doc,None)?;
        let mut key=[0u8;32];getrandom::fill(&mut key).map_err(|_|ProtectionError::Invalid("secure randomness is unavailable"))?;
        let mut permissions=Permissions::COPYABLE_FOR_ACCESSIBILITY;
        for (enabled,bit) in [(p.print,Permissions::PRINTABLE),(p.print_high_quality,Permissions::PRINTABLE_IN_HIGH_QUALITY),(p.copy_content,Permissions::COPYABLE),(p.edit_content,Permissions::MODIFIABLE),(p.annotate,Permissions::ANNOTABLE),(p.fill_forms,Permissions::FILLABLE),(p.assemble,Permissions::ASSEMBLABLE)]{if enabled{permissions|=bit;}}
        let filter=b"StdCF".to_vec();let filters=BTreeMap::from([(filter.clone(),Arc::new(Aes256CryptFilter) as Arc<dyn CryptFilter>)]);
        let state=EncryptionState::try_from(EncryptionVersion::V5{encrypt_metadata:true,crypt_filters:filters,file_encryption_key:&key,stream_filter:filter.clone(),string_filter:filter,owner_password:&owner,user_password:&user,permissions}).map_err(|_|ProtectionError::Invalid("could not create password protection"))?;
        if doc.version.as_str()<"1.7"{doc.version="1.7".into();}
        let root=doc.trailer.get(b"Root").and_then(Object::as_reference).map_err(|_|ProtectionError::Invalid("missing catalog"))?;
        let catalog=doc.get_dictionary(root).map_err(|_|ProtectionError::Invalid("invalid catalog"))?;
        let mut extensions=match catalog.get(b"Extensions"){Ok(value)=>doc.dereference(value).and_then(|(_,v)|v.as_dict()).cloned().map_err(|_|ProtectionError::Invalid("invalid extensions dictionary"))?,Err(_)=>lopdf::Dictionary::new()};
        let level=extensions.get(b"ADBE").ok().and_then(|v|doc.dereference(v).ok()).and_then(|(_,v)|v.as_dict().ok()).and_then(|v|v.get(b"ExtensionLevel").ok()).and_then(|v|v.as_i64().ok()).unwrap_or(0).max(8);
        extensions.set("ADBE",dictionary!{"BaseVersion"=>"1.7","ExtensionLevel"=>level});doc.get_dictionary_mut(root).map_err(|_|ProtectionError::Invalid("invalid catalog"))?.set("Extensions",extensions);
        doc.trailer.set("ID",vec![Object::String(uuid::Uuid::new_v4().as_bytes().to_vec(),lopdf::StringFormat::Hexadecimal),Object::String(uuid::Uuid::new_v4().as_bytes().to_vec(),lopdf::StringFormat::Hexadecimal)]);
        doc.encrypt(&state).map_err(|_|ProtectionError::Invalid("could not encrypt the PDF copy"))?;
        save(&mut doc)
    }
}
