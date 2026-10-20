//! File and filesystem-related syscalls
const FD_STDOUT: usize = 1;

/// write buf of length `len`  to a file with `fd`
pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    trace!("kernel: sys_write");
    if fd != FD_STDOUT {
        return -1;
    }
    let s = unsafe { core::str::from_utf8_unchecked(core::slice::from_raw_parts(buf, len)) };
    print!("{}", s);
    len as isize
}
