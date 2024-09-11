use anchor_client::solana_sdk::commitment_config::CommitmentLevel;
use async_trait::async_trait;
use solana_account_decoder::UiAccountEncoding;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_client::rpc_config::{
    RpcAccountInfoConfig, RpcProgramAccountsConfig, RpcSendTransactionConfig,
};
use solana_client::rpc_filter::{Memcmp, MemcmpEncodedBytes, RpcFilterType};
use tracing::{debug, trace};

use super::*;
use crate::Result;

#[async_trait]
impl AsyncClient for RpcClient {
    async fn simulate_transaction(
        &self,
        transaction: &VersionedTransaction,
    ) -> Result<RpcSimulateTransactionResult> {
        <RpcClient>::simulate_transaction(self, transaction)
            .await
            .map_err(Into::into)
            .map(|response| response.value)
    }

    async fn send_transaction_no_retry(
        &self,
        transaction: &VersionedTransaction,
    ) -> Result<Signature> {
        let sim_res = <RpcClient>::simulate_transaction(self, transaction).await?;
        if let Some(ref err) = sim_res.value.err {
            debug!("Transaction simulation failed: {:#?}", sim_res);
            return Err(solana_client::client_error::ClientError::from(
                solana_client::client_error::ClientErrorKind::TransactionError(err.clone()),
            )
            .into());
        }
        <RpcClient>::send_transaction_with_config(
            self,
            transaction,
            RpcSendTransactionConfig {
                skip_preflight: true,
                preflight_commitment: Some(CommitmentLevel::Finalized),
                max_retries: Some(0),
                ..RpcSendTransactionConfig::default()
            },
        )
        .await
        .map_err(Into::into)
    }

    async fn send_transaction(&self, transaction: &VersionedTransaction) -> Result<Signature> {
        let sim_res = <RpcClient>::simulate_transaction(self, transaction).await?;
        if let Some(ref err) = sim_res.value.err {
            debug!("Transaction simulation failed: {:#?}", sim_res);
            return Err(solana_client::client_error::ClientError::from(
                solana_client::client_error::ClientErrorKind::TransactionError(err.clone()),
            )
            .into());
        }
        <RpcClient>::send_transaction_with_config(
            self,
            transaction,
            RpcSendTransactionConfig {
                skip_preflight: true,
                preflight_commitment: Some(CommitmentLevel::Finalized),
                max_retries: None, // Let the rpc handle retries til blockhash expiration
                ..RpcSendTransactionConfig::default()
            },
        )
        .await
        .map_err(Into::into)
    }

    async fn send_transaction_no_retry_no_preflight(
        &self,
        transaction: &VersionedTransaction,
    ) -> Result<Signature> {
        <RpcClient>::send_transaction_with_config(
            self,
            transaction,
            RpcSendTransactionConfig {
                skip_preflight: true,
                preflight_commitment: None,
                max_retries: Some(0),
                ..RpcSendTransactionConfig::default()
            },
        )
        .await
        .map_err(Into::into)
    }

    async fn get_minimum_balance_for_rent_exemption(&self, data_len: usize) -> Result<u64> {
        <RpcClient>::get_minimum_balance_for_rent_exemption(self, data_len)
            .await
            .map_err(Into::into)
    }

    async fn get_signature_statuses(
        &self,
        signatures: &[Signature],
    ) -> Result<Vec<Option<TransactionStatus>>> {
        <RpcClient>::get_signature_statuses(self, signatures)
            .await
            .map(|response| response.value)
            .map_err(Into::into)
    }

    async fn get_latest_blockhash(&self) -> Result<Hash> {
        <RpcClient>::get_latest_blockhash_with_commitment(self, CommitmentConfig::finalized())
            .await
            .map(|r| r.0)
            .map_err(Into::into)
    }

    async fn is_blockhash_valid(&self, blockhash: &Hash) -> Result<bool> {
        <RpcClient>::is_blockhash_valid(self, blockhash, CommitmentConfig::processed())
            .await
            .map_err(Into::into)
    }

    async fn get_balance(&self, pubkey: &Pubkey) -> Result<u64> {
        <RpcClient>::get_balance(self, pubkey)
            .await
            .map_err(Into::into)
    }

    async fn get_account(&self, pubkey: &Pubkey) -> Result<Account> {
        Ok(
            <RpcClient>::get_account_with_commitment(self, pubkey, CommitmentConfig::processed())
                .await?
                .value
                .ok_or_else(|| {
                    solana_client::client_error::ClientError::from(
                        solana_client::client_error::ClientErrorKind::Custom(format!(
                            "AccountNotFound: pubkey={pubkey}"
                        )),
                    )
                })?,
        )
    }

    async fn get_multiple_accounts(&self, pubkeys: &[Pubkey]) -> Result<Vec<Option<Account>>> {
        <RpcClient>::get_multiple_accounts(self, pubkeys)
            .await
            .map_err(Into::into)
    }

    async fn get_program_accounts_with_size_and_discriminator(
        &self,
        program_id: &Pubkey,
        size: u64,
        discriminator: ClientDiscriminator,
    ) -> Result<Vec<(Pubkey, Account)>> {
        let memcmp = RpcFilterType::Memcmp(Memcmp::new(
            0,
            MemcmpEncodedBytes::Bytes(discriminator.to_vec()),
        ));
        let config = RpcProgramAccountsConfig {
            filters: Some(vec![RpcFilterType::DataSize(size), memcmp]),
            account_config: RpcAccountInfoConfig {
                encoding: Some(UiAccountEncoding::Base64Zstd),
                ..RpcAccountInfoConfig::default()
            },
            ..RpcProgramAccountsConfig::default()
        };

        <RpcClient>::get_program_accounts_with_config(self, program_id, config)
            .await
            .map_err(Into::into)
    }

    async fn get_program_accounts_with_discriminator(
        &self,
        program_id: &Pubkey,
        discriminator: ClientDiscriminator,
    ) -> Result<Vec<(Pubkey, Account)>> {
        let memcmp = RpcFilterType::Memcmp(Memcmp::new(
            0,
            MemcmpEncodedBytes::Bytes(discriminator.to_vec()),
        ));
        let config = RpcProgramAccountsConfig {
            filters: Some(vec![memcmp]),
            account_config: RpcAccountInfoConfig {
                encoding: Some(UiAccountEncoding::Base64Zstd),
                ..RpcAccountInfoConfig::default()
            },
            ..RpcProgramAccountsConfig::default()
        };

        <RpcClient>::get_program_accounts_with_config(self, program_id, config)
            .await
            .map_err(Into::into)
    }

    async fn get_program_accounts_with_discriminator_and_filters(
        &self,
        program_id: &Pubkey,
        discriminator: ClientDiscriminator,
        filters: Vec<RpcFilterType>,
    ) -> Result<Vec<(Pubkey, Account)>> {
        let memcmp = RpcFilterType::Memcmp(Memcmp::new(
            0,
            MemcmpEncodedBytes::Bytes(discriminator.to_vec()),
        ));

        let mut all_filters = filters.clone();
        all_filters.push(memcmp);
        let config = RpcProgramAccountsConfig {
            filters: Some(filters),
            account_config: RpcAccountInfoConfig {
                encoding: Some(UiAccountEncoding::Base64Zstd),
                ..RpcAccountInfoConfig::default()
            },
            ..RpcProgramAccountsConfig::default()
        };

        <RpcClient>::get_program_accounts_with_config(self, program_id, config)
            .await
            .map_err(Into::into)
    }

    async fn get_slot_with_commitment(&self, commitment: CommitmentConfig) -> Result<Slot> {
        <RpcClient>::get_slot_with_commitment(self, commitment)
            .await
            .map_err(Into::into)
    }

    async fn get_recommended_micro_lamport_fee(&self) -> Result<u64> {
        self.get_recommended_micro_lamport_fee_for_accounts(&[])
            .await
    }

    async fn get_recommended_micro_lamport_fee_for_accounts(
        &self,
        accounts: &[Pubkey],
    ) -> Result<u64> {
        let fees = self.get_recent_prioritization_fees(accounts).await?;
        trace!("Recent fees: {:#?}", fees);
        let fee = fees
            .into_iter()
            .fold(0, |acc, x| u64::max(acc, x.prioritization_fee));

        debug!("Selected fee: {}", fee);

        Ok(fee)
    }
}
