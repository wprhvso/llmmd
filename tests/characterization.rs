mod common;

use common::{actions, actions_chunked, corpus_seeded};

const SEED: u64 = 0x0C0D_E515;
const DOCUMENTS: usize = 512;
const CHUNK_SIZES: &[usize] = &[0, 1, 3, 7, 64];

const RECORDED: &str = include_str!("snapshots/action_stream.txt");

fn absorb(hash: u64, bytes: &[u8]) -> u64 {
    let mut hash = hash;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn fingerprint(document: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for &size in CHUNK_SIZES {
        let stream = if size == 0 {
            actions(document)
        } else {
            actions_chunked(document, size)
        };
        hash = absorb(hash, format!("{stream:?}").as_bytes());
    }
    format!("{hash:016x}")
}

#[test]
fn the_action_stream_matches_the_recorded_fingerprints() {
    let documents = corpus_seeded(SEED, DOCUMENTS);
    let actual: Vec<String> = documents.iter().map(|d| fingerprint(d)).collect();
    let recorded: Vec<&str> = RECORDED.lines().filter(|line| !line.is_empty()).collect();

    assert_eq!(
        recorded.len(),
        actual.len(),
        "tests/snapshots/action_stream.txt is out of date, it should read:\n{}",
        actual.join("\n")
    );

    for (index, (got, want)) in actual.iter().zip(recorded).enumerate() {
        assert_eq!(
            got,
            want,
            "the action stream changed for document {index}: {:?}",
            documents.get(index)
        );
    }
}
