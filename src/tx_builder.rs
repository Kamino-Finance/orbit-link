use anchor_client::{
    anchor_lang::{InstructionData, ToAccountMetas},
    solana_sdk::{
        address_lookup_table_account::AddressLookupTableAccount,
        compute_budget::ComputeBudgetInstruction, instruction::Instruction, message::Message,
        pubkey::Pubkey, signer::Signer, transaction::VersionedTransaction,
    },
};
use base64::engine::{general_purpose::STANDARD as BS64, Engine};
use tracing::{debug, trace, warn};

use crate::{errors, OrbitLink, Result};

pub const DEFAULT_IX_BUDGET: u32 = 200_000;
pub const MAX_IX_BUDGET: u32 = 1_400_000;

#[derive(Clone)]
pub struct TxBuilder<'link, T, S>
where
    T: crate::async_client::AsyncClient,
    S: Signer,
{
    instructions: Vec<Instruction>,
    lookup_tables: Vec<AddressLookupTableAccount>,
    total_budget: u32,
    link: &'link OrbitLink<T, S>,
}

impl<'link, T, S> TxBuilder<'link, T, S>
where
    T: crate::async_client::AsyncClient,
    S: Signer,
{
    pub fn new(link: &'link OrbitLink<T, S>) -> Self {
        TxBuilder {
            instructions: vec![],
            lookup_tables: vec![],
            total_budget: 0,
            link,
        }
    }

    pub fn add_lookup_table(mut self, lookup_table: AddressLookupTableAccount) -> Self {
        self.lookup_tables.push(lookup_table);
        self
    }

    pub fn add_ix_with_budget(mut self, instruction: Instruction, budget: u32) -> Self {
        self.instructions.push(instruction);
        self.total_budget += budget;
        self
    }

    pub fn add_ixs_with_budget(
        mut self,
        instructions: impl IntoIterator<Item = (Instruction, u32)>,
    ) -> Self {
        self.instructions
            .extend(instructions.into_iter().map(|(instruction, budget)| {
                self.total_budget += budget;
                instruction
            }));
        self
    }

    pub fn add_anchor_ix_with_budget(
        mut self,
        program_id: &Pubkey,
        accounts: impl ToAccountMetas,
        args: impl InstructionData,
        budget: u32,
    ) -> Self {
        self.instructions.push(Instruction {
            program_id: *program_id,
            data: args.data(),
            accounts: accounts.to_account_metas(None),
        });
        self.total_budget += budget;
        self
    }

    pub fn add_ix(self, instruction: Instruction) -> Self {
        self.add_ix_with_budget(instruction, DEFAULT_IX_BUDGET)
    }

    pub fn add_ixs(self, instructions: impl IntoIterator<Item = Instruction>) -> Self {
        let budgeted_instructions = instructions
            .into_iter()
            .map(|instruction| (instruction, DEFAULT_IX_BUDGET));
        self.add_ixs_with_budget(budgeted_instructions)
    }

    pub fn add_anchor_ix(
        self,
        program_id: &Pubkey,
        accounts: impl ToAccountMetas,
        args: impl InstructionData,
    ) -> Self {
        self.add_anchor_ix_with_budget(program_id, accounts, args, DEFAULT_IX_BUDGET)
    }

    pub async fn build(self, extra_signers: &[&dyn Signer]) -> Result<VersionedTransaction> {
        self.link
            .create_tx_with_extra_lookup_tables(
                &self.instructions,
                extra_signers,
                &self.lookup_tables,
            )
            .await
    }

    fn get_budget_ix(&self) -> Option<Instruction> {
        if self.total_budget > 200_000 || self.instructions.len() > 1 {
            Some(ComputeBudgetInstruction::set_compute_unit_limit(
                self.total_budget.min(MAX_IX_BUDGET),
            ))
        } else {
            // No need for an extra compute budget instruction
            None
        }
    }

    pub async fn build_with_budget(
        self,
        extra_signers: &[&dyn Signer],
    ) -> Result<VersionedTransaction> {
        if self.instructions.is_empty() {
            return Err(errors::ErrorKind::NoInstructions);
        }

        let mut instructions = Vec::with_capacity(self.instructions.len() + 1);
        if let Some(ix_budget) = self.get_budget_ix() {
            instructions.push(ix_budget);
        }

        instructions.extend(self.instructions);

        self.link
            .create_tx_with_extra_lookup_tables(&instructions, extra_signers, &self.lookup_tables)
            .await
    }

    pub async fn build_with_budget_and_fee(
        self,
        extra_signers: &[&dyn Signer],
    ) -> Result<VersionedTransaction> {
        if self.instructions.is_empty() {
            return Err(errors::ErrorKind::NoInstructions);
        }

        let mut instructions = Vec::with_capacity(self.instructions.len() + 2);

        if let Some(ix_budget) = self.get_budget_ix() {
            instructions.push(ix_budget);
        }

        let fee = self.link.get_recommended_micro_lamport_fee();
        if fee > 0 {
            instructions.push(ComputeBudgetInstruction::set_compute_unit_price(fee));
        }

        instructions.extend(self.instructions);

        self.link
            .create_tx_with_extra_lookup_tables(&instructions, extra_signers, &self.lookup_tables)
            .await
    }

    /// Build the a versionned transaction with the instructions and lookup tables
    /// but ignore the provided budget. Instead simulate the transaction to get the
    /// required budget and set it in the transaction.
    pub async fn build_with_auto_budget_and_fee(
        self,
        extra_signers: &[&dyn Signer],
        cu_budget_extra_bps: u64,
    ) -> Result<VersionedTransaction> {
        if self.instructions.is_empty() {
            return Err(errors::ErrorKind::NoInstructions);
        }

        trace!("Simulating transaction to get compute unit budget");

        let simulation_cu = {
            let mut instructions = Vec::with_capacity(self.instructions.len() + 2);

            instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(
                self.total_budget,
            ));

            let fee = self.link.get_recommended_micro_lamport_fee();
            if fee > 0 {
                instructions.push(ComputeBudgetInstruction::set_compute_unit_price(fee));
            }

            instructions.extend(self.instructions.clone());

            let simulation_tx = self
                .link
                .create_tx_with_extra_lookup_tables(
                    &instructions,
                    extra_signers,
                    &self.lookup_tables,
                )
                .await?;
            let sim_res = self.link.client.simulate_transaction(&simulation_tx).await;
            match sim_res {
                Ok(sim_res) => {
                    if let Some(tx_err) = sim_res.err.as_ref() {
                        warn!(
                            "Transaction simulation failed with {:?}\n{:#?}",
                            tx_err,
                            sim_res.logs.unwrap_or_default()
                        );
                        return Err(tx_err.clone().into());
                    }
                    sim_res.units_consumed
                }
                Err(e) => {
                    warn!("Transaction simulation failed: {:?}", e);
                    return Err(e);
                }
            }
        };

        let cu_budget = if let Some(simulation_cu) = simulation_cu {
            trace!("Simulation consumed {} compute units", simulation_cu);
            (simulation_cu * (10_000 + cu_budget_extra_bps) / 10_000) as u32
        } else {
            // If simulation didn't provide a budget, use the default budget
            debug!(
                "Simulation returned without a consumed budget, using default budget: {}",
                self.total_budget
            );
            self.total_budget
        }
        .min(MAX_IX_BUDGET);

        let mut instructions = Vec::with_capacity(self.instructions.len() + 2);

        instructions.push(ComputeBudgetInstruction::set_compute_unit_limit(cu_budget));

        let fee = self.link.get_recommended_micro_lamport_fee();
        if fee > 0 {
            instructions.push(ComputeBudgetInstruction::set_compute_unit_price(fee));
        }

        instructions.extend(self.instructions);

        self.link
            .create_tx_with_extra_lookup_tables(&instructions, extra_signers, &self.lookup_tables)
            .await
    }

    /// Build a raw message from the known instructions
    ///
    /// The message is not signed, and the blockhash is not set allowing future signing by a multisig.
    /// Note: This is not compatible with versioned transactions yet and does not include lookup tables.
    pub fn build_raw_msg(&self) -> Vec<u8> {
        let payer_pubkey = self.link.payer_pubkey();

        let msg = Message::new(&self.instructions, Some(&payer_pubkey));
        msg.serialize()
    }

    /// Build a base64 encoded raw message from the known instructions.
    ///
    /// See `build_raw_msg` for more details.
    pub fn to_base64(&self) -> String {
        let raw_msg = self.build_raw_msg();
        BS64.encode(raw_msg)
    }

    /// Build a base58 encoded raw message from the known instructions.
    ///
    /// See `build_raw_msg` for more details.
    pub fn to_base58(&self) -> String {
        let raw_msg = self.build_raw_msg();
        bs58::encode(raw_msg).into_string()
    }
}
