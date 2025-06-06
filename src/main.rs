use std::{
    fmt, fs,
    io::{stdout, Write},
    path::PathBuf,
};

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::{collections::HashMap, fmt::Debug};

const WINDOW_SIZE: usize = 8;

#[derive(PartialEq, Eq)]
enum Op<'a> {
    Copy(u32, u32),
    Add(&'a [u8]),
}

impl<'a> Op<'a> {
    #[inline]
    fn serialize_to(&self, out: &mut Vec<u8>) {
        match self {
            Op::Copy(offset, len) => {
                out.push(0x00);
                out.extend_from_slice(&offset.to_le_bytes());
                out.extend_from_slice(&len.to_le_bytes());
            }
            Op::Add(bytes) => {
                out.push(0x01);
                let len = bytes.len() as u32;
                out.extend_from_slice(&len.to_le_bytes());
                out.extend_from_slice(bytes);
            }
        }
    }

    #[inline]
    fn deserialize(input: &'a [u8]) -> Result<(Self, &'a [u8]), &'static str> {
        match input {
            [0x00, rest @ ..] => {
                if rest.len() < 8 {
                    return Err("unexpected EOF in Copy");
                }
                let offset = u32::from_le_bytes(rest[0..4].try_into().unwrap());
                let len = u32::from_le_bytes(rest[4..8].try_into().unwrap());
                Ok((Op::Copy(offset, len), &rest[8..]))
            }
            [0x01, rest @ ..] => {
                if rest.len() < 4 {
                    return Err("unexpected EOF in Add header");
                }
                let len = u32::from_le_bytes(rest[0..4].try_into().unwrap()) as usize;
                if rest.len() < len + 4 {
                    return Err("unexpected EOF in Add body");
                }
                Ok((Op::Add(&rest[4..4 + len]), &rest[4 + len..]))
            }
            _ => Err("invalid opcode"),
        }
    }

    fn deserialize_all(input: &'a [u8]) -> Result<Vec<Self>, &'static str> {
        let mut input = input;
        let mut ops = vec![];
        while !input.is_empty() {
            let (op, next) = Self::deserialize(input)?;
            input = next;
            ops.push(op);
        }
        Ok(ops)
    }
}

impl Debug for Op<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        enum Content<'a> {
            Text(&'a str),
            Bytes(&'a [u8]),
        }

        impl<'a> From<&'a [u8]> for Content<'a> {
            fn from(value: &'a [u8]) -> Self {
                match std::str::from_utf8(value) {
                    Ok(s) => Content::Text(s),
                    Err(_) => Content::Bytes(value),
                }
            }
        }

        impl<'a> fmt::Debug for Content<'a> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    Content::Text(s) => write!(f, "Text({:?})", s),
                    Content::Bytes(b) => {
                        write!(f, "Bytes(")?;
                        for (i, byte) in b.iter().enumerate() {
                            if i > 0 {
                                write!(f, " ")?;
                            }
                            write!(f, "{:02X}", byte)?;
                        }
                        write!(f, ")")
                    }
                }
            }
        }

        match self {
            Self::Copy(offset, len) => f.debug_tuple("Copy").field(offset).field(len).finish(),
            Self::Add(content) => f
                .debug_tuple("Add")
                .field(&Content::from(*content))
                .finish(),
        }
    }
}

fn build_hash_map(old: &[u8]) -> HashMap<&[u8], usize, ahash::RandomState> {
    let mut map =
        HashMap::with_capacity_and_hasher(old.len() / WINDOW_SIZE, ahash::RandomState::default());
    for i in 0..=old.len().saturating_sub(WINDOW_SIZE) {
        map.insert(&old[i..i + WINDOW_SIZE], i);
    }
    map
}

fn make_diff(old: &[u8], new: &[u8]) -> Vec<u8> {
    if new.len() < WINDOW_SIZE || old.len() < WINDOW_SIZE {
        let mut patch = vec![];
        Op::Add(new).serialize_to(&mut patch);
        return patch;
    }

    let mut patch = Vec::with_capacity(512);
    let map = build_hash_map(old);
    let mut i = 0;

    while i < new.len() {
        if i + WINDOW_SIZE <= new.len() {
            let window = &new[i..i + WINDOW_SIZE];
            if let Some(&pos) = map.get(window) {
                let len = simd_memcmp(&new[i..], &old[pos..]);
                let op = Op::Copy(pos as u32, len as u32);
                op.serialize_to(&mut patch);
                i += len;
                continue;
            }
        }

        let start = i;
        i += 1;
        while i < new.len() {
            if i + WINDOW_SIZE <= new.len() {
                let window = &new[i..i + WINDOW_SIZE];
                if map.contains_key(window) {
                    break;
                }
            }
            i += 1;
        }

        Op::Add(&new[start..i]).serialize_to(&mut patch);
    }

    patch
}

fn apply_patch(old: &[u8], patch: &[u8]) -> Result<Vec<u8>, &'static str> {
    let mut patch = patch;
    let mut out = Vec::new();

    while !patch.is_empty() {
        let (op, next_patch) = Op::deserialize(patch)?;
        patch = next_patch;
        match op {
            Op::Copy(offset, len) => {
                let start = offset as usize;
                let end = start + len as usize;
                if end > old.len() {
                    return Err("copy out of bounds");
                }
                out.extend_from_slice(&old[start..end]);
            }
            Op::Add(bytes) => {
                out.extend_from_slice(bytes);
            }
        }
    }

    Ok(out)
}

fn simd_memcmp(a: &[u8], b: &[u8]) -> usize {
    #[cfg(target_arch = "x86_64")]
    {
        if std::is_x86_feature_detected!("avx2") {
            return unsafe { simd_memcmp_avx2(a, b) };
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if std::arch::is_aarch64_feature_detected!("neon") {
            return unsafe { simd_memcmp_neon(a, b) };
        }
    }

    simd_memcmp_fallback(a, b)
}

#[cfg(target_arch = "x86_64")]
#[inline]
unsafe fn simd_memcmp_avx2(a: &[u8], b: &[u8]) -> usize {
    use std::arch::x86_64::*;
    let len = a.len().min(b.len());
    let mut i = 0;
    let pa = a.as_ptr();
    let pb = b.as_ptr();

    while i + 32 <= len {
        let chunk_a = _mm256_loadu_si256(pa.add(i) as *const __m256i);
        let chunk_b = _mm256_loadu_si256(pb.add(i) as *const __m256i);
        let cmp = _mm256_cmpeq_epi8(chunk_a, chunk_b);
        let mask = _mm256_movemask_epi8(cmp);
        if mask != -1 {
            let diff_index = (!mask as u32).trailing_zeros() as usize;
            return i + diff_index;
        }
        i += 32;
    }

    while i < len && a[i] == b[i] {
        i += 1;
    }

    i
}

#[cfg(target_arch = "aarch64")]
#[inline]
unsafe fn simd_memcmp_neon(a: &[u8], b: &[u8]) -> usize {
    use std::arch::aarch64::*;
    let len = a.len().min(b.len());
    let mut i = 0;
    let a_ptr = a.as_ptr();
    let b_ptr = b.as_ptr();

    while i + 16 <= len {
        let chunk_a = vld1q_u8(a_ptr.add(i));
        let chunk_b = vld1q_u8(b_ptr.add(i));
        let cmp = vceqq_u8(chunk_a, chunk_b);
        if vminvq_u8(cmp) != 0xFF {
            for j in 0..16 {
                if a[i + j] != b[i + j] {
                    return i + j;
                }
            }
        }
        i += 16;
    }

    while i < len && a[i] == b[i] {
        i += 1;
    }

    i
}

#[inline]
fn simd_memcmp_fallback(a: &[u8], b: &[u8]) -> usize {
    let len = a.len().min(b.len());
    let mut i = 0;
    while i < len && a[i] == b[i] {
        i += 1;
    }
    i
}

#[derive(Parser)]
#[command(name = "bindiff")]
#[command(about = "Create and apply binary patches", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create binary patch from two files and write to stdout
    Diff { old: PathBuf, new: PathBuf },
    /// Apply binary patch to a file and write result to stdout
    Patch { old: PathBuf, patch: PathBuf },
    /// Print patch opcodes
    Debug { patch: PathBuf },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Diff { old, new } => {
            let old = fs::read(old)?;
            let new = fs::read(new)?;
            let patch = make_diff(&old, &new);
            stdout().write_all(&patch)?;
        }
        Commands::Patch { old, patch } => {
            let old = fs::read(old)?;
            let patch = fs::read(patch)?;
            let new = apply_patch(&old, &patch).map_err(|e| anyhow::anyhow!("{e}"))?;
            stdout().write_all(&new)?;
        }
        Commands::Debug { patch } => {
            let patch = fs::read(patch)?;
            println!(
                "{:#?}",
                Op::deserialize_all(&patch).map_err(|e| anyhow::anyhow!("{e}"))?
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() {
        let digits = (0..100).into_iter().collect::<Vec<_>>();
        let tail = vec![0, 1];
        let shifted_digits = digits
            .clone()
            .into_iter()
            .skip(2)
            .chain(tail.iter().copied())
            .collect::<Vec<_>>();
        let baz = b"baz".to_vec();

        let cases: Vec<(Vec<u8>, Vec<u8>, _)> = vec![
            (digits.clone(), digits.clone(), Some(vec![Op::Copy(0, 100)])),
            (
                digits.clone(),
                shifted_digits.clone(),
                Some(vec![Op::Copy(2, 98), Op::Add(tail.as_slice())]),
            ),
            (vec![], vec![1, 2, 3, 4, 5, 6, 7, 8], None),
            (vec![], vec![1, 2, 3], None),
            (vec![], vec![], Some(vec![Op::Add(&[])])),
            (
                vec![1, 2, 3, 4, 5, 6, 7, 8],
                vec![],
                Some(vec![Op::Add(&[])]),
            ),
            (
                b"-foo-bar-hello-world".into(),
                b"hello-world-foo-bar-baz".into(),
                Some(vec![Op::Copy(9, 11), Op::Copy(0, 9), Op::Add(&baz)]),
            ),
            (
                b"just-swaps-with-no-adds".into(),
                b"-with-no-addsjust-swaps".into(),
                Some(vec![Op::Copy(10, 13), Op::Copy(0, 10)]),
            ),
        ];

        for (old, new, expected_ops) in cases {
            let patch = make_diff(&old, &new);
            let ops = Op::deserialize_all(&patch).unwrap();
            if let Some(expected) = expected_ops {
                assert_eq!(ops, expected);
            }
            let patched = apply_patch(&old, &patch).unwrap();
            assert_eq!(&patched, &new);
        }
    }
}
