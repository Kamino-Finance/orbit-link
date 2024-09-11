use anchor_client::solana_sdk::commitment_config::CommitmentLevel;
use async_trait::async_trait;
use serde_json::json;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_client::rpc_config::RpcSendTransactionConfig;
use solana_client::rpc_request::RpcRequest;
use solana_client::rpc_response::RpcPrioritizationFee;
use tracing::{debug, trace};

use super::*;
use crate::Result;

pub struct CompositeRpcClient {
    rpc_client: RpcClient,
    sender_client: RpcClient,
}

impl CompositeRpcClient {
    pub fn new(
        rpc_client_url: String,
        sender_client_url: String,
        commitment_config: CommitmentConfig,
    ) -> Self {
        let rpc_client = RpcClient::new_with_commitment(rpc_client_url.clone(), commitment_config);
        let sender_client =
            RpcClient::new_with_commitment(sender_client_url.clone(), commitment_config);
        Self {
            rpc_client,
            sender_client,
        }
    }
}

#[async_trait]
impl AsyncClient for CompositeRpcClient {
    async fn simulate_transaction(
        &self,
        transaction: &VersionedTransaction,
    ) -> Result<RpcSimulateTransactionResult> {
        AsyncClient::simulate_transaction(&self.rpc_client, transaction).await
    }

    async fn send_transaction_no_retry(
        &self,
        transaction: &VersionedTransaction,
    ) -> Result<Signature> {
        let sim_res = <RpcClient>::simulate_transaction(&self.rpc_client, transaction).await?;
        if let Some(ref err) = sim_res.value.err {
            debug!("Transaction simulation failed: {:#?}", sim_res);
            return Err(solana_client::client_error::ClientError::from(
                solana_client::client_error::ClientErrorKind::TransactionError(err.clone()),
            )
            .into());
        }
        let sig = <RpcClient>::send_transaction_with_config(
            &self.sender_client,
            transaction,
            RpcSendTransactionConfig {
                skip_preflight: true,
                preflight_commitment: Some(CommitmentLevel::Finalized),
                max_retries: Some(0),
                ..RpcSendTransactionConfig::default()
            },
        )
        .await?;
        trace!("Sent transaction no retry: {:?}", sig);
        Ok(sig)
    }

    async fn send_transaction(&self, transaction: &VersionedTransaction) -> Result<Signature> {
        let sim_res = <RpcClient>::simulate_transaction(&self.rpc_client, transaction).await?;
        if let Some(ref err) = sim_res.value.err {
            debug!("Transaction simulation failed: {:#?}", sim_res);
            return Err(solana_client::client_error::ClientError::from(
                solana_client::client_error::ClientErrorKind::TransactionError(err.clone()),
            )
            .into());
        }
        let sig = <RpcClient>::send_transaction_with_config(
            &self.sender_client,
            transaction,
            RpcSendTransactionConfig {
                skip_preflight: true,
                preflight_commitment: Some(CommitmentLevel::Finalized),
                max_retries: None, // Let the rpc handle retries til blockhash expiration
                ..RpcSendTransactionConfig::default()
            },
        )
        .await?;
        trace!("Sent transaction: {:?}", sig);
        Ok(sig)
    }

    async fn send_transaction_no_retry_no_preflight(
        &self,
        transaction: &VersionedTransaction,
    ) -> Result<Signature> {
        let sig = <RpcClient>::send_transaction_with_config(
            &self.sender_client,
            transaction,
            RpcSendTransactionConfig {
                skip_preflight: true,
                preflight_commitment: Some(CommitmentLevel::Finalized),
                max_retries: Some(0),
                ..RpcSendTransactionConfig::default()
            },
        )
        .await?;
        trace!("Sent transaction no retry: {:?}", sig);
        Ok(sig)
    }

    async fn get_minimum_balance_for_rent_exemption(&self, data_len: usize) -> Result<u64> {
        AsyncClient::get_minimum_balance_for_rent_exemption(&self.rpc_client, data_len).await
    }

    async fn get_signature_statuses(
        &self,
        signatures: &[Signature],
    ) -> Result<Vec<Option<TransactionStatus>>> {
        AsyncClient::get_signature_statuses(&self.rpc_client, signatures).await
    }

    async fn get_latest_blockhash(&self) -> Result<Hash> {
        AsyncClient::get_latest_blockhash(&self.rpc_client).await
    }

    async fn is_blockhash_valid(&self, blockhash: &Hash) -> Result<bool> {
        AsyncClient::is_blockhash_valid(&self.rpc_client, blockhash).await
    }

    async fn get_balance(&self, pubkey: &Pubkey) -> Result<u64> {
        AsyncClient::get_balance(&self.rpc_client, pubkey).await
    }

    async fn get_account(&self, pubkey: &Pubkey) -> Result<Account> {
        AsyncClient::get_account(&self.rpc_client, pubkey).await
    }

    async fn get_multiple_accounts(&self, pubkeys: &[Pubkey]) -> Result<Vec<Option<Account>>> {
        AsyncClient::get_multiple_accounts(&self.rpc_client, pubkeys).await
    }

    async fn get_program_accounts_with_size_and_discriminator(
        &self,
        program_id: &Pubkey,
        size: u64,
        discriminator: ClientDiscriminator,
    ) -> Result<Vec<(Pubkey, Account)>> {
        AsyncClient::get_program_accounts_with_size_and_discriminator(
            &self.rpc_client,
            program_id,
            size,
            discriminator,
        )
        .await
    }

    async fn get_program_accounts_with_discriminator(
        &self,
        program_id: &Pubkey,
        discriminator: ClientDiscriminator,
    ) -> Result<Vec<(Pubkey, Account)>> {
        AsyncClient::get_program_accounts_with_discriminator(
            &self.rpc_client,
            program_id,
            discriminator,
        )
        .await
    }

    async fn get_slot_with_commitment(&self, commitment: CommitmentConfig) -> Result<Slot> {
        AsyncClient::get_slot_with_commitment(&self.rpc_client, commitment).await
    }

    async fn get_program_accounts_with_discriminator_and_filters(
        &self,
        program_id: &Pubkey,
        discriminator: ClientDiscriminator,
        filters: Vec<RpcFilterType>,
    ) -> Result<Vec<(Pubkey, Account)>> {
        AsyncClient::get_program_accounts_with_discriminator_and_filters(
            &self.rpc_client,
            program_id,
            discriminator,
            filters,
        )
        .await
    }

    async fn get_recommended_micro_lamport_fee(&self) -> Result<u64> {
        self.get_recommended_micro_lamport_fee_for_accounts(&[])
            .await
    }

    async fn get_recommended_micro_lamport_fee_for_accounts(
        &self,
        accounts: &[Pubkey],
    ) -> Result<u64> {
        // Composite RPC assume a triton RPC and implements https://docs.triton.one/chains/solana/improved-priority-fees-api
        let addresses: Vec<_> = accounts.iter().map(|address| address.to_string()).collect();
        let fees: Vec<RpcPrioritizationFee> = self
            .rpc_client
            .send(
                RpcRequest::GetRecentPrioritizationFees,
                json!([[addresses], {"percentile": 2500}]), // 25th percentile
            )
            .await?;
        trace!("Recent fees: {:#?}", fees);
        let fee = fees
            .into_iter()
            .fold(0, |acc, x| u64::max(acc, x.prioritization_fee));

        debug!("Selected fee: {}", fee);

        Ok(fee)
    }
}
