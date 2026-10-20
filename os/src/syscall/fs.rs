//! 与文件/标准输出相关的系统调用。
//!
//! 第三章还没有真正的文件系统，唯一可用的“文件”是标准输出（fd = 1）。

/// 标准输出对应的文件描述符。
const FD_STDOUT: usize = 1;

/// 把用户态缓冲区 `buf` 里 `len` 字节的内容写到 `fd` 指向的文件。
///
/// 目前只支持标准输出：把缓冲解释成 UTF-8 字符串后直接打印。其它 fd 一律
/// 视为不支持，panic 让内核停下。
pub fn write_to_stdout(fd: usize, buf: *const u8, len: usize) -> isize {
    trace!("kernel: sys_write");
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
