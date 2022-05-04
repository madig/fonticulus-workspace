//! This library is used by the fonttools crate. No user-serviceable parts inside.
#[macro_use]
extern crate shrinkwraprs;

// necessary for us to use macros defined in otspec_macros
extern crate self as _tag_macro_crate;

use crate::types::*;
use std::convert::TryInto;
use std::mem;
mod counted;
pub mod layout;
pub mod offsetmanager;
mod offsets;
pub mod utils;
pub use counted::{Counted, Counted32};
pub mod tables;
mod tag;
pub mod types;

#[derive(Debug)]
pub struct SerializationError(pub String);
#[derive(Clone, Debug)]
pub struct DeserializationError(pub String);

pub struct ReaderContext {
    pub input: Vec<u8>,
    pub ptr: usize,
    top_of_table_stack: Vec<usize>,
}

impl ReaderContext {
    pub fn new(input: Vec<u8>) -> Self {
        ReaderContext {
            input,
            ptr: 0,
            top_of_table_stack: vec![0],
        }
    }

    fn consume_or_peek(
        &mut self,
        bytes: usize,
        consume: bool,
    ) -> Result<&[u8], DeserializationError> {
        if self.ptr + bytes > self.input.len() {
            Err(DeserializationError("End of file".to_string()))
        } else {
            let subslice = &self.input[self.ptr..self.ptr + bytes];
            if consume {
                self.ptr += bytes;
            }
            Ok(subslice)
        }
    }

    fn consume(&mut self, bytes: usize) -> Result<&[u8], DeserializationError> {
        self.consume_or_peek(bytes, true)
    }

    pub fn peek(&mut self, bytes: usize) -> Result<&[u8], DeserializationError> {
        self.consume_or_peek(bytes, false)
    }

    pub fn push(&mut self) {
        self.top_of_table_stack.push(self.ptr);
    }
    pub fn pop(&mut self) {
        self.top_of_table_stack
            .pop()
            .expect("pop with no matching push");
    }
    pub fn top_of_table(&self) -> usize {
        *self.top_of_table_stack.last().expect("not in a struct")
    }
    pub fn skip(&mut self, bytes: usize) {
        self.ptr += bytes;
    }

    pub fn follow_offset<T>(&mut self, offset: uint16) -> Result<(), DeserializationError> {
        let destination = self.top_of_table() + offset as usize;
        if destination > self.input.len() {
            return Err(DeserializationError(
                format!("Offset fell off end of data trying to deserialize a {:?} (probably a missing [offset_base])",
                    std::any::type_name::<T>()
                    )
            ));
        }
        self.ptr = destination;
        Ok(())
    }
}

pub trait Serializer<T>
where
    T: Serialize,
{
    fn put(&mut self, data: T) -> Result<(), SerializationError>;
}

impl<T> Serializer<T> for Vec<u8>
where
    T: Serialize,
{
    fn put(&mut self, data: T) -> Result<(), SerializationError> {
        data.to_bytes(self)
    }
}

pub trait Deserializer<T>
where
    T: Deserialize,
{
    fn de(&mut self) -> Result<T, DeserializationError>;
    fn de_counted(&mut self, s: usize) -> Result<Vec<T>, DeserializationError>;
}

impl<T> Deserializer<T> for ReaderContext
where
    T: Deserialize,
{
    fn de(&mut self) -> Result<T, DeserializationError> {
        T::from_bytes(self)
    }
    fn de_counted(&mut self, s: usize) -> Result<Vec<T>, DeserializationError> {
        (0..s)
            .map(|_| {
                let c: Result<T, DeserializationError> = self.de();
                c
            })
            .collect()
    }
}

impl std::fmt::Display for SerializationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Serialization error {:}", self.0)
    }
}

impl std::fmt::Display for DeserializationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Deserialization error {:}", self.0)
    }
}

impl std::error::Error for SerializationError {}
impl std::error::Error for DeserializationError {}

use crate::offsets::OffsetMarkerTrait;

pub trait Serialize: std::fmt::Debug {
    fn to_bytes(&self, data: &mut Vec<u8>) -> Result<(), SerializationError>;
    fn ot_binary_size(&self) -> usize {
        // Lazy implementation that works everywhere
        let mut d = vec![];
        self.to_bytes(&mut d).unwrap();
        d.len()
    }
    fn offset_fields(&self) -> Vec<&dyn OffsetMarkerTrait> {
        vec![]
    }
    fn to_bytes_shallow(&self, data: &mut Vec<u8>) -> Result<(), SerializationError> {
        self.to_bytes(data)
    }
}

pub trait Deserialize {
    fn from_bytes(c: &mut ReaderContext) -> Result<Self, DeserializationError>
    where
        Self: std::marker::Sized;
}

macro_rules! otspec_primitive {
    ($t: ty) => {
        impl Serialize for $t {
            fn to_bytes(&self, data: &mut Vec<u8>) -> Result<(), SerializationError> {
                data.extend_from_slice(&self.to_be_bytes());
                Ok(())
            }

            fn ot_binary_size(&self) -> usize {
                mem::size_of::<$t>()
            }
        }

        impl Deserialize for $t {
            fn from_bytes(c: &mut ReaderContext) -> Result<Self, DeserializationError> {
                const SIZE: usize = mem::size_of::<$t>();
                let bytes: &[u8] = c.consume(SIZE)?;
                let bytes_array: [u8; SIZE] = bytes
                    .try_into()
                    .map_err(|_| DeserializationError("Slice with incorrect length".to_string()))?;
                Ok(<$t>::from_be_bytes(bytes_array))
            }
        }
    };
}

otspec_primitive!(i8);
otspec_primitive!(u8);
otspec_primitive!(u16);
otspec_primitive!(u32);
otspec_primitive!(i16);
otspec_primitive!(i32);
otspec_primitive!(i64);

impl<T> Serialize for Option<T>
where
    T: Serialize,
{
    fn to_bytes(&self, data: &mut Vec<u8>) -> Result<(), SerializationError> {
        if let Some(v) = self {
            v.to_bytes(data)?
        }
        Ok(())
    }
    fn ot_binary_size(&self) -> usize {
        self.iter().map(|x| x.ot_binary_size()).sum()
    }
    fn offset_fields(&self) -> Vec<&dyn types::OffsetMarkerTrait> {
        if let Some(v) = self {
            v.offset_fields()
        } else {
            vec![]
        }
    }
}

impl<T> Serialize for Vec<T>
where
    T: Serialize,
{
    fn to_bytes(&self, data: &mut Vec<u8>) -> Result<(), SerializationError> {
        self.as_slice().to_bytes(data)
    }
    fn ot_binary_size(&self) -> usize {
        self.as_slice().ot_binary_size()
    }
    fn offset_fields(&self) -> Vec<&dyn types::OffsetMarkerTrait> {
        self.as_slice().offset_fields()
    }
}

impl<T> Serialize for [T]
where
    T: Serialize,
{
    fn to_bytes(&self, data: &mut Vec<u8>) -> Result<(), SerializationError> {
        for el in self {
            el.to_bytes(data)?
        }
        Ok(())
    }
    fn ot_binary_size(&self) -> usize {
        self.iter().map(|x| x.ot_binary_size()).sum()
    }
    fn offset_fields(&self) -> Vec<&dyn types::OffsetMarkerTrait> {
        let mut v = vec![];
        for el in self {
            v.extend(el.offset_fields())
        }
        v
    }
}

impl<T> Deserialize for Vec<T>
where
    T: Deserialize,
{
    fn from_bytes(c: &mut ReaderContext) -> Result<Self, DeserializationError> {
        let mut res: Vec<T> = vec![];
        loop {
            let maybe: Result<T, DeserializationError> = c.de();
            if let Ok(x) = maybe {
                res.push(x);
            } else {
                break;
            }
        }
        Ok(res)
    }
}

impl<T> Serialize for &T
where
    T: Serialize,
{
    fn to_bytes(
        &self,
        data: &mut std::vec::Vec<u8>,
    ) -> std::result::Result<(), SerializationError> {
        (*self).to_bytes(data)
    }
    fn ot_binary_size(&self) -> usize {
        (*self).ot_binary_size()
    }
    fn offset_fields(&self) -> Vec<&dyn types::OffsetMarkerTrait> {
        (*self).offset_fields()
    }
}

/* Provide a serde-style interface */
pub mod ser {
    use crate::{SerializationError, Serialize, Serializer};

    pub fn to_bytes<T: Serialize>(data: &T) -> Result<Vec<u8>, SerializationError> {
        let mut out = vec![];
        out.put(data)?;
        Ok(out)
    }
}
pub mod de {
    pub use crate::{DeserializationError, Deserialize, Deserializer, ReaderContext};
    pub fn from_bytes<T: Deserialize>(data: &[u8]) -> Result<T, DeserializationError> {
        let mut rc = ReaderContext::new(data.to_vec());
        rc.de()
    }
}

extern crate self as otspec;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ser_primitive() {
        let mut out = vec![];
        out.put(1_u16).unwrap();
        out.put(2_u16).unwrap();
        out.put(4_u32).unwrap();
        assert_eq!(out, [0x00, 0x01, 0x00, 0x02, 0x00, 0x00, 0x00, 0x04]);
    }

    #[test]
    fn de_primitive() {
        let mut rc = ReaderContext::new(vec![0x00, 0x01, 0x00, 0x02, 0x00, 0x00, 0x00, 0x04]);
        let first: u16 = rc.de().unwrap();
        let second: u16 = rc.de().unwrap();
        let third: u32 = rc.de().unwrap();
        assert_eq!(first, 1);
        assert_eq!(second, 2);
        assert_eq!(third, 4);
    }

    #[test]
    fn ser_vec() {
        let mut out = vec![];
        let t: Vec<u16> = vec![1, 2, 3];
        out.put(t).unwrap();
        assert_eq!(out, [0x00, 0x01, 0x00, 0x02, 0x00, 0x03]);
    }

    #[test]
    fn ser_counted() {
        let mut out = vec![];
        let t: Counted<u16> = vec![10, 11].into();
        out.put(t).unwrap();
        assert_eq!(out, [0x00, 0x02, 0x00, 0x0a, 0x00, 0x0b]);
    }

    #[test]
    fn de_counted() {
        let mut rc = ReaderContext::new(vec![0x00, 0x02, 0x00, 0x0a, 0x00, 0x0b]);
        let t: Counted<u16> = rc.de().unwrap();
        assert_eq!(t[0], 10);
        assert_eq!(t, vec![10, 11]);
    }

    #[test]
    fn ser_tag() {
        let t = Tag::from_raw("GSUB").unwrap();
        let mut out = vec![];
        out.put(t).unwrap();
        assert_eq!(out, [0x47, 0x53, 0x55, 0x42]);
    }

    #[test]
    fn de_tag() {
        let mut rc = ReaderContext::new(vec![0x47, 0x53, 0x55, 0x42]);
        let t: Tag = rc.de().unwrap();
        assert_eq!(t.as_str(), "GSUB");
    }

    // use otspec_macros::{Deserialize, Serialize};

    // #[derive(Serialize, Deserialize, Debug, PartialEq)]
    // struct TestStruct {
    //     test1: uint16,
    //     test2: uint16,
    // }

    // #[test]
    // fn ser_struct() {
    //     let mut out = vec![];
    //     let t = TestStruct {
    //         test1: 10,
    //         test2: 11,
    //     };
    //     out.put(t).unwrap();
    //     assert_eq!(out, [0x00, 0x0a, 0x00, 0x0b]);
    // }

    // #[test]
    // fn de_struct() {
    //     let mut rc = ReaderContext::new(vec![0x00, 0x0a, 0x00, 0x0b]);
    //     let t: TestStruct = rc.de().unwrap();
    //     assert_eq!(
    //         t,
    //         TestStruct {
    //             test1: 10,
    //             test2: 11
    //         }
    //     );
    // }

    // #[derive(Serialize, Deserialize, Debug, PartialEq)]
    // struct TestCounted {
    //     t: Counted<u16>,
    // }

    // #[derive(Serialize, Deserialize, Debug, PartialEq)]
    // struct TestCounted2 {
    //     t0: u32,
    //     t1: Counted<u16>,
    //     t2: u16,
    //     t3: Counted<TestCounted>,
    // }

    // #[test]
    // fn serde_interface() {
    //     let c1a = TestCounted {
    //         t: vec![0xaa, 0xbb, 0xcc].into(),
    //     };
    //     let c1b = TestCounted {
    //         t: vec![0xdd, 0xee].into(),
    //     };
    //     let c2 = TestCounted2 {
    //         t0: 0x01020304,
    //         t1: vec![0x10, 0x20].into(),
    //         t2: 0x1,
    //         t3: vec![c1a, c1b].into(),
    //     };
    //     let binary_c2 = vec![
    //         0x01, 0x02, 0x03, 0x04, /* t0 */
    //         0x00, 0x02, /* count */
    //         0x00, 0x10, 0x00, 0x20, /* t1 */
    //         0x00, 0x01, /* t2 */
    //         0x00, 0x02, /* count */
    //         0x00, 0x03, /* c1a count */
    //         0x00, 0xaa, 0x00, 0xbb, 0x00, 0xcc, /* c1a */
    //         0x00, 0x02, /* c1b count */
    //         0x00, 0xdd, 0x00, 0x0ee, /* c1b*/
    //     ];
    //     assert_eq!(ser::to_bytes(&c2).unwrap(), binary_c2);
    //     assert_eq!(de::from_bytes::<TestCounted2>(&binary_c2).unwrap(), c2);
    // }

    // use otspec_macros::tables;
    // tables!(hhea {
    //     uint16 majorVersion
    //     uint16 minorVersion
    //     FWORD ascender
    //     FWORD descender
    //     FWORD lineGap
    //     UFWORD  advanceWidthMax
    //     FWORD   minLeftSideBearing
    //     FWORD   minRightSideBearing
    //     FWORD   xMaxExtent
    //     int16   caretSlopeRise
    //     int16   caretSlopeRun
    //     int16   caretOffset
    //     int16   reserved0
    //     int16   reserved1
    //     int16   reserved2
    //     int16   reserved3
    //     int16   metricDataFormat
    //     uint16  numberOfHMetrics
    // });
    // #[test]
    // fn hhea_ser() {
    //     let fhhea = hhea {
    //         majorVersion: 1,
    //         minorVersion: 0,
    //         ascender: 705,
    //         descender: -180,
    //         lineGap: 0,
    //         advanceWidthMax: 1311,
    //         minLeftSideBearing: -382,
    //         minRightSideBearing: -382,
    //         xMaxExtent: 1245,
    //         caretSlopeRise: 1,
    //         caretSlopeRun: 0,
    //         caretOffset: 0,
    //         reserved0: 0,
    //         reserved1: 0,
    //         reserved2: 0,
    //         reserved3: 0,
    //         metricDataFormat: 0,
    //         numberOfHMetrics: 1117,
    //     };
    //     let binary_hhea = vec![
    //         0x00, 0x01, 0x00, 0x00, 0x02, 0xc1, 0xff, 0x4c, 0x00, 0x00, 0x05, 0x1f, 0xfe, 0x82,
    //         0xfe, 0x82, 0x04, 0xdd, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    //         0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x5d,
    //     ];
    //     assert_eq!(ser::to_bytes(&fhhea).unwrap(), binary_hhea);
    // }

    // #[test]
    // fn hhea_de() {
    //     let fhhea = hhea {
    //         majorVersion: 1,
    //         minorVersion: 0,
    //         ascender: 705,
    //         descender: -180,
    //         lineGap: 0,
    //         advanceWidthMax: 1311,
    //         minLeftSideBearing: -382,
    //         minRightSideBearing: -382,
    //         xMaxExtent: 1245,
    //         caretSlopeRise: 1,
    //         caretSlopeRun: 0,
    //         caretOffset: 0,
    //         reserved0: 0,
    //         reserved1: 0,
    //         reserved2: 0,
    //         reserved3: 0,
    //         metricDataFormat: 0,
    //         numberOfHMetrics: 1117,
    //     };
    //     let binary_hhea = vec![
    //         0x00, 0x01, 0x00, 0x00, 0x02, 0xc1, 0xff, 0x4c, 0x00, 0x00, 0x05, 0x1f, 0xfe, 0x82,
    //         0xfe, 0x82, 0x04, 0xdd, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    //         0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x5d,
    //     ];
    //     let deserialized: hhea = de::from_bytes(&binary_hhea).unwrap();
    //     assert_eq!(deserialized, fhhea);
    // }
}
