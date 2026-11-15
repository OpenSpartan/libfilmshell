//! Bond Compact Binary v2, the encoding of map variants, of the game variant
//! a film's chunk 0 carries, and of the multiplayer properties a film carries
//! for a placed object.
//!
//! A struct is its length in bytes (ULEB128), then its fields, each a byte
//! with the type in the low five bits and the id in the high three (6: the id
//! in the next byte, 7: in the next two, little-endian), then the value.
//! Fields end at a stop byte (type 0) or at the struct's end; type 1 ends a
//! base struct's fields, and the derived struct's follow. Unsigned integers
//! are ULEB128, signed ones zigzag ULEB128, floats little-endian. A list or
//! set is a byte with the element type in the low five bits and, below 7,
//! one more than the count in the high three (else the count follows as
//! ULEB128); a map is the key type, the value type and the count.

/// How deep structs and containers may nest: past it a document is refused,
/// so damaged bytes can't exhaust the stack.
const MAX_DEPTH: u32 = 64;

/// A value. Integers of every width and sign are `Int` (a `u64` past
/// `i64::MAX` wraps), floats of both widths `Float`, and a list or set of
/// bytes is `Bytes`.
#[derive(Clone, Debug, PartialEq)]
pub enum Value<'a> {
    Bool(bool),
    Int(i64),
    Float(f64),
    /// A UTF-8 string's bytes.
    Str(&'a [u8]),
    /// A UTF-16 string's bytes, little-endian.
    WStr(&'a [u8]),
    Bytes(&'a [u8]),
    Struct(Struct<'a>),
    /// A list or a set.
    List(Vec<Value<'a>>),
    Map(Vec<(Value<'a>, Value<'a>)>),
}

/// A struct's fields by id, in the order written.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Struct<'a> {
    pub fields: Vec<(u16, Value<'a>)>,
}

impl<'a> Struct<'a> {
    pub fn get(&self, id: u16) -> Option<&Value<'a>> {
        self.fields.iter().find(|f| f.0 == id).map(|f| &f.1)
    }

    /// An integer field, or a struct field that holds one as its field 0
    /// (how the game wraps ids and fixed-point numbers).
    pub fn int(&self, id: u16) -> Option<i64> {
        match self.get(id)? {
            Value::Int(v) => Some(*v),
            Value::Struct(s) => match s.get(0)? {
                Value::Int(v) => Some(*v),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn float(&self, id: u16) -> Option<f64> {
        match self.get(id)? {
            Value::Float(v) => Some(*v),
            _ => None,
        }
    }

    pub fn bool(&self, id: u16) -> Option<bool> {
        match self.get(id)? {
            Value::Bool(v) => Some(*v),
            _ => None,
        }
    }

    pub fn strukt(&self, id: u16) -> Option<&Struct<'a>> {
        match self.get(id)? {
            Value::Struct(s) => Some(s),
            _ => None,
        }
    }

    pub fn list(&self, id: u16) -> &[Value<'a>] {
        match self.get(id) {
            Some(Value::List(l)) => l,
            _ => &[],
        }
    }

    /// A list field's first struct, or a struct field itself.
    pub fn first(&self, id: u16) -> Option<&Struct<'a>> {
        match self.get(id)? {
            Value::List(l) => match l.first()? {
                Value::Struct(s) => Some(s),
                _ => None,
            },
            Value::Struct(s) => Some(s),
            _ => None,
        }
    }

    pub fn map(&self, id: u16) -> &[(Value<'a>, Value<'a>)] {
        match self.get(id) {
            Some(Value::Map(m)) => m,
            _ => &[],
        }
    }
}

/// The struct `bytes` start with. Err holds the offset where the bytes stop
/// making sense.
pub fn read(bytes: &[u8]) -> Result<Struct<'_>, usize> {
    let mut r = Reader { b: bytes, p: 0 };
    r.strukt(0).ok_or(r.p)
}

/// The fields of a struct whose bytes, `bytes`, come without their length:
/// how a film carries an object's properties, after a length of its own.
pub fn fields(bytes: &[u8]) -> Result<Struct<'_>, usize> {
    let mut r = Reader { b: bytes, p: 0 };
    r.body(bytes.len(), 0).ok_or(r.p)
}

/// The ULEB128 number `bytes` start with, and how many bytes it takes.
pub fn uleb(bytes: &[u8]) -> Option<(u64, usize)> {
    let mut r = Reader { b: bytes, p: 0 };
    let v = r.uleb()?;
    Some((v, r.p))
}

struct Reader<'a> {
    b: &'a [u8],
    p: usize,
}

impl<'a> Reader<'a> {
    fn u8(&mut self) -> Option<u8> {
        let v = *self.b.get(self.p)?;
        self.p += 1;
        Some(v)
    }

    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.p.checked_add(n)?;
        let v = self.b.get(self.p..end)?;
        self.p = end;
        Some(v)
    }

    fn uleb(&mut self) -> Option<u64> {
        let (mut v, mut sh) = (0u64, 0);
        loop {
            let b = self.u8()?;
            if sh < 64 {
                v |= u64::from(b & 0x7f) << sh;
            }
            sh += 7;
            if b & 0x80 == 0 {
                return Some(v);
            }
            if sh > 70 {
                return None;
            }
        }
    }

    fn count(&mut self) -> Option<usize> {
        usize::try_from(self.uleb()?).ok()
    }

    fn strukt(&mut self, depth: u32) -> Option<Struct<'a>> {
        let n = self.count()?;
        let end = self.p.checked_add(n).filter(|&e| e <= self.b.len())?;
        self.body(end, depth)
    }

    /// A struct's fields, up to `end`.
    fn body(&mut self, end: usize, depth: u32) -> Option<Struct<'a>> {
        if depth > MAX_DEPTH {
            return None;
        }
        let mut fields = Vec::new();
        while self.p < end {
            let h = self.u8()?;
            let (ty, id) = (h & 0x1f, h >> 5);
            match ty {
                0 => break,
                1 => continue,
                _ => {}
            }
            let id = match id {
                6 => u16::from(self.u8()?),
                7 => u16::from_le_bytes([self.u8()?, self.u8()?]),
                i => u16::from(i),
            };
            fields.push((id, self.value(ty, depth)?));
        }
        if self.p > end {
            return None;
        }
        self.p = end;
        Some(Struct { fields })
    }

    fn value(&mut self, ty: u8, depth: u32) -> Option<Value<'a>> {
        Some(match ty {
            2 => Value::Bool(self.u8()? != 0),
            3 => Value::Int(i64::from(self.u8()?)),
            14 => Value::Int(i64::from(self.u8()? as i8)),
            4..=6 => Value::Int(self.uleb()? as i64),
            15..=17 => {
                let u = self.uleb()?;
                Value::Int((u >> 1) as i64 ^ -((u & 1) as i64))
            }
            7 => Value::Float(f64::from(f32::from_le_bytes(self.take(4)?.try_into().ok()?))),
            8 => Value::Float(f64::from_le_bytes(self.take(8)?.try_into().ok()?)),
            9 => {
                let n = self.count()?;
                Value::Str(self.take(n)?)
            }
            18 => {
                let n = self.count()?;
                Value::WStr(self.take(n.checked_mul(2)?)?)
            }
            10 => Value::Struct(self.strukt(depth + 1)?),
            11 | 12 => {
                let h = self.u8()?;
                let (e, c) = (h & 0x1f, h >> 5);
                let n = if c == 0 { self.count()? } else { usize::from(c) - 1 };
                if e == 3 || e == 14 {
                    return Some(Value::Bytes(self.take(n)?));
                }
                if depth >= MAX_DEPTH {
                    return None;
                }
                // Every element takes a byte at least: a count past the bytes
                // left is damage, and allocates nothing.
                let mut l = Vec::with_capacity(n.min(self.b.len() - self.p));
                for _ in 0..n {
                    l.push(self.value(e, depth + 1)?);
                }
                Value::List(l)
            }
            13 => {
                let (k, v) = (self.u8()? & 0x1f, self.u8()? & 0x1f);
                let n = self.count()?;
                if depth >= MAX_DEPTH {
                    return None;
                }
                let mut m = Vec::with_capacity(n.min(self.b.len() - self.p));
                for _ in 0..n {
                    m.push((self.value(k, depth + 1)?, self.value(v, depth + 1)?));
                }
                Value::Map(m)
            }
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalars_and_ids() {
        // { 0: bool true, 1: u8 200, 2: i32 -3, 3: u32 300, 4: float 1.5,
        //   5: double -0.25, 6: string "ab", 300: i8 -1 } and a stop.
        let mut b = vec![
            0x22, 0x02, 0x01, 0x23, 200, 0x50, 0x05, 0x65, 0xac, 0x02, 0x87, 0x00, 0x00, 0xc0, 0x3f, 0xa8, 0, 0, 0, 0,
            0, 0, 0xd0, 0xbf, 0xc9, 6, 2, b'a', b'b', 0xee, 0x2c, 0x01, 0xff, 0x00,
        ];
        b[0] = b.len() as u8 - 1;
        let s = read(&b).unwrap();
        assert_eq!(s.bool(0), Some(true));
        assert_eq!(s.int(1), Some(200));
        assert_eq!(s.int(2), Some(-3));
        assert_eq!(s.int(3), Some(300));
        assert_eq!(s.float(4), Some(1.5));
        assert_eq!(s.float(5), Some(-0.25));
        assert_eq!(s.get(6), Some(&Value::Str(b"ab")));
        assert_eq!(s.int(300), Some(-1));
        assert_eq!(s.get(7), None);
    }

    #[test]
    fn containers() {
        // { 0: list<struct> [{0: i32 5}], 1: map<u32, float> {7: 2.0},
        //   2: set<i32> of 8 elements, 3: list<u8> [1, 2], 4: struct { 0: i32 {0: 9} } }
        let mut b = vec![0u8];
        b.extend([0x0b, 0x4a, 0x03, 0x10, 0x0a, 0x00]);
        b.extend([0x2d, 0x05, 0x07, 0x01, 0x07, 0x00, 0x00, 0x00, 0x40]);
        b.extend([0x4c, 0x10, 0x08, 2, 4, 6, 8, 10, 12, 14, 16]);
        b.extend([0x6b, 0x63, 1, 2]);
        b.extend([0x8a, 0x06, 0x0a, 0x03, 0x10, 0x12, 0x00, 0x00]);
        b[0] = b.len() as u8 - 1;
        let s = read(&b).unwrap();
        assert_eq!(s.first(0).and_then(|e| e.int(0)), Some(5));
        assert_eq!(s.map(1), &[(Value::Int(7), Value::Float(2.0))]);
        assert_eq!(s.list(2).len(), 8);
        assert_eq!(s.list(2)[7], Value::Int(8));
        assert_eq!(s.get(3), Some(&Value::Bytes(&[1, 2])));
        assert_eq!(s.strukt(4).and_then(|v| v.int(0)), Some(9));
        assert_eq!(s.first(4).and_then(|v| v.int(0)), Some(9));
    }

    #[test]
    fn a_base_struct_and_its_derived_fields_read_as_one() {
        // { base: { 0: u8 1 }, then 1: u8 2 }
        let b = [0x06, 0x03, 1, 0x01, 0x23, 2, 0x00];
        let s = read(&b).unwrap();
        assert_eq!((s.int(0), s.int(1)), (Some(1), Some(2)));
        assert_eq!(fields(&b[1..]), Ok(s));
    }

    #[test]
    fn damage_is_refused() {
        // Cut short, past a struct's length, an unknown type, a count past
        // the bytes.
        assert!(read(&[0x05, 0x23]).is_err());
        assert!(read(&[0x02, 0x10, 0x80, 0x80]).is_err());
        assert!(read(&[0x02, 0x13, 0x00]).is_err());
        assert!(read(&[0x04, 0x0b, 0x0a, 0xff, 0x7f]).is_err());
        // Structs nested as deep as the limit, and past it.
        let nested = |depth: usize| {
            let mut s = vec![0u8];
            for _ in 0..depth {
                let mut outer = vec![0x0au8];
                outer.extend(&s);
                let mut n = outer.len();
                s = Vec::new();
                while n >= 0x80 {
                    s.push(n as u8 | 0x80);
                    n >>= 7;
                }
                s.push(n as u8);
                s.extend(outer);
            }
            s
        };
        assert!(read(&nested(MAX_DEPTH as usize)).is_ok());
        assert!(read(&nested(MAX_DEPTH as usize + 2)).is_err());
    }
}
