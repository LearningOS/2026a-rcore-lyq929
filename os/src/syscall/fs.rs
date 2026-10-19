//! File and filesystem-related syscalls
const FD_STDOUT: usize = 1;
use crate::syscall::process::SYS_CALL_COUNT;
use super::SYSCALL_WRITE;

/// write buf of length `len`  to a file with `fd`
pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    trace!("kernel: sys_write");
    SYS_CALL_COUNT.borrow_mut()[SYSCALL_WRITE] += 1;
    match fd {
        FD_STDOUT => {
            let slice = unsafe { core::slice::from_raw_parts(buf, len) };
            let str = core::str::from_utf8(slice).unwrap();
            print!("{}", str);
            len as isize
        }
        _ => {
            panic!("Unsupported fd in sys_write!");
        }
    }
}
