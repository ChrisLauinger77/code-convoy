#![allow(clippy::missing_safety_doc)]
#![allow(non_camel_case_types)]

use std::{
    ffi::{OsStr, OsString},
    iter::once,
    mem::size_of,
    os::windows::prelude::{OsStrExt, OsStringExt},
    ptr::NonNull,
};

use windows_sys::Win32::{
    Globalization::lstrcpynW,
    System::Memory::{GlobalAlloc, GlobalFree, GPTR},
};

static mut G_STRINGSIZE: u32 = 0;
static mut G_STACKTOP: *mut *mut stack_t = std::ptr::null_mut();

// CodeConvoy 2026-10-16: validate local pointer snapshots, allocation and UTF-16
// capacity; consume invalid-Unicode nodes too. See pluginapi-stack-safety.patch.
// NSIS invokes these helpers synchronously on one installer thread. The caller
// must provide a live stack slot and GlobalAlloc nodes with string_size UTF-16
// code units. Null slots and empty stacks are supported; arbitrary dangling
// host pointers cannot be validated here. Calls must not overlap.
pub unsafe fn exdll_init(string_size: u32, _variables: *mut wchar_t, stacktop: *mut *mut stack_t) {
    G_STRINGSIZE = string_size;
    G_STACKTOP = stacktop;
}

pub type wchar_t = u16;

#[derive(Debug)]
pub enum Error {
    InvalidStackError,
    InvalidUnicode,
}

#[repr(C)]
#[derive(Debug)]
pub struct stack_t {
    next: *mut stack_t,
    text: [wchar_t; 1],
}

pub unsafe fn pushstring(s: impl AsRef<OsStr>) {
    let Some(stacktop) = NonNull::new(G_STACKTOP) else {
        return;
    };
    let capacity = G_STRINGSIZE;
    if capacity == 0 || capacity > i32::MAX as u32 {
        return;
    }
    let Some(bytes) = (capacity as usize)
        .checked_mul(size_of::<wchar_t>())
        .and_then(|bytes| bytes.checked_add(size_of::<stack_t>()))
    else {
        return;
    };
    let Some(node) = NonNull::new(GlobalAlloc(GPTR, bytes) as *mut stack_t) else {
        return;
    };
    let string_wide = encode_wide(s);
    lstrcpynW(
        std::ptr::addr_of_mut!((*node.as_ptr()).text).cast(),
        string_wide.as_ptr(),
        capacity as i32,
    );
    (*node.as_ptr()).next = *stacktop.as_ptr();
    *stacktop.as_ptr() = node.as_ptr();
}

pub unsafe fn popstring() -> Result<String, Error> {
    let stacktop = NonNull::new(G_STACKTOP).ok_or(Error::InvalidStackError)?;
    let node = NonNull::new(*stacktop.as_ptr()).ok_or(Error::InvalidStackError)?;
    let capacity = G_STRINGSIZE;
    if capacity == 0 || capacity > i32::MAX as u32 {
        return Err(Error::InvalidStackError);
    }
    let mut string_wide = vec![0; capacity as usize];
    lstrcpynW(
        string_wide.as_mut_ptr(),
        std::ptr::addr_of!((*node.as_ptr()).text).cast(),
        capacity as i32,
    );
    *stacktop.as_ptr() = (*node.as_ptr()).next;
    GlobalFree(node.as_ptr() as _);
    decode_wide(&string_wide)
        .into_string()
        .map_err(|_| Error::InvalidUnicode)
}

pub unsafe fn pushint(int: i32) {
    pushstring(int.to_string())
}

pub fn encode_wide(string: impl AsRef<OsStr>) -> Vec<u16> {
    string.as_ref().encode_wide().chain(once(0)).collect()
}

pub fn decode_wide(mut wide_c_string: &[u16]) -> OsString {
    if let Some(null_pos) = wide_c_string.iter().position(|c| *c == 0) {
        wide_c_string = &wide_c_string[..null_pos];
    }

    OsString::from_wide(wide_c_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    // One test owns the upstream global API state; do not run calls in parallel.
    #[test]
    fn unicode_stack_bounds_ownership_and_invalid_inputs() {
        unsafe {
            assert_eq!(size_of::<wchar_t>(), 2);
            assert_eq!(
                std::mem::offset_of!(stack_t, text),
                size_of::<*mut stack_t>()
            );
            exdll_init(1024, std::ptr::null_mut(), std::ptr::null_mut());
            pushstring("ignored");
            assert!(matches!(popstring(), Err(Error::InvalidStackError)));
            let mut top = std::ptr::null_mut();
            exdll_init(1024, std::ptr::null_mut(), &mut top);
            assert!(matches!(popstring(), Err(Error::InvalidStackError)));
            pushstring("sentinel");
            pushstring("安装 ü 🚚");
            assert_eq!(popstring().unwrap(), "安装 ü 🚚");
            pushstring(OsString::from_wide(&[0xd800]));
            assert!(matches!(popstring(), Err(Error::InvalidUnicode)));
            assert_eq!(popstring().unwrap(), "sentinel");
            assert!(top.is_null());
            exdll_init(4, std::ptr::null_mut(), &mut top);
            pushstring("abcdef");
            assert_eq!(popstring().unwrap(), "abc");
            exdll_init(1, std::ptr::null_mut(), &mut top);
            pushstring("a");
            assert_eq!(popstring().unwrap(), "");
            exdll_init(0, std::ptr::null_mut(), &mut top);
            pushstring("ignored");
            assert!(top.is_null());
            assert!(matches!(popstring(), Err(Error::InvalidStackError)));
            exdll_init(u32::MAX, std::ptr::null_mut(), &mut top);
            pushstring("ignored");
            assert!(top.is_null());
            exdll_init(0, std::ptr::null_mut(), std::ptr::null_mut());
        }
    }
}
