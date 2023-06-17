use core::mem::MaybeUninit;

use crate::Error;
use crate::ffi;

const BUF_SZ: usize = {
    const S: usize = 0x2_0000;
    let mut sz = ffi::FUSE_MIN_READ_BUFFER;

    if sz < S {
	sz = S;
    }

    sz
};

#[repr(align(64))]
pub struct ReadBuf {
    buf:	[MaybeUninit<u8>; BUF_SZ],
}

impl ReadBuf {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
	Self {
	    buf:	[MaybeUninit::uninit(); BUF_SZ]
	}
    }

    pub fn read<'a, R: std::io::Read>(&'a mut self, r: &mut R) -> std::io::Result<ReadBufIter<'a>> {
	let buf = unsafe {
	    core::slice::from_raw_parts_mut(&mut self.buf as * mut _ as * mut u8,
					    self.buf.len())
	};

	let len = r.read(buf)?;

	Ok(ReadBufIter{
	    buf:	&buf[..len],
	    consumed:	0,
	})
    }

    pub fn buf_size(&self) -> usize {
	self.buf.len()
    }
}

pub struct ReadBufIter<'a>
{
    buf:	&'a [u8],
    consumed:	usize,
}

impl <'a> ReadBufIter<'a>
{
    pub fn next<T>(&mut self) -> std::result::Result<Option<&'a T>, Error> {
	if self.buf.is_empty() {
	    return Ok(None);
	}

	let (head, data, rest) = unsafe {
	    self.buf.align_to::<T>()
	};

	if !head.is_empty() {
	    return Err(Error::Alignment(head.len()));
	}

	if data.is_empty() {
	    return Err(Error::Size(rest.len()));
	}

	let sz = core::mem::size_of_val(&data[0]);
	self.buf = &self.buf[sz..];
	self.consumed += sz;

	Ok(Some(&data[0]))
    }

    pub fn truncate(&mut self, mut pos: usize) -> Result<usize, Error> {
	if pos < self.consumed {
	    return Err(Error::BadTruncate(pos, self.consumed));
	}

	pos -= self.consumed;

	if pos > self.buf.len() {
	    return Err(Error::Size(pos - self.buf.len()));
	}

	let delta = self.buf.len() - pos;

	self.buf = &self.buf[..pos];

	Ok(delta)
    }

    pub fn is_empty(&self) -> bool {
	self.buf.is_empty()
    }

    pub fn get_consumed(&self) -> usize {
	self.consumed
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    pub fn test_00() {
	let mut tmp = Vec::new();

	tmp.extend(23_u64.to_ne_bytes());
	tmp.extend(42_u32.to_ne_bytes());
	tmp.extend(66_u16.to_ne_bytes());
	tmp.extend(67_u16.to_ne_bytes());
	tmp.extend(68_u16.to_ne_bytes());

	let mut buf = std::io::Cursor::new(tmp);
	let mut read = ReadBuf::new();

	let mut iter = read.read(&mut buf).unwrap();

	assert_eq!(*iter.next::<u64>().unwrap().unwrap(), 23);
	assert_eq!(*iter.next::<u32>().unwrap().unwrap(), 42);
	assert_eq!(*iter.next::<u16>().unwrap().unwrap(), 66);
	assert_eq!(*iter.next::<u16>().unwrap().unwrap(), 67);
	assert_eq!(*iter.next::<u16>().unwrap().unwrap(), 68);
	assert!(iter.next::<u8>().unwrap().is_none());
    }
}
