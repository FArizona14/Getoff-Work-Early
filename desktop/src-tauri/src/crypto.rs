use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand::RngCore;
use std::{fs, path::Path};
use windows_sys::Win32::{
    Foundation::LocalFree,
    Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    },
};

pub struct Crypto(Aes256Gcm);
impl Crypto {
    pub fn open(dir: &Path, existing_data: bool) -> Result<Self, String> {
        let path = dir.join("user-key.dpapi");
        let key = if path.exists() {
            protect(&fs::read(path).map_err(|_| "密钥读取失败")?, false)?
        } else {
            if existing_data {
                return Err("密钥缺失，已停止写入；请恢复原账户密钥".into());
            }
            let mut key = [0u8; 32];
            rand::thread_rng().fill_bytes(&mut key);
            let wrapped = protect(&key, true)?;
            fs::write(&path, wrapped).map_err(|_| "密钥保存失败")?;
            key.to_vec()
        };
        Ok(Self(
            Aes256Gcm::new_from_slice(&key).map_err(|_| "密钥格式错误")?,
        ))
    }
    pub fn encrypt(&self, text: &str) -> Result<Vec<u8>, String> {
        let mut nonce = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce);
        let mut result = vec![1];
        result.extend(nonce);
        result.extend(
            self.0
                .encrypt(Nonce::from_slice(&nonce), text.as_bytes())
                .map_err(|_| "加密失败")?,
        );
        Ok(result)
    }
    pub fn decrypt(&self, data: &[u8]) -> Result<String, String> {
        if data.len() < 29 || data[0] != 1 {
            return Err("密文格式错误".into());
        }
        String::from_utf8(
            self.0
                .decrypt(Nonce::from_slice(&data[1..13]), &data[13..])
                .map_err(|_| "数据解密校验失败")?,
        )
        .map_err(|_| "文本编码错误".into())
    }
}
fn protect(bytes: &[u8], encrypt: bool) -> Result<Vec<u8>, String> {
    unsafe {
        let input = CRYPT_INTEGER_BLOB {
            cbData: bytes.len() as u32,
            pbData: bytes.as_ptr() as *mut u8,
        };
        let mut output: CRYPT_INTEGER_BLOB = std::mem::zeroed();
        let ok = if encrypt {
            CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        if ok == 0 {
            return Err("Windows 账户密钥保护失败".into());
        }
        let result = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(output.pbData as *mut _);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authenticated_encryption() {
        let c = Crypto(Aes256Gcm::new_from_slice(&[7; 32]).unwrap());
        let a = c.encrypt("机密任务").unwrap();
        let b = c.encrypt("机密任务").unwrap();
        assert_ne!(a, b);
        assert_eq!(c.decrypt(&a).unwrap(), "机密任务");
        let mut broken = a;
        broken[14] ^= 1;
        assert!(c.decrypt(&broken).is_err());
    }
    #[test]
    fn dpapi_current_user_round_trip() {
        let data = protect(&[9; 32], true).unwrap();
        assert_eq!(protect(&data, false).unwrap(), vec![9; 32]);
    }
}
