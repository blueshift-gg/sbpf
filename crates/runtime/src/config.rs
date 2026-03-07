use {solana_clock::Clock, solana_epoch_schedule::EpochSchedule, solana_rent::Rent};

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub compute_budget: u64,
    pub max_cpi_depth: usize,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            compute_budget: 200_000,
            max_cpi_depth: 4,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SysvarContext {
    pub clock: Clock,
    pub rent: Rent,
    pub epoch_schedule: EpochSchedule,
}

impl Default for SysvarContext {
    fn default() -> Self {
        Self {
            clock: Clock::default(),
            rent: Rent::default(),
            epoch_schedule: EpochSchedule::default(),
        }
    }
}

/// Reference: https://github.com/anza-xyz/agave/blob/master/program-runtime/src/execution_budget.rs
#[derive(Debug, Clone)]
pub struct ExecutionCost {
    pub syscall_base_cost: u64,
    pub log_64_units: u64,
    pub log_pubkey_units: u64,
    pub create_program_address_units: u64,
    pub invoke_units: u64,
    pub max_cpi_instruction_size: u64,
    pub cpi_bytes_per_unit: u64,
    pub max_instruction_stack_depth: usize,
    pub max_instruction_trace_length: usize,
    pub sha256_base_cost: u64,
    pub sha256_byte_cost: u64,
    pub sha256_max_slices: u64,
    pub sysvar_base_cost: u64,
    pub secp256k1_recover_cost: u64,
    pub curve25519_edwards_validate_point_cost: u64,
    pub curve25519_edwards_add_cost: u64,
    pub curve25519_edwards_subtract_cost: u64,
    pub curve25519_edwards_multiply_cost: u64,
    pub curve25519_edwards_msm_base_cost: u64,
    pub curve25519_edwards_msm_incremental_cost: u64,
    pub curve25519_ristretto_validate_point_cost: u64,
    pub curve25519_ristretto_add_cost: u64,
    pub curve25519_ristretto_subtract_cost: u64,
    pub curve25519_ristretto_multiply_cost: u64,
    pub curve25519_ristretto_msm_base_cost: u64,
    pub curve25519_ristretto_msm_incremental_cost: u64,
    pub heap_size: u64,
    pub heap_cost: u64,
    pub mem_op_base_cost: u64,
    pub alt_bn128_addition_cost: u64,
    pub alt_bn128_multiplication_cost: u64,
    pub alt_bn128_pairing_one_pair_cost_first: u64,
    pub alt_bn128_pairing_one_pair_cost_other: u64,
    pub alt_bn128_g1_compress: u64,
    pub alt_bn128_g1_decompress: u64,
    pub alt_bn128_g2_compress: u64,
    pub alt_bn128_g2_decompress: u64,
    pub big_modular_exponentiation_cost: u64,
    pub poseidon_cost_coefficient_a: u64,
    pub poseidon_cost_coefficient_c: u64,
    pub get_remaining_compute_units_cost: u64,
}

impl Default for ExecutionCost {
    fn default() -> Self {
        Self {
            syscall_base_cost: 100,
            log_64_units: 100,
            log_pubkey_units: 100,
            create_program_address_units: 1500,
            invoke_units: 1000,
            max_cpi_instruction_size: 46 * 1024,
            cpi_bytes_per_unit: 250,
            max_instruction_stack_depth: 5,
            max_instruction_trace_length: 64,
            sha256_base_cost: 85,
            sha256_byte_cost: 1,
            sha256_max_slices: 20_000,
            sysvar_base_cost: 100,
            secp256k1_recover_cost: 25_000,
            curve25519_edwards_validate_point_cost: 159,
            curve25519_edwards_add_cost: 473,
            curve25519_edwards_subtract_cost: 475,
            curve25519_edwards_multiply_cost: 2_177,
            curve25519_edwards_msm_base_cost: 2_273,
            curve25519_edwards_msm_incremental_cost: 758,
            curve25519_ristretto_validate_point_cost: 169,
            curve25519_ristretto_add_cost: 521,
            curve25519_ristretto_subtract_cost: 519,
            curve25519_ristretto_multiply_cost: 2_208,
            curve25519_ristretto_msm_base_cost: 2_303,
            curve25519_ristretto_msm_incremental_cost: 788,
            heap_size: 32 * 1024,
            heap_cost: 8,
            mem_op_base_cost: 10,
            alt_bn128_addition_cost: 334,
            alt_bn128_multiplication_cost: 3_840,
            alt_bn128_pairing_one_pair_cost_first: 36_364,
            alt_bn128_pairing_one_pair_cost_other: 12_121,
            alt_bn128_g1_compress: 30,
            alt_bn128_g1_decompress: 398,
            alt_bn128_g2_compress: 86,
            alt_bn128_g2_decompress: 13_610,
            big_modular_exponentiation_cost: 33,
            poseidon_cost_coefficient_a: 61,
            poseidon_cost_coefficient_c: 542,
            get_remaining_compute_units_cost: 100,
        }
    }
}
