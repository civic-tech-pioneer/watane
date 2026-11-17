//! Deterministic CBOR, arrays only (TECHNICAL 4.2). The reader accepts
//! exactly the shortest encodings of: unsigned integers, byte strings, text,
//! null, booleans and definite arrays of these. Everything else is refused,
//! so what is accepted always encodes back to itself.

const MAX_DEPTH: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Item<'a> {
    Uint(u64),
    Bytes(&'a [u8]),
    Text(&'a str),
    Null,
    Bool(bool),
    Array(Vec<Item<'a>>),
}

pub(crate) fn head(major: u8, n: u64, out: &mut Vec<u8>) {
    let m = major << 5;
    if n < 24 {
        out.push(m | n as u8);
    } else if n <= 0xff {
        out.extend([m | 24, n as u8]);
    } else if n <= 0xffff {
        out.push(m | 25);
        out.extend((n as u16).to_be_bytes());
    } else if n <= 0xffff_ffff {
        out.push(m | 26);
        out.extend((n as u32).to_be_bytes());
    } else {
        out.push(m | 27);
        out.extend(n.to_be_bytes());
    }
}
pub(crate) fn uint(n: u64, out: &mut Vec<u8>) {
    head(0, n, out);
}
pub(crate) fn bytes(b: &[u8], out: &mut Vec<u8>) {
    head(2, b.len() as u64, out);
    out.extend_from_slice(b);
}
pub(crate) fn text(t: &str, out: &mut Vec<u8>) {
    head(3, t.len() as u64, out);
    out.extend_from_slice(t.as_bytes());
}
pub(crate) fn array(len: usize, out: &mut Vec<u8>) {
    head(4, len as u64, out);
}
pub(crate) fn null(out: &mut Vec<u8>) {
    out.push(0xf6);
}
pub(crate) fn boolean(b: bool, out: &mut Vec<u8>) {
    out.push(if b { 0xf5 } else { 0xf4 });
}

/// Reads a CBOR sequence item by item.
pub(crate) struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Reader { data, pos: 0 }
    }
    pub(crate) fn at_end(&self) -> bool {
        self.pos == self.data.len()
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], &'static str> {
        let end = self.pos.checked_add(n).ok_or("truncated")?;
        let slice = self.data.get(self.pos..end).ok_or("truncated")?;
        self.pos = end;
        Ok(slice)
    }
    fn argument(&mut self, info: u8) -> Result<u64, &'static str> {
        let (n, min) = match info {
            0..=23 => return Ok(info as u64),
            24 => (self.take(1)?[0] as u64, 24),
            25 => (
                u16::from_be_bytes(self.take(2)?.try_into().expect("take(2) returns 2 bytes"))
                    as u64,
                0x100,
            ),
            26 => (
                u32::from_be_bytes(self.take(4)?.try_into().expect("take(4) returns 4 bytes"))
                    as u64,
                0x1_0000,
            ),
            27 => (
                u64::from_be_bytes(self.take(8)?.try_into().expect("take(8) returns 8 bytes")),
                0x1_0000_0000,
            ),
            31 => return Err("indefinite length"),
            _ => return Err("reserved additional information"),
        };
        if n < min {
            return Err("not the shortest encoding");
        }
        Ok(n)
    }
    /// The next item, or an error that names what is wrong.
    pub(crate) fn item(&mut self) -> Result<Item<'a>, &'static str> {
        self.item_at(0)
    }
    fn item_at(&mut self, depth: usize) -> Result<Item<'a>, &'static str> {
        let initial = self.take(1)?[0];
        let (major, info) = (initial >> 5, initial & 31);
        match major {
            0 => Ok(Item::Uint(self.argument(info)?)),
            2 | 3 => {
                let len = self.argument(info)?;
                let len = usize::try_from(len).map_err(|_| "truncated")?;
                let body = self.take(len)?;
                if major == 2 {
                    Ok(Item::Bytes(body))
                } else {
                    Ok(Item::Text(
                        std::str::from_utf8(body).map_err(|_| "text is not UTF-8")?,
                    ))
                }
            }
            4 => {
                if depth >= MAX_DEPTH {
                    return Err("nested too deep");
                }
                let len = self.argument(info)?;
                // an item takes at least one byte
                if len > (self.data.len() - self.pos) as u64 {
                    return Err("truncated");
                }
                let mut items = Vec::with_capacity(len as usize);
                for _ in 0..len {
                    items.push(self.item_at(depth + 1)?);
                }
                Ok(Item::Array(items))
            }
            7 => match info {
                20 => Ok(Item::Bool(false)),
                21 => Ok(Item::Bool(true)),
                22 => Ok(Item::Null),
                _ => Err("not null or a boolean"),
            },
            1 => Err("negative integer"),
            5 => Err("map"),
            6 => Err("tag"),
            _ => Err("unexpected major type"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(bytes: &[u8]) -> Result<Item<'_>, &'static str> {
        let mut r = Reader::new(bytes);
        let item = r.item()?;
        if r.at_end() {
            Ok(item)
        } else {
            Err("trailing")
        }
    }

    #[test]
    fn accepts_shortest_forms() {
        assert_eq!(one(&[0x17]), Ok(Item::Uint(23)));
        assert_eq!(one(&[0x18, 24]), Ok(Item::Uint(24)));
        assert_eq!(one(&[0x19, 1, 0]), Ok(Item::Uint(256)));
        assert_eq!(
            one(&[0x82, 0x01, 0xf6]),
            Ok(Item::Array(vec![Item::Uint(1), Item::Null]))
        );
    }

    #[test]
    fn refuses_the_rest() {
        for bad in [
            &[0x18, 23][..],         // 23 in two bytes
            &[0x19, 0, 255],         // 255 in three bytes
            &[0x1a, 0, 0, 255, 255], // 65535 in five bytes
            &[0x1b, 0, 0, 0, 0, 255, 255, 255, 255],
            &[0x9f, 0xff],       // indefinite array
            &[0x5f, 0xff],       // indefinite bytes
            &[0x7f, 0xff],       // indefinite text
            &[0xa0],             // map
            &[0x20],             // negative integer
            &[0xc0, 0x00],       // tag
            &[0xf9, 0, 0],       // float
            &[0xf7],             // undefined
            &[0x62, 0xff, 0xfe], // text that is not UTF-8
            &[0x82, 0x01],       // truncated array
            &[0x01, 0x02],       // trailing
        ] {
            assert!(one(bad).is_err(), "{bad:02x?}");
        }
    }
}
