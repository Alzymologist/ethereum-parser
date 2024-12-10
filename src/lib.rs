#![no_std]
#![deny(unused_crate_dependencies)]

#[cfg(any(feature = "std", test))]
extern crate std;

#[cfg(all(not(feature = "std"), not(test)))]
extern crate alloc;

#[cfg(any(feature = "std", test))]
use std::vec::Vec;

#[cfg(all(not(feature = "std"), not(test)))]
use alloc::vec::Vec;

use external_memory_tools::{AddressableBuffer, BufferError, ExternalMemory};

#[derive(Debug, Eq, PartialEq)]
pub enum ParsedData {
    Byte(u8),
    List(Vec<ParsedData>),
    String(Vec<u8>),
}

#[derive(Debug, Eq, PartialEq)]
pub enum Error<E: ExternalMemory> {
    Buffer(BufferError<E>),
    NotWorking,
    SomeDataUnused { from: usize },
}

pub const BORDER_A: u8 = 0x80;
pub const BORDER_B: u8 = 0xb8;
pub const BORDER_C: u8 = 0xc0;
pub const BORDER_D: u8 = 0xf8;

pub fn decode_whole_blob<B, E>(data: &B, ext_memory: &mut E) -> Result<ParsedData, Error<E>>
where
    B: AddressableBuffer<E>,
    E: ExternalMemory,
{
    let mut position = 0;
    let parsed_data = decode_blob_portion_at_position(data, ext_memory, &mut position)?;
    if position < data.total_len() {
        Err(Error::SomeDataUnused { from: position })
    } else {
        Ok(parsed_data)
    }
}

pub fn decode_blob_portion_at_position<B, E>(
    data: &B,
    ext_memory: &mut E,
    position: &mut usize,
) -> Result<ParsedData, Error<E>>
where
    B: AddressableBuffer<E>,
    E: ExternalMemory,
{
    let current_byte = data
        .read_byte(ext_memory, *position)
        .map_err(Error::Buffer)?;
    *position += 1;

    match current_byte {
        a if (..BORDER_A).contains(&a) => Ok(ParsedData::Byte(a)),
        a if (BORDER_A..BORDER_B).contains(&a) => {
            let string_length = (a - BORDER_A) as usize;
            let slice = data
                .read_slice(ext_memory, *position, string_length)
                .map_err(Error::Buffer)?;
            *position += string_length;
            Ok(ParsedData::String(slice.as_ref().to_vec()))
        }
        a if (BORDER_B..BORDER_C).contains(&a) => {
            let string_length_info_length = (a + 1 - BORDER_B) as usize;

            let string_length_slice = data
                .read_slice(ext_memory, *position, string_length_info_length)
                .map_err(Error::Buffer)?;
            *position += string_length_info_length;

            let mut string_length_bytes = [0; 8];
            string_length_bytes[8 - string_length_info_length..8]
                .copy_from_slice(string_length_slice.as_ref());

            let string_length = u64::from_be_bytes(string_length_bytes) as usize;
            let slice = data
                .read_slice(ext_memory, *position, string_length)
                .map_err(Error::Buffer)?;
            *position += string_length;
            Ok(ParsedData::String(slice.as_ref().to_vec()))
        }
        a if (BORDER_C..BORDER_D).contains(&a) => {
            let list_length = (a - BORDER_C) as usize;
            let border_position = *position + list_length;
            let mut list_content: Vec<ParsedData> = Vec::new();

            let limited_data = data.limit_length(border_position).map_err(Error::Buffer)?;

            while *position < border_position {
                let parsed_data =
                    decode_blob_portion_at_position(&limited_data, ext_memory, position)?;
                list_content.push(parsed_data);
            }

            Ok(ParsedData::List(list_content))
        }
        a => {
            let list_length_info_length = (a + 1 - BORDER_D) as usize;

            let list_length_slice = data
                .read_slice(ext_memory, *position, list_length_info_length)
                .map_err(Error::Buffer)?;
            *position += list_length_info_length;

            let mut list_length_bytes = [0; 8];
            list_length_bytes[8 - list_length_info_length..8]
                .copy_from_slice(list_length_slice.as_ref());

            let list_length = u64::from_be_bytes(list_length_bytes) as usize;

            let border_position = *position + list_length;
            let mut list_content: Vec<ParsedData> = Vec::new();

            let limited_data = data.limit_length(border_position).map_err(Error::Buffer)?;

            while *position < border_position {
                let parsed_data =
                    decode_blob_portion_at_position(&limited_data, ext_memory, position)?;
                list_content.push(parsed_data);
            }

            Ok(ParsedData::List(list_content))
        }
    }
}

#[cfg(any(feature = "std", test))]
#[cfg(test)]
mod tests {
    use alloy_rlp::{Encodable, RlpEncodable};
    use std::{borrow::ToOwned, string::String, vec};

    use super::*;

    #[test]
    fn decode_1() {
        let hex_input = "0d";
        let bytes_input = hex::decode(hex_input).unwrap();
        let parsed = decode_whole_blob::<&[u8], ()>(&bytes_input.as_ref(), &mut ()).unwrap();
        assert_eq!(parsed, ParsedData::Byte(13));
    }

    #[test]
    fn decode_2() {
        let hex_input = "80";
        let bytes_input = hex::decode(hex_input).unwrap();
        let parsed = decode_whole_blob::<&[u8], ()>(&bytes_input.as_ref(), &mut ()).unwrap();
        assert_eq!(parsed, ParsedData::String(Vec::new()));
    }

    #[test]
    fn decode_3() {
        let mock_string = String::from("Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.");
        let mut buffer = Vec::<u8>::new();
        mock_string.encode(&mut buffer);

        let parsed = decode_whole_blob::<&[u8], ()>(&buffer.as_ref(), &mut ()).unwrap();
        assert_eq!(parsed, ParsedData::String(mock_string.into_bytes()));
    }

    #[test]
    fn decode_4() {
        #[derive(RlpEncodable)]
        struct MockStruct {
            a: String,
            b: String,
            c: String,
            d: [u8; 20],
        }

        let string1 = String::from("string1");
        let string2 = String::from("string2");
        let string3 = String::from("string3");
        let array = [144; 20];

        let mock_struct = MockStruct {
            a: string1.to_owned(),
            b: string2.to_owned(),
            c: string3.to_owned(),
            d: array,
        };
        let mut buffer = Vec::<u8>::new();
        mock_struct.encode(&mut buffer);

        let parsed = decode_whole_blob::<&[u8], ()>(&buffer.as_ref(), &mut ()).unwrap();
        assert_eq!(
            parsed,
            ParsedData::List(vec![
                ParsedData::String(string1.into_bytes()),
                ParsedData::String(string2.into_bytes()),
                ParsedData::String(string3.into_bytes()),
                ParsedData::String(vec![144; 20])
            ])
        );
    }

    #[test]
    fn decode_5() {
        #[derive(RlpEncodable)]
        struct MockStruct {
            long_array1: [u8; 2000],
            long_array2: [u8; 1000],
        }

        let long_array1 = [15; 2000];
        let long_array2 = [144; 1000];
        let mock_struct = MockStruct {
            long_array1,
            long_array2,
        };
        let mut buffer = Vec::<u8>::new();
        mock_struct.encode(&mut buffer);

        let parsed = decode_whole_blob::<&[u8], ()>(&buffer.as_ref(), &mut ()).unwrap();
        assert_eq!(
            parsed,
            ParsedData::List(vec![
                ParsedData::String(long_array1.to_vec()),
                ParsedData::String(long_array2.to_vec())
            ])
        );
    }

    #[test]
    fn decode_err_1() {
        let hex_input = "8080";
        let bytes_input = hex::decode(hex_input).unwrap();
        let parsed_err =
            decode_whole_blob::<&[u8], ()>(&bytes_input.as_ref(), &mut ()).unwrap_err();
        assert_eq!(parsed_err, Error::SomeDataUnused { from: 1 });
    }
}
