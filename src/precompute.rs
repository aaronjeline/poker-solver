use crate::cards::Card;
use crate::hands::*;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::collections::HashMap;
use std::io::{Read, Write};

pub struct ScoreTable(HashMap<Hand, TableEntry>);

impl ScoreTable {
    pub fn score(&self, hand: &Hand) -> TableEntry {
        *self.0.get(hand).unwrap()
    }
}

pub fn load_table(mut file: impl Read) -> std::io::Result<ScoreTable> {
    let mut table = HashMap::with_capacity(ALL_HANDS);
    let mut v = Vec::with_capacity(buffer_size());
    file.read_to_end(&mut v)?;
    let mut bs = Bytes::from_owner(v);
    for _ in 0..ALL_HANDS {
        let next = Entry::deserialize(&mut bs);
        table.insert(next.hand, next.into());
    }

    Ok(ScoreTable(table))
}

pub fn precompute(mut output: impl Write) -> std::io::Result<()> {
    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    eprintln!("  Precomputing poker hand lookup table");
    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    let mut buffer = BytesMut::with_capacity(buffer_size());
    let total = ALL_HANDS;

    for (i, hand) in all_hands().enumerate() {
        let e = hand.score();
        e.serialize(&mut buffer);

        // Update progress every 100k hands
        if i % 100_000 == 0 {
            let percent = (i as f64 / total as f64) * 100.0;
            eprint!(
                "\r  ⚡ Progress: {}/{} ({:.1}%)",
                format_number(i),
                format_number(total),
                percent
            );
        }
    }

    eprintln!(
        "\r  ✓ Computed: {}/{} (100.0%)  ",
        format_number(total),
        format_number(total)
    );
    eprintln!();
    eprintln!("  Writing to disk...");

    let buffer = buffer.freeze();
    output.write_all(&buffer)?;

    eprintln!("  ✓ Wrote {} bytes", format_number(buffer.len()));
    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    eprintln!("  Done!");
    eprintln!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    Ok(())
}

fn format_number(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

pub const fn buffer_size() -> usize {
    Entry::size() * ALL_HANDS
}

#[derive(Default, PartialEq, Eq)]
pub struct Entry {
    pub hand: Hand,
    pub rank: u8,
    pub hi: u8,
}

impl Into<TableEntry> for Entry {
    fn into(self) -> TableEntry {
        TableEntry {
            rank: self.rank,
            hi: self.hi,
        }
    }
}

#[derive(Default, PartialEq, Eq, Clone, Copy)]
pub struct TableEntry {
    pub rank: u8,
    pub hi: u8,
}

impl PartialOrd for TableEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match self.rank.cmp(&other.rank) {
            std::cmp::Ordering::Equal => Some(self.hi.cmp(&other.hi)),
            ord => Some(ord),
        }
    }
}

impl Ord for TableEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl Entry {
    pub const fn size() -> usize {
        HAND_SIZE + 2
    }

    pub fn serialize(&self, bytes: &mut BytesMut) {
        bytes.put_slice(self.hand.as_slice());
        bytes.put_u8(self.rank);
        bytes.put_u8(self.hi);
    }

    pub fn deserialize(bytes: &mut Bytes) -> Self {
        let mut e = Entry::default();
        for i in 0..e.hand.0.len() {
            e.hand.0[i] = Card(bytes.get_u8());
        }
        e.rank = bytes.get_u8();
        e.hi = bytes.get_u8();
        e
    }
}
