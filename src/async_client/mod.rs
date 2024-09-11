#[cfg(feature = "banks-client")]
pub mod banks_client;
#[cfg(feature = "rpc-client")]
pub mod composite_rpc_client;
#[cfg(feature = "rpc-client")]
pub mod rpc_client;

use anchor_client::{
    solana_client::rpc_response::RpcSimulateTransactionResult,
    solana_sdk::{
        account::Account, clock::Slot, commitment_config::CommitmentConfig, hash::Hash,
        pubkey::Pubkey, signature::Signature, transaction::VersionedTransaction,
    },
};
use async_trait::async_trait;
use solana_client::rpc_filter::RpcFilterType;
use solana_transaction_status::TransactionStatus;

use crate::Result;

pub enum ClientDiscriminator {
    Byte(u8),
    Bytes([u8; 8]),
}

impl ClientDiscriminator {
    pub fn to_vec(&self) -> Vec<u8> {
        match self {
            ClientDiscriminator::Byte(b) => vec![*b],
            ClientDiscriminator::Bytes(b) => b.to_vec(),
        }
    }
}

#[async_trait]
pub trait AsyncClient: Sync + Send {
    async fn simulate_transaction(
        &self,
        transaction: &VersionedTransaction,
    ) -> Result<RpcSimulateTransactionResult>;

    async fn send_transaction(&self, transaction: &VersionedTransaction) -> Result<Signature>;

    async fn send_transaction_no_retry(
        &self,
        transaction: &VersionedTransaction,
    ) -> Result<Signature>;

    async fn send_transaction_no_retry_no_preflight(
        &self,
        transaction: &VersionedTransaction,
    ) -> Result<Signature>;

    async fn get_signature_statuses(
        &self,
        signatures: &[Signature],
    ) -> Result<Vec<Option<TransactionStatus>>>;

    async fn get_latest_blockhash(&self) -> Result<Hash>;

    async fn is_blockhash_valid(&self, blockhash: &Hash) -> Result<bool>;

    async fn get_minimum_balance_for_rent_exemption(&self, data_len: usize) -> Result<u64>;

    async fn get_balance(&self, pubkey: &Pubkey) -> Result<u64>;

    async fn get_account(&self, pubkey: &Pubkey) -> Result<Account>;

    async fn get_multiple_accounts(&self, pubkeys: &[Pubkey]) -> Result<Vec<Option<Account>>>;

    async fn get_program_accounts_with_size_and_discriminator(
        &self,
        program_id: &Pubkey,
        size: u64,
        discriminator: ClientDiscriminator,
    ) -> Result<Vec<(Pubkey, Account)>>;

    async fn get_program_accounts_with_discriminator(
        &self,
        program_id: &Pubkey,
        discriminator: ClientDiscriminator,
    ) -> Result<Vec<(Pubkey, Account)>>;

    async fn get_program_accounts_with_discriminator_and_filters(
        &self,
        program_id: &Pubkey,
        discriminator: ClientDiscriminator,
        filters: Vec<RpcFilterType>,
    ) -> Result<Vec<(Pubkey, Account)>>;

    async fn get_slot_with_commitment(&self, commitment: CommitmentConfig) -> Result<Slot>;

    async fn get_recommended_micro_lamport_fee(&self) -> Result<u64>;

    async fn get_recommended_micro_lamport_fee_for_accounts(
        &self,
        accounts_pk: &[Pubkey],
    ) -> Result<u64>;
}
