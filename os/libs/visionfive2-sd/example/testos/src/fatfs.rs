use alloc::{string::String, vec::Vec};

use fatfs::{IoBase, Read, Seek, SeekFrom, Write};
use log::error;

use visionfive2_sd::{SDIo, Vf2SdDriver};

use crate::{println, SdIoImpl, SleepOpsImpl};

pub fn init_fatfs(mmc: Vf2SdDriver<SdIoImpl, SleepOpsImpl>) {
    let buf_stream = BufStream::new(mmc);
    let fs = fatfs::FileSystem::new(buf_stream, fatfs::FsOptions::new()).unwrap();
    let root_dir = fs.root_dir();
    let mut file = root_dir.create_file("root.txt").unwrap();
    file.write_all(b"hello world").unwrap();
    root_dir.iter().for_each(|x| {
        if let Ok(x) = x {
            let name = x.file_name();
            let len = x.len();
            println!("name: {:?}, size: {}", name, len);
        }
    });
    file.seek(SeekFrom::Start(0)).unwrap();
    let mut buf = [0; 512];
    let read = file.read(&mut buf).unwrap();
    let str = String::from_utf8_lossy(&buf[..read]);
    println!("read {} bytes: {}", read, str);

    let mut file = root_dir.open_file("bash").unwrap();
    file.seek(SeekFrom::Start(0));
    let mut buf = [0; 512];

    let mut count = 0;
    loop {
        let r = file.read(&mut buf).unwrap();
        if r == 0 {
            break;
        }
        count += r;
    }
    println!("read bash size: {}bytes", count);
}

struct BufStream {
    offset: usize,
    mmc: Vf2SdDriver<SdIoImpl, SleepOpsImpl>,
}

impl BufStream {
    pub fn new(mmc: Vf2SdDriver<SdIoImpl, SleepOpsImpl>) -> BufStream {
        Self { offset: 0, mmc }
    }
}

impl IoBase for BufStream {
    type Error = ();
}

impl Read for BufStream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        error!("read buf len: {}, offset:{}", buf.len(), self.offset);
        let buf_len = buf.len();
        let mut offset = self.offset;
        let mut read_len = 0;
        let mut tmp_buf = [0; 512]; //64*8=512
        while read_len < buf_len {
            let block_id = offset / 512;
            let block_offset = offset % 512;
            self.mmc.read_block(block_id, &mut tmp_buf);
            let copy_len = 512 - block_offset;
            let copy_len = if copy_len > buf_len - read_len {
                buf_len - read_len
            } else {
                copy_len
            };
            buf[read_len..read_len + copy_len]
                .copy_from_slice(&tmp_buf[block_offset..block_offset + copy_len]);
            read_len += copy_len;
            offset += copy_len;
        }
        self.offset = offset;
        error!("read len: {}", read_len);
        Ok(read_len)
    }
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Self::Error> {
        self.read(buf).map(|_| ())
    }
}

impl Write for BufStream {
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        error!("write buf len: {}, offset:{}", buf.len(), self.offset);
        // let buf = unsafe { core::mem::transmute::<&[u8], &mut [u8]>(buf) };
        let buf_len = buf.len();
        let mut offset = self.offset;
        let mut write_len = 0;
        let mut tmp_buf = [0; 512];
        while write_len < buf_len {
            let block_id = offset / 512;
            let block_offset = offset % 512;
            let copy_len = 512 - block_offset;
            let copy_len = if copy_len > buf_len - write_len {
                buf_len - write_len
            } else {
                copy_len
            };
            if copy_len < 512 {
                self.mmc.read_block(block_id, &mut tmp_buf);
                tmp_buf[block_offset..block_offset + copy_len]
                    .copy_from_slice(&buf[write_len..write_len + copy_len]);
                self.mmc.write_block(block_id, &mut tmp_buf);
            } else {
                self.mmc
                    .write_block(block_id, &buf[write_len..write_len + copy_len]);
            }
            write_len += copy_len;
            offset += copy_len;
        }
        self.offset = offset;
        Ok(write_len)
    }
    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl Seek for BufStream {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, Self::Error> {
        match pos {
            SeekFrom::Current(pos) => {
                self.offset = (self.offset as i64 + pos) as usize;
                Ok(self.offset as u64)
            }
            SeekFrom::Start(pos) => {
                self.offset = pos as usize;
                Ok(self.offset as u64)
            }
            SeekFrom::End(pos) => {
                // self.offset = (self.mmc.size() as i64 + pos) as usize;
                // Ok(self.offset as u64)
                panic!("SeekFrom::End not implemented")
            }
        }
    }
}
