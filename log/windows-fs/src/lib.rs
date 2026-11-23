//! Win32 handle operations for the log's private files (log/TECHNICAL 4.7).

#![cfg(windows)]

use std::ffi::c_void;
use std::fs::{File, OpenOptions};
use std::io;
use std::mem::{size_of, zeroed};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::Path;
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::{
    GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE, LocalFree,
};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo,
    SE_FILE_OBJECT,
};
use windows_sys::Win32::Security::{
    ACCESS_ALLOWED_ACE, ACL, DACL_SECURITY_INFORMATION, EqualSid, GetAce, GetTokenInformation,
    OWNER_SECURITY_INFORMATION, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER, TokenUser,
};
use windows_sys::Win32::Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, CREATE_NEW, CreateDirectoryW, CreateFileW,
    FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_REPARSE_POINT,
    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_GENERIC_READ,
    FILE_GENERIC_WRITE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_TYPE_DISK,
    GetFileInformationByHandle, GetFileType, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    MoveFileExW,
};
use windows_sys::Win32::System::SystemServices::ACCESS_ALLOWED_ACE_TYPE;
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

struct LocalAllocation(*mut c_void);

impl Drop for LocalAllocation {
    fn drop(&mut self) {
        // SAFETY: Windows allocated this pointer with LocalAlloc.
        unsafe { LocalFree(self.0) };
    }
}

struct User {
    token_info: Vec<usize>,
}

impl User {
    fn current() -> io::Result<Self> {
        let mut token = null_mut();
        // SAFETY: The pseudo handle is valid and token points to writable storage.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: OpenProcessToken returned a new owned handle.
        let token = unsafe { OwnedHandle::from_raw_handle(token) };
        let mut bytes = 0;
        // SAFETY: A null buffer with length zero asks for the required length.
        unsafe { GetTokenInformation(token.as_raw_handle(), TokenUser, null_mut(), 0, &mut bytes) };
        if (bytes as usize) < size_of::<TOKEN_USER>() {
            return Err(io::Error::last_os_error());
        }
        let mut token_info = vec![0usize; (bytes as usize).div_ceil(size_of::<usize>())];
        // SAFETY: The aligned buffer has room for bytes and remains owned by User.
        if unsafe {
            GetTokenInformation(
                token.as_raw_handle(),
                TokenUser,
                token_info.as_mut_ptr().cast(),
                bytes,
                &mut bytes,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { token_info })
    }

    fn sid(&self) -> *mut c_void {
        // SAFETY: A successful TokenUser call wrote a TOKEN_USER at the aligned start.
        unsafe { (*self.token_info.as_ptr().cast::<TOKEN_USER>()).User.Sid }
    }

    fn descriptor(&self, directory: bool) -> io::Result<LocalAllocation> {
        let mut sid = null_mut();
        // SAFETY: The token information holds a valid SID and sid receives the allocation.
        if unsafe { ConvertSidToStringSidW(self.sid(), &mut sid) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let _sid_allocation = LocalAllocation(sid.cast());
        let mut len = 0;
        // SAFETY: The API returns a nul-terminated string.
        while unsafe { *sid.add(len) } != 0 {
            len += 1;
        }
        // SAFETY: The scan established the length while the allocation is live.
        let sid = String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(sid, len) });
        let inheritance = if directory { "OICI" } else { "" };
        let sddl: Vec<_> = format!("O:{sid}D:P(A;{inheritance};FA;;;{sid})")
            .encode_utf16()
            .chain([0])
            .collect();
        let mut descriptor = null_mut();
        // SAFETY: sddl is nul terminated and descriptor receives the allocation.
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                1,
                &mut descriptor,
                null_mut(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(LocalAllocation(descriptor))
    }
}

fn absolute_wide(path: &Path) -> io::Result<Vec<u16>> {
    let path = std::path::absolute(path)?;
    let mut wide: Vec<_> = path.as_os_str().encode_wide().collect();
    if wide.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "nul in Windows path",
        ));
    }
    wide.push(0);
    Ok(wide)
}

fn private_attributes(descriptor: &LocalAllocation) -> SECURITY_ATTRIBUTES {
    SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: 0,
    }
}

/// Create a directory with a protected, owner-only ACL from the first instant.
pub fn create_dir(path: &Path) -> io::Result<()> {
    let path = absolute_wide(path)?;
    let descriptor = User::current()?.descriptor(true)?;
    let attributes = private_attributes(&descriptor);
    // SAFETY: The path is nul terminated and attributes borrow the live descriptor.
    if unsafe { CreateDirectoryW(path.as_ptr(), &attributes) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Create missing directories with the same private ACL as the final directory.
pub fn create_dir_all(path: &Path) -> io::Result<()> {
    match create_dir(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            let parent = path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .ok_or(e)?;
            create_dir_all(parent)?;
            match create_dir(path) {
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(()),
                result => result,
            }
        }
        Err(e) => Err(e),
    }
}

/// Create an exclusive new file with a protected owner-only ACL.
pub fn create_file(path: &Path) -> io::Result<File> {
    let path = absolute_wide(path)?;
    let descriptor = User::current()?.descriptor(false)?;
    let attributes = private_attributes(&descriptor);
    // SAFETY: The path is nul terminated and attributes borrow the live descriptor.
    let handle = unsafe {
        CreateFileW(
            path.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            &attributes,
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
            null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: CreateFileW returned a new owned file handle.
    Ok(unsafe { File::from_raw_handle(handle) })
}

pub fn open_path(path: &Path, write: bool) -> io::Result<File> {
    // Omitting share-delete pins the final name while this handle is open.
    OpenOptions::new()
        .access_mode(FILE_GENERIC_READ | if write { FILE_GENERIC_WRITE } else { 0 })
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
}

#[derive(Debug)]
pub enum FileKind {
    Directory,
    Regular { links: u32 },
    ReparsePoint,
    Other,
}

pub fn kind(file: &File) -> io::Result<FileKind> {
    let handle = file.as_raw_handle();
    // SAFETY: This Win32 result structure is valid zero-initialized.
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { zeroed() };
    // SAFETY: The live handle and writable info are valid for the call.
    if unsafe { GetFileType(handle) } != FILE_TYPE_DISK {
        return Ok(FileKind::Other);
    }
    // SAFETY: The live handle and writable info are valid for the call.
    if unsafe { GetFileInformationByHandle(handle, &mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(
        if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            FileKind::ReparsePoint
        } else if info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0 {
            FileKind::Directory
        } else {
            FileKind::Regular {
                links: info.nNumberOfLinks,
            }
        },
    )
}

pub fn check_owner_only(file: &File) -> io::Result<()> {
    let user = User::current()?;
    let mut owner = null_mut();
    let mut dacl: *mut ACL = null_mut();
    let mut descriptor = null_mut();
    // SAFETY: All outputs point to writable storage. Owner and DACL borrow the descriptor.
    let status = unsafe {
        GetSecurityInfo(
            file.as_raw_handle(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            null_mut(),
            &mut dacl,
            null_mut(),
            &mut descriptor,
        )
    };
    if status != 0 {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    let _descriptor = LocalAllocation(descriptor);
    let refused = || {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            "requires an owner-only Windows ACL and ownership by the current user",
        )
    };
    // SAFETY: Win32 supplied an owner SID if non-null. The user SID is live.
    if owner.is_null() || dacl.is_null() || unsafe { EqualSid(owner, user.sid()) } == 0 {
        return Err(refused());
    }
    // SAFETY: GetSecurityInfo supplies a live ACL.
    if unsafe { (*dacl).AceCount } != 1 {
        return Err(refused());
    }
    let mut ace = null_mut();
    // SAFETY: The ACL contains one ACE and ace receives a pointer to it.
    if unsafe { GetAce(dacl, 0, &mut ace) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let ace = ace.cast::<ACCESS_ALLOWED_ACE>();
    // SAFETY: Every ACE starts with an ACE_HEADER. Read the SID only for an allow ACE.
    if unsafe { (*ace).Header.AceType } != ACCESS_ALLOWED_ACE_TYPE as u8 {
        return Err(refused());
    }
    // SAFETY: The inline SID of this access-allowed ACE is live with its descriptor.
    if unsafe { EqualSid((&raw mut (*ace).SidStart).cast(), user.sid()) } == 0 {
        return Err(refused());
    }
    Ok(())
}

/// Publish the flushed temporary file and wait for Windows to complete the move.
pub fn publish(tmp: &Path, key: &Path, replace: bool) -> io::Result<()> {
    let tmp = absolute_wide(tmp)?;
    let key = absolute_wide(key)?;
    let flags = MOVEFILE_WRITE_THROUGH
        | if replace {
            MOVEFILE_REPLACE_EXISTING
        } else {
            0
        };
    // SAFETY: Both paths are nul-terminated buffers live for the call.
    if unsafe { MoveFileExW(tmp.as_ptr(), key.as_ptr(), flags) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
