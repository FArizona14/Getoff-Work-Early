use aes_gcm::{aead::{Aead, KeyInit}, Aes256Gcm, Nonce};
use argon2::Argon2;
use rand::RngCore;
use std::{fs, path::{Path, PathBuf}};
use windows_sys::Win32::{Foundation::LocalFree, Security::Cryptography::{CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB}};

const LOCK_MAGIC: &[u8; 5] = b"EOWL2";

pub struct Crypto { key: Option<[u8;32]>, locked_envelope: Option<Vec<u8>>, path: PathBuf }
impl Crypto {
    pub fn open(dir: &Path, existing_data: bool) -> Result<Self,String> {
        let path=dir.join("user-key.dpapi");
        if !path.exists() {
            if existing_data { return Err("密钥缺失，已停止写入；请恢复原账户密钥".into()) }
            let mut key=[0u8;32]; rand::thread_rng().fill_bytes(&mut key); fs::write(&path,protect(&key,true)?).map_err(|_|"密钥保存失败")?;
            return Ok(Self{key:Some(key),locked_envelope:None,path});
        }
        let raw=protect(&fs::read(&path).map_err(|_|"密钥读取失败")?,false)?;
        if raw.starts_with(LOCK_MAGIC) { Ok(Self{key:None,locked_envelope:Some(raw),path}) }
        else if raw.len()==32 { let mut key=[0u8;32]; key.copy_from_slice(&raw); Ok(Self{key:Some(key),locked_envelope:None,path}) }
        else { Err("密钥格式错误".into()) }
    }
    pub fn is_locked(&self)->bool { self.key.is_none() }
    pub fn lock_enabled(&self)->bool { self.locked_envelope.is_some() }
    fn cipher(&self)->Result<Aes256Gcm,String>{ Aes256Gcm::new_from_slice(self.key.as_ref().ok_or("应用已锁定，请先解锁")?).map_err(|_|"密钥格式错误".into()) }
    pub fn encrypt(&self,text:&str)->Result<Vec<u8>,String>{ encrypt_with(&self.cipher()?,text.as_bytes()) }
    pub fn decrypt(&self,data:&[u8])->Result<String,String>{ String::from_utf8(decrypt_with(&self.cipher()?,data)?).map_err(|_|"文本编码错误".into()) }
    pub fn raw_key(&self)->Result<[u8;32],String>{ self.key.ok_or("应用已锁定，请先解锁".into()) }
    pub fn unlock(&mut self,password:&str)->Result<(),String>{
        if self.key.is_some(){return Ok(())} let env=self.locked_envelope.as_ref().ok_or("应用未启用锁定")?;
        if env.len()<5+16+29{return Err("应用锁数据损坏".into())} let salt=&env[5..21]; let mut derived=[0u8;32];
        Argon2::default().hash_password_into(password.as_bytes(),salt,&mut derived).map_err(|_|"密码校验失败")?;
        let plain=decrypt_with(&Aes256Gcm::new_from_slice(&derived).unwrap(),&env[21..]).map_err(|_|"密码不正确")?;
        if plain.len()!=32{return Err("应用锁数据损坏".into())} let mut key=[0u8;32]; key.copy_from_slice(&plain); self.key=Some(key); Ok(())
    }
    pub fn enable_lock(&mut self,password:&str)->Result<(),String>{
        if password.chars().count()<6{return Err("应用锁密码至少 6 位".into())} let key=self.raw_key()?; let mut salt=[0u8;16]; rand::thread_rng().fill_bytes(&mut salt); let mut derived=[0u8;32];
        Argon2::default().hash_password_into(password.as_bytes(),&salt,&mut derived).map_err(|_|"应用锁创建失败")?;
        let mut env=LOCK_MAGIC.to_vec(); env.extend(salt); env.extend(encrypt_with(&Aes256Gcm::new_from_slice(&derived).unwrap(),&key)?);
        fs::write(&self.path,protect(&env,true)?).map_err(|_|"应用锁保存失败")?; self.locked_envelope=Some(env); Ok(())
    }
    pub fn disable_lock(&mut self,password:&str)->Result<(),String>{ if self.key.is_none(){self.unlock(password)?} let key=self.raw_key()?; fs::write(&self.path,protect(&key,true)?).map_err(|_|"密钥保存失败")?; self.locked_envelope=None; Ok(()) }
    pub fn replace_key(&mut self,key:[u8;32])->Result<(),String>{ fs::write(&self.path,protect(&key,true)?).map_err(|_|"密钥保存失败")?; self.key=Some(key); self.locked_envelope=None; Ok(()) }
}
fn encrypt_with(cipher:&Aes256Gcm,bytes:&[u8])->Result<Vec<u8>,String>{ let mut nonce=[0u8;12]; rand::thread_rng().fill_bytes(&mut nonce); let mut out=vec![1]; out.extend(nonce); out.extend(cipher.encrypt(Nonce::from_slice(&nonce),bytes).map_err(|_|"加密失败")?); Ok(out) }
fn decrypt_with(cipher:&Aes256Gcm,data:&[u8])->Result<Vec<u8>,String>{ if data.len()<29||data[0]!=1{return Err("密文格式错误".into())} cipher.decrypt(Nonce::from_slice(&data[1..13]),&data[13..]).map_err(|_|"数据解密校验失败".into()) }
pub fn password_encrypt(password:&str,plain:&[u8])->Result<Vec<u8>,String>{ if password.chars().count()<8{return Err("备份密码至少 8 位".into())} let mut salt=[0u8;16];rand::thread_rng().fill_bytes(&mut salt);let mut key=[0u8;32];Argon2::default().hash_password_into(password.as_bytes(),&salt,&mut key).map_err(|_|"备份加密失败")?;let mut out=b"EOWBAK2\0".to_vec();out.extend(salt);out.extend(encrypt_with(&Aes256Gcm::new_from_slice(&key).unwrap(),plain)?);Ok(out)}
pub fn password_decrypt(password:&str,data:&[u8])->Result<Vec<u8>,String>{if data.len()<8+16+29||&data[..8]!=b"EOWBAK2\0"{return Err("备份格式不正确".into())}let mut key=[0u8;32];Argon2::default().hash_password_into(password.as_bytes(),&data[8..24],&mut key).map_err(|_|"备份密码校验失败")?;decrypt_with(&Aes256Gcm::new_from_slice(&key).unwrap(),&data[24..]).map_err(|_|"备份密码不正确或文件已损坏".into())}
fn protect(bytes:&[u8],encrypt:bool)->Result<Vec<u8>,String>{unsafe{let input=CRYPT_INTEGER_BLOB{cbData:bytes.len() as u32,pbData:bytes.as_ptr() as *mut u8};let mut output:CRYPT_INTEGER_BLOB=std::mem::zeroed();let ok=if encrypt{CryptProtectData(&input,std::ptr::null(),std::ptr::null(),std::ptr::null(),std::ptr::null(),CRYPTPROTECT_UI_FORBIDDEN,&mut output)}else{CryptUnprotectData(&input,std::ptr::null_mut(),std::ptr::null(),std::ptr::null(),std::ptr::null(),CRYPTPROTECT_UI_FORBIDDEN,&mut output)};if ok==0{return Err("Windows 账户密钥保护失败".into())}let result=std::slice::from_raw_parts(output.pbData,output.cbData as usize).to_vec();LocalFree(output.pbData as *mut _);Ok(result)}}

#[cfg(test)] mod tests{use super::*;#[test]fn authenticated_encryption(){let path=std::env::temp_dir().join(format!("eow-c-{}",rand::random::<u64>()));fs::create_dir_all(&path).unwrap();let mut c=Crypto::open(&path,false).unwrap();let a=c.encrypt("机密任务").unwrap();let b=c.encrypt("机密任务").unwrap();assert_ne!(a,b);assert_eq!(c.decrypt(&a).unwrap(),"机密任务");c.enable_lock("12345678").unwrap();drop(c);let mut c=Crypto::open(&path,true).unwrap();assert!(c.is_locked());assert!(c.unlock("badbad").is_err());c.unlock("12345678").unwrap();assert_eq!(c.decrypt(&a).unwrap(),"机密任务");fs::remove_dir_all(path).unwrap();}#[test]fn portable_roundtrip(){let p=password_encrypt("12345678",b"hello").unwrap();assert_eq!(password_decrypt("12345678",&p).unwrap(),b"hello");assert!(password_decrypt("87654321",&p).is_err());}}
