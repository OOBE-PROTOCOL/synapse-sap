//! Stampa INIT_SPACE reale di ogni account per l'analisi rent.
//! Esegui: cargo test -p synapse-agent-sap --test space_report -- --nocapture

use anchor_lang::Space;
use synapse_agent_sap::state::*;

#[test]
fn space_report() {
    fn show<T: Space>(name: &str) {
        let total = 8 + T::INIT_SPACE;
        println!(
            "{name:24} INIT_SPACE={:5}  +disc=8 => {}",
            T::INIT_SPACE,
            total
        );
        // Rent-exempt standard: (128 + total) * 6.96 lamports/byte ≈ lamports a 2 anni.
        let lamports = (128 + total) * 6960 / 1000;
        println!("{name:24} rent-exempt ≈ {lamports} lamports");
    }
    show::<AgentAccount>("AgentAccount");
    show::<AgentStats>("AgentStats");
    show::<AgentPricingMenu>("AgentPricingMenu");
    show::<GlobalRegistry>("GlobalRegistry");
    show::<FeedbackAccount>("FeedbackAccount");
    show::<CapabilityIndex>("CapabilityIndex");
    show::<ProtocolIndex>("ProtocolIndex");
    show::<EscrowAccountV2>("EscrowAccountV2");
    show::<PendingSettlement>("PendingSettlement");
    show::<DisputeRecord>("DisputeRecord");
    show::<ReceiptBatch>("ReceiptBatch");
    show::<AgentStake>("AgentStake");
    show::<Subscription>("Subscription");
    show::<CounterShard>("CounterShard");
    show::<IndexPage>("IndexPage");
    show::<SessionLedger>("SessionLedger");
    show::<EpochPage>("EpochPage");
    show::<SettlementReceipt>("SettlementReceipt");
    show::<ToolDescriptor>("ToolDescriptor");
    show::<AgentAttestation>("AgentAttestation");
}