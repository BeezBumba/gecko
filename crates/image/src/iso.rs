use crate::dvd::*;
use std::io::{Read, Seek, SeekFrom};
use std::sync::Mutex;
use zerocopy::FromBytes;

pub struct Iso {
    pub header: Header,
    pub header_info: HeaderInfo,
    pub apploader: Apploader,
    pub filesystem: FstNode,
    data: Vec<u8>,
}

impl Iso {
    pub fn parse(data: Vec<u8>) -> Self {
        let header = Header::read_from_bytes(&data[DVD_HEADER_OFFSET..DVD_HEADER_OFFSET + DVD_HEADER_SIZE]).unwrap();
        let header_info =
            HeaderInfo::read_from_bytes(&data[DVD_HEADER_INFO_OFFSET..DVD_HEADER_INFO_OFFSET + DVD_HEADER_INFO_SIZE])
                .unwrap();
        let apploader =
            Apploader::read_from_bytes(&data[DVD_APPLOADER_OFFSET..DVD_APPLOADER_OFFSET + DVD_APPLOADER_SIZE]).unwrap();

        let fst_start = header.offset_filesystem.get() as usize;
        let fst_end = fst_start + header.filesystem_size.get() as usize;
        let file_offset_shift = if header.gc_magic == crate::GC_MAGIC { 0 } else { 2 };
        let filesystem = FstNode::parse(&data[fst_start..fst_end], file_offset_shift);

        Iso {
            header,
            header_info,
            apploader,
            filesystem,
            data,
        }
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

impl crate::Dvd for Iso {
    fn header(&self) -> &Header {
        &self.header
    }

    fn apploader(&self) -> &Apploader {
        &self.apploader
    }

    fn read_disc_into(&self, offset: usize, buf: &mut [u8]) {
        buf.copy_from_slice(&self.data[offset..offset + buf.len()]);
    }

    fn data_partition_offset(&self) -> u64 {
        if self.header.is_wii() { 0xF80_0000 } else { 0 }
    }

    fn read_raw_disc(&self, offset: usize, buf: &mut [u8]) {
        buf.copy_from_slice(&self.data[offset..offset + buf.len()]);
    }
}

trait ReadSeek: Read + Seek {}
impl<T: Read + Seek> ReadSeek for T {}

pub struct IsoStream {
    pub header: Header,
    pub apploader: Apploader,
    pub filesystem: FstNode,
    file_offset_shift: u32,
    reader: Mutex<Box<dyn ReadSeek + Send>>,
}

impl IsoStream {
    pub fn parse_from_reader<R>(mut reader: R) -> Self
    where
        R: Read + Seek + Send + 'static,
    {
        let mut header_bytes = [0u8; DVD_HEADER_SIZE];
        self::read_exact_at(&mut reader, DVD_HEADER_OFFSET as u64, &mut header_bytes);
        let header = Header::read_from_bytes(&header_bytes).expect("invalid ISO header");

        let mut apploader_bytes = [0u8; DVD_APPLOADER_SIZE];
        self::read_exact_at(&mut reader, DVD_APPLOADER_OFFSET as u64, &mut apploader_bytes);
        let apploader = Apploader::read_from_bytes(&apploader_bytes).expect("invalid ISO apploader");

        let fst_start = header.offset_filesystem.get() as usize;
        let fst_size = header.filesystem_size.get() as usize;
        let mut fst_bytes = vec![0u8; fst_size];
        self::read_exact_at(&mut reader, fst_start as u64, &mut fst_bytes);

        let file_offset_shift = if header.gc_magic == crate::GC_MAGIC { 0 } else { 2 };
        let filesystem = FstNode::parse(&fst_bytes, file_offset_shift);

        Self {
            header,
            apploader,
            filesystem,
            file_offset_shift,
            reader: Mutex::new(Box::new(reader)),
        }
    }

    pub fn file_offset_shift(&self) -> u32 {
        self.file_offset_shift
    }
}

impl crate::Dvd for IsoStream {
    fn header(&self) -> &Header {
        &self.header
    }

    fn apploader(&self) -> &Apploader {
        &self.apploader
    }

    fn read_disc_into(&self, offset: usize, buf: &mut [u8]) {
        let mut guard = self.reader.lock().expect("failed to lock ISO stream reader");
        self::read_exact_at(&mut *guard, offset as u64, buf);
    }

    fn data_partition_offset(&self) -> u64 {
        if self.header.is_wii() { 0xF80_0000 } else { 0 }
    }

    fn read_raw_disc(&self, offset: usize, buf: &mut [u8]) {
        let mut guard = self.reader.lock().expect("failed to lock ISO stream reader");
        self::read_exact_at(&mut *guard, offset as u64, buf);
    }
}

fn read_exact_at<R: Read + Seek + ?Sized>(reader: &mut R, offset: u64, out: &mut [u8]) {
    reader
        .seek(SeekFrom::Start(offset))
        .expect("failed to seek ISO stream");
    reader
        .read_exact(out)
        .expect("failed to read bytes from ISO stream");
}
