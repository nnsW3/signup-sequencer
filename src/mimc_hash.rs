//! Hash function compatible with Semaphore's Merkle tree hash function
//!
//! See <https://github.com/appliedzkp/semaphore/blob/master/circuits/circom/semaphore-base.circom#L10>
//! See <https://github.com/kobigurk/circomlib/blob/4284dc1ef984a204db08864f5da530c97f9376ef/circuits/mimcsponge.circom>
//! See <https://github.com/iden3/circomlibjs/blob/main/src/mimcsponge.js>

use ethers::utils::keccak256;
use once_cell::sync::Lazy;
use zkp_u256::{Montgomery, U256};

const NUM_ROUNDS: usize = 220;

static MODULUS: Lazy<U256> = Lazy::new(|| {
    U256::from_decimal_str(
        "21888242871839275222246405745257275088548364400416034343698204186575808495617",
    )
    .unwrap()
});

static ROUND_CONSTANTS: Lazy<[U256; NUM_ROUNDS]> = Lazy::new(|| {
    const SEED: &str = "mimcsponge";
    let mut result = [U256::ZERO; NUM_ROUNDS];
    let mut bytes = keccak256(SEED.as_bytes());
    for constant in result[1..NUM_ROUNDS - 1].iter_mut() {
        bytes = keccak256(&bytes);
        *constant = U256::from_bytes_be(&bytes);
        *constant %= &*MODULUS;
    }
    result
});

/// See <https://github.com/iden3/circomlibjs/blob/main/src/mimcsponge.js#L44>
fn hash(left: &U256, right: &U256) -> (U256, U256) {
    let mut left = left % &*MODULUS;
    let mut right = right % &*MODULUS;
    for (i, round_constant) in ROUND_CONSTANTS.iter().enumerate() {
        // Modulus is less than 2**252, so addition doesn't overflow
        let t = (&left + round_constant) % &*MODULUS;
        let t2 = t.mulmod(&t, &*MODULUS);
        let t4 = t2.mulmod(&t2, &*MODULUS);
        let t5 = t.mulmod(&t4, &*MODULUS);
        let temp = (&right + t5) % &*MODULUS;
        if i == NUM_ROUNDS - 1 {
            right = temp;
        } else {
            right = left.clone();
            left = temp;
        }
    }
    (left, right)
}

#[cfg(test)]
pub mod test {
    use super::*;

    #[test]
    fn test_round_constants() {
        // See <https://github.com/kobigurk/circomlib/blob/4284dc1ef984a204db08864f5da530c97f9376ef/circuits/mimcsponge.circom#L44>
        assert_eq!(ROUND_CONSTANTS[0], U256::ZERO);
        assert_eq!(
            ROUND_CONSTANTS[1],
            U256::from_decimal_str(
                "7120861356467848435263064379192047478074060781135320967663101236819528304084"
            )
            .unwrap()
        );
        assert_eq!(
            ROUND_CONSTANTS[2],
            U256::from_decimal_str(
                "5024705281721889198577876690145313457398658950011302225525409148828000436681"
            )
            .unwrap()
        );
        assert_eq!(
            ROUND_CONSTANTS[218],
            U256::from_decimal_str(
                "2119542016932434047340813757208803962484943912710204325088879681995922344971"
            )
            .unwrap()
        );
        assert_eq!(ROUND_CONSTANTS[219], U256::ZERO);
    }

    #[test]
    fn test_inner_hash() {
        let left = U256::ONE;
        let right = U256::ZERO;
        let (left, right) = hash(&left, &right);
        assert_eq!(
            left,
            U256::from_decimal_str(
                "8792246410719720074073794355580855662772292438409936688983564419486782556587"
            )
            .unwrap()
        );
        assert_eq!(
            right,
            U256::from_decimal_str(
                "7326554092124867281481480523863654579712861994895051796475958890524736238844"
            )
            .unwrap()
        );
        let left = left + U256::from(2);
        let (left, right) = hash(&left, &right);
        assert_eq!(
            left,
            U256::from_decimal_str(
                "19814528709687996974327303300007262407299502847885145507292406548098437687919"
            )
            .unwrap()
        );
        assert_eq!(
            right,
            U256::from_decimal_str(
                "3888906192024793285683241274210746486868893421288515595586335488978789653213"
            )
            .unwrap()
        );
    }
}

#[cfg(feature = "bench")]
pub mod bench {
    use criterion::Criterion;

    pub fn group(criterion: &mut Criterion) {}
}
