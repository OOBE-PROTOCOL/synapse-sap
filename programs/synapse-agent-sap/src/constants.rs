use anchor_lang::prelude::Pubkey;

pub const PROTOCOL_TREASURY: Pubkey = Pubkey::new_from_array([
    254, 58, 35, 254, 177, 114, 214, 200, 0, 39, 3, 234, 1, 113, 168, 249, 59, 219, 108, 203, 245,
    191, 214, 229, 139, 126, 120, 29, 50, 104, 73, 50,
]);

pub const PROTOCOL_FEE_BPS: u64 = 50;
pub const BPS_DENOMINATOR: u64 = 10_000;
