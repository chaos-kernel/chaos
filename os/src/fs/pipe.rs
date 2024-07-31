use alloc::{
    sync::{Arc, Weak},
    vec::Vec,
};

use super::{file::File, inode::Stat};
use crate::{mm::UserBuffer, sync::UPSafeCell, task::suspend_current_and_run_next};

/// IPC pipe
pub struct Pipe {
    readable: bool,
    writable: bool,
    buffer:   Arc<UPSafeCell<PipeRingBuffer>>,
}

impl Pipe {
    /// create readable pipe
    pub fn read_end_with_buffer(buffer: Arc<UPSafeCell<PipeRingBuffer>>) -> Self {
        Self {
            readable: true,
            writable: false,
            buffer,
        }
    }
    /// create writable pipe
    pub fn write_end_with_buffer(buffer: Arc<UPSafeCell<PipeRingBuffer>>) -> Self {
        Self {
            readable: false,
            writable: true,
            buffer,
        }
    }
}

const RING_BUFFER_SIZE: usize = 32;

#[derive(Copy, Clone, PartialEq, Debug)]
enum RingBufferStatus {
    Full,
    Empty,
    Normal,
}

pub struct PipeRingBuffer {
    arr:       [u8; RING_BUFFER_SIZE],
    head:      usize,
    tail:      usize,
    status:    RingBufferStatus,
    write_end: Option<Weak<Pipe>>,
    read_end:  Option<Weak<Pipe>>,
}

impl Default for PipeRingBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl PipeRingBuffer {
    pub fn new() -> Self {
        Self {
            arr:       [0; RING_BUFFER_SIZE],
            head:      0,
            tail:      0,
            status:    RingBufferStatus::Empty,
            write_end: None,
            read_end:  None,
        }
    }
    pub fn set_write_end(&mut self, write_end: &Arc<Pipe>) {
        self.write_end = Some(Arc::downgrade(write_end));
    }
    pub fn write_byte(&mut self, byte: u8) {
        self.status = RingBufferStatus::Normal;
        self.arr[self.tail] = byte;
        self.tail = (self.tail + 1) % RING_BUFFER_SIZE;
        if self.tail == self.head {
            self.status = RingBufferStatus::Full;
        }
    }
    pub fn read_byte(&mut self) -> u8 {
        self.status = RingBufferStatus::Normal;
        let c = self.arr[self.head];
        self.head = (self.head + 1) % RING_BUFFER_SIZE;
        if self.head == self.tail {
            self.status = RingBufferStatus::Empty;
        }
        c
    }
    pub fn available_read(&self) -> usize {
        if self.status == RingBufferStatus::Empty {
            0
        } else if self.tail > self.head {
            self.tail - self.head
        } else {
            self.tail + RING_BUFFER_SIZE - self.head
        }
    }
    pub fn available_write(&self) -> usize {
        // error!("status: {:?}", self.status);
        if self.status == RingBufferStatus::Full {
            0
        } else {
            RING_BUFFER_SIZE - self.available_read()
        }
    }
    pub fn all_write_ends_closed(&self) -> bool {
        self.write_end.as_ref().unwrap().upgrade().is_none()
    }
    pub fn all_read_ends_closed(&self) -> bool {
        self.read_end.as_ref().unwrap().upgrade().is_none()
    }
}

/// Return (read_end, write_end)
pub fn make_pipe() -> (Arc<Pipe>, Arc<Pipe>) {
    trace!("kernel: make_pipe");
    let buffer = Arc::new(unsafe { UPSafeCell::new(PipeRingBuffer::new()) });
    let read_end = Arc::new(Pipe::read_end_with_buffer(buffer.clone()));
    let write_end = Arc::new(Pipe::write_end_with_buffer(buffer.clone()));
    buffer
        .exclusive_access(file!(), line!())
        .set_write_end(&write_end);
    (read_end, write_end)
}

impl File for Pipe {
    fn readable(&self) -> bool {
        // TODO: check if the write end is closed
        true
    }
    fn writable(&self) -> bool {
        // TODO: check if the read end is closed
        true
    }
    fn read(&self, buf: &mut [u8]) -> usize {
        trace!("kernel: Pipe::read");
        assert!(self.readable());
        let want_to_read = buf.len();
        let mut buf_iter = buf.into_iter();
        let mut already_read = 0usize;
        loop {
            let mut ring_buffer = self.buffer.exclusive_access(file!(), line!());
            let loop_read = ring_buffer.available_read();
            if loop_read == 0 {
                if ring_buffer.all_write_ends_closed() {
                    return already_read;
                }
                drop(ring_buffer);
                suspend_current_and_run_next();
                continue;
            }
            for _ in 0..loop_read {
                if let Some(byte_ref) = buf_iter.next() {
                    *byte_ref = ring_buffer.read_byte();
                    warn!("read byte: {}", *byte_ref as char);
                    already_read += 1;
                    if already_read == want_to_read {
                        return want_to_read;
                    }
                } else {
                    return already_read;
                }
            }
        }
    }
    fn read_all(&self) -> Vec<u8> {
        trace!("kernel: Pipe::read_all");
        let mut v = Vec::new();
        let mut buf = [0u8; 512];
        loop {
            let len = self.read(&mut buf);
            if len == 0 {
                break;
            }
            v.extend_from_slice(&buf[..len]);
        }
        v
    }
    fn write(&self, buf: &[u8]) -> usize {
        trace!("kernel: Pipe::write");
        assert!(self.writable());
        let want_to_write = buf.len();
        let mut buf_iter = buf.into_iter();
        let mut already_write = 0usize;
        loop {
            let mut ring_buffer = self.buffer.exclusive_access(file!(), line!());
            let loop_write = ring_buffer.available_write();
            if loop_write == 0 {
                drop(ring_buffer);
                suspend_current_and_run_next();
                continue;
            }
            // write at most loop_write bytes
            for _ in 0..loop_write {
                if let Some(byte_ref) = buf_iter.next() {
                    ring_buffer.write_byte(unsafe { *byte_ref });
                    already_write += 1;
                    if already_write == want_to_write {
                        return want_to_write;
                    }
                } else {
                    return already_write;
                }
            }
        }
    }
    fn fstat(&self) -> Option<Stat> {
        panic!("Pipe::fstat not implemented");
    }
    fn hang_up(&self) -> bool {
        let mut ring_buffer = self.buffer.exclusive_access(file!(), line!());
        if self.readable {
            ring_buffer.all_write_ends_closed()
        } else {
            ring_buffer.all_read_ends_closed()
        }
    }
    fn r_ready(&self) -> bool {
        let ring_buffer = self.buffer.exclusive_access(file!(), line!());
        ring_buffer.status != RingBufferStatus::Empty
    }
    fn w_ready(&self) -> bool {
        let ring_buffer = self.buffer.exclusive_access(file!(), line!());
        ring_buffer.status != RingBufferStatus::Full
    }
}
