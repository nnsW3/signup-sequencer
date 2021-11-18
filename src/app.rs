use crate::{
    hash::Hash,
    mimc_tree::MimcTree,
    server::Error,
    solidity::{
        initialize_semaphore, parse_identity_commitments, ContractSigner, SemaphoreContract,
    },
};
use ethers::prelude::*;
use eyre::{eyre, Result as EyreResult};
use hyper::{Body, Response};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    sync::atomic::{AtomicUsize, Ordering},
};
use structopt::StructOpt;
use tokio::sync::RwLock;

pub const COMMITMENTS_FILE: &str = "./commitments.json";

#[derive(Debug, PartialEq, StructOpt)]
pub struct Options {
    /// Number of layers in the tree. Defaults to 21 to match Semaphore.sol
    /// defaults.
    #[structopt(long, env, default_value = "21")]
    pub tree_depth: usize,

    /// Initial value of the Merkle tree leaves. Defaults to the initial value
    /// in Semaphore.sol.
    #[structopt(
        long,
        env,
        default_value = "1c4823575d154474ee3e5ac838d002456a815181437afd14f126da58a9912bbe"
    )]
    pub initial_leaf: Hash,
}

pub struct App {
    merkle_tree:        RwLock<MimcTree>,
    last_leaf:          AtomicUsize,
    signer:             ContractSigner,
    semaphore_contract: SemaphoreContract,
}

impl App {
    pub async fn new(options: Options) -> EyreResult<Self> {
        let (signer, semaphore) = initialize_semaphore().await?;
        let mut merkle_tree = MimcTree::new(options.tree_depth, options.initial_leaf);
        let last_leaf = parse_identity_commitments(&mut merkle_tree, semaphore.clone()).await?;
        Ok(Self {
            merkle_tree: RwLock::new(merkle_tree),
            last_leaf: AtomicUsize::new(last_leaf),
            signer,
            semaphore_contract: semaphore,
        })
    }

    pub async fn insert_identity(&self, commitment: &Hash) -> Result<Response<Body>, Error> {
        {
            let mut merkle_tree = self.merkle_tree.write().await;
            let last_leaf = self.last_leaf.fetch_add(1, Ordering::AcqRel);

            // TODO: Error handling
            merkle_tree.set(last_leaf, *commitment);
            let num = self.signer.get_block_number().await.map_err(|e| eyre!(e))?;
            serde_json::to_writer(
                &File::create(COMMITMENTS_FILE).map_err(|e| eyre!(e))?,
                &JsonCommitment {
                    last_block:  num.as_usize(),
                    commitments: merkle_tree.leaves()[..=last_leaf].to_vec(),
                },
            )?;
        }

        let tx = self.semaphore_contract.insert_identity(commitment.into());
        let pending_tx = self.signer.send_transaction(tx.tx, None).await.unwrap();
        let _receipt = pending_tx.await.map_err(|e| eyre!(e))?;
        // TODO: What does it mean if `_receipt` is None?
        Ok(Response::new("Insert Identity!\n".into()))
    }

    #[allow(clippy::unused_async)]
    pub async fn inclusion_proof(&self, commitment: &Hash) -> Result<Response<Body>, Error> {
        let merkle_tree = self.merkle_tree.read().await;
        let proof = merkle_tree
            .position(commitment)
            .map(|i| merkle_tree.proof(i));

        println!("Proof: {:?}", proof);
        // TODO handle commitment not found
        let response = "Inclusion Proof!\n"; // TODO: proof
        Ok(Response::new(response.into()))
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonCommitment {
    pub last_block:  usize,
    pub commitments: Vec<Hash>,
}

impl From<&Hash> for U256 {
    fn from(hash: &Hash) -> Self {
        Self::from_big_endian(hash.as_bytes_be())
    }
}

impl From<U256> for Hash {
    fn from(u256: U256) -> Self {
        let mut bytes = [0_u8; 32];
        u256.to_big_endian(&mut bytes);
        Self::from_bytes_be(bytes)
    }
}
