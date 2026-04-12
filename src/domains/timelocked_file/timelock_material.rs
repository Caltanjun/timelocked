//! Defines the serialization and deserialization of the Timelock puzzle material
//! stored immediately after the cleartext header.

use std::io::{Read, Write};

use num_bigint::BigUint;

use crate::base::{Error, Result};

#[derive(Debug, Clone)]
pub struct TimelockPayloadMaterial {
    pub modulus_n: BigUint,
    pub base_a: BigUint,
    pub wrapped_key: [u8; 32],
}

pub fn write_timelock_material(
    writer: &mut impl Write,
    material: &TimelockPayloadMaterial,
) -> Result<()> {
    let n_bytes = material.modulus_n.to_bytes_be();
    let a_bytes = material.base_a.to_bytes_be();

    writer.write_all(&(n_bytes.len() as u32).to_le_bytes())?;
    writer.write_all(&n_bytes)?;
    writer.write_all(&(a_bytes.len() as u32).to_le_bytes())?;
    writer.write_all(&a_bytes)?;
    writer.write_all(&material.wrapped_key)?;
    Ok(())
}

pub fn read_timelock_material(reader: &mut impl Read) -> Result<TimelockPayloadMaterial> {
    let n_len = read_u32(reader)? as usize;
    if n_len == 0 {
        return Err(Error::InvalidFormat(
            "modulus length cannot be zero".to_string(),
        ));
    }
    let mut n_bytes = vec![0_u8; n_len];
    reader.read_exact(&mut n_bytes)?;

    let a_len = read_u32(reader)? as usize;
    if a_len == 0 {
        return Err(Error::InvalidFormat(
            "base length cannot be zero".to_string(),
        ));
    }
    let mut a_bytes = vec![0_u8; a_len];
    reader.read_exact(&mut a_bytes)?;

    let mut wrapped_key = [0_u8; 32];
    reader.read_exact(&mut wrapped_key)?;

    Ok(TimelockPayloadMaterial {
        modulus_n: BigUint::from_bytes_be(&n_bytes),
        base_a: BigUint::from_bytes_be(&a_bytes),
        wrapped_key,
    })
}

fn read_u32(reader: &mut impl Read) -> Result<u32> {
    let mut bytes = [0_u8; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}
