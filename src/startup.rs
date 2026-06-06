use std::{env, mem::size_of};

use windows::{
    Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, WIN32_ERROR},
        System::Registry::{
            HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SAM_FLAGS, REG_SZ,
            REG_VALUE_TYPE, RegCloseKey, RegCreateKeyW, RegDeleteValueW, RegOpenKeyExW,
            RegQueryValueExW, RegSetValueExW,
        },
    },
    core::{Error, HRESULT, PCWSTR, Result, w},
};

const RUN_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const RUN_VALUE_NAME: PCWSTR = w!("desktop-switcher");

pub fn is_enabled() -> Result<bool> {
    let key = match RegistryKey::open(KEY_QUERY_VALUE) {
        Ok(key) => key,
        Err(error) if error.code() == HRESULT::from_win32(ERROR_FILE_NOT_FOUND.0) => {
            return Ok(false);
        }
        Err(error) => return Err(error),
    };

    let mut value_type = REG_VALUE_TYPE::default();
    let mut byte_len = 0u32;
    let result = unsafe {
        RegQueryValueExW(
            key.handle,
            RUN_VALUE_NAME,
            None,
            Some(&mut value_type),
            None,
            Some(&mut byte_len),
        )
    };

    if result == ERROR_FILE_NOT_FOUND {
        return Ok(false);
    }
    win32_result(result)?;

    if value_type != REG_SZ || byte_len == 0 {
        return Ok(false);
    }

    let mut bytes = vec![0u8; byte_len as usize];
    let result = unsafe {
        RegQueryValueExW(
            key.handle,
            RUN_VALUE_NAME,
            None,
            Some(&mut value_type),
            Some(bytes.as_mut_ptr()),
            Some(&mut byte_len),
        )
    };
    win32_result(result)?;

    if value_type != REG_SZ {
        return Ok(false);
    }

    let value = decode_reg_sz(&bytes[..byte_len as usize]);
    Ok(value == startup_command()?)
}

pub fn set_enabled(enabled: bool) -> Result<()> {
    if enabled { enable() } else { disable() }
}

fn enable() -> Result<()> {
    let key = RegistryKey::create()?;
    let encoded = encode_reg_sz(&startup_command()?);
    let bytes = wide_as_bytes(&encoded);
    let result = unsafe { RegSetValueExW(key.handle, RUN_VALUE_NAME, None, REG_SZ, Some(bytes)) };

    win32_result(result)
}

fn disable() -> Result<()> {
    let key = match RegistryKey::open(KEY_SET_VALUE) {
        Ok(key) => key,
        Err(error) if error.code() == HRESULT::from_win32(ERROR_FILE_NOT_FOUND.0) => {
            return Ok(());
        }
        Err(error) => return Err(error),
    };

    let result = unsafe { RegDeleteValueW(key.handle, RUN_VALUE_NAME) };
    if result == ERROR_FILE_NOT_FOUND {
        return Ok(());
    }

    win32_result(result)
}

fn startup_command() -> Result<String> {
    let exe = env::current_exe().map_err(|error| {
        Error::new(
            HRESULT::from_win32(error.raw_os_error().unwrap_or(1) as u32),
            format!("无法获取当前程序路径: {error}"),
        )
    })?;

    Ok(format!("\"{}\"", exe.display()))
}

fn encode_reg_sz(value: &str) -> Vec<u16> {
    value.encode_utf16().chain([0]).collect()
}

fn decode_reg_sz(bytes: &[u8]) -> String {
    let mut wide = Vec::with_capacity(bytes.len() / size_of::<u16>());
    for chunk in bytes.chunks_exact(size_of::<u16>()) {
        wide.push(u16::from_le_bytes([chunk[0], chunk[1]]));
    }

    while wide.last() == Some(&0) {
        wide.pop();
    }

    String::from_utf16_lossy(&wide)
}

fn wide_as_bytes(value: &[u16]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(value.as_ptr().cast::<u8>(), std::mem::size_of_val(value)) }
}

fn win32_result(result: WIN32_ERROR) -> Result<()> {
    if result == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(Error::from_hresult(HRESULT::from_win32(result.0)))
    }
}

struct RegistryKey {
    handle: HKEY,
}

impl RegistryKey {
    fn open(sam_desired: REG_SAM_FLAGS) -> Result<Self> {
        let mut handle = HKEY::default();
        let result =
            unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, RUN_KEY, None, sam_desired, &mut handle) };
        win32_result(result)?;

        Ok(Self { handle })
    }

    fn create() -> Result<Self> {
        let mut handle = HKEY::default();
        let result = unsafe { RegCreateKeyW(HKEY_CURRENT_USER, RUN_KEY, &mut handle) };
        win32_result(result)?;

        Ok(Self { handle })
    }
}

impl Drop for RegistryKey {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.handle);
        }
    }
}
