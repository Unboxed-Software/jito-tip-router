#[cfg(test)]
mod tests {
    use jito_restaking_core::{config::Config, ncn_vault_ticket::NcnVaultTicket};
    use jito_tip_router_core::constants::{MAX_OPERATORS, WEIGHT, WEIGHT_PRECISION};
    use solana_sdk::{
        native_token::sol_to_lamports, pubkey::Pubkey, signature::Keypair, signer::Signer,
    };

    use crate::fixtures::{test_builder::TestBuilder, TestResult};

    #[ignore = "20-30 minute test"]
    #[tokio::test]
    async fn simulation_test() -> TestResult<()> {
        let mut fixture = TestBuilder::new().await;
        let mut stake_pool_client = fixture.stake_pool_client();
        let mut tip_router_client = fixture.tip_router_client();
        let mut vault_program_client = fixture.vault_client();
        let mut restaking_client = fixture.restaking_program_client();

        const OPERATOR_COUNT: usize = 13;

        let mints = vec![
            (Keypair::new(), WEIGHT),           // JitoSOL
            (Keypair::new(), WEIGHT),           // JTO
            (Keypair::new(), WEIGHT),           // BnSOL
            (Keypair::new(), WEIGHT_PRECISION), // nSol
        ];

        let delegations = [
            1,
            sol_to_lamports(1000.0),
            sol_to_lamports(10000.0),
            sol_to_lamports(100000.0),
            sol_to_lamports(1000000.0),
            sol_to_lamports(10000000.0),
        ];

        // Setup NCN
        let mut test_ncn = fixture.create_test_ncn().await?;
        let ncn = test_ncn.ncn_root.ncn_pubkey;
        let pool_root = stake_pool_client.do_initialize_stake_pool().await?;

        // Add operators and vaults
        {
            fixture
                .add_operators_to_test_ncn(&mut test_ncn, OPERATOR_COUNT, Some(100))
                .await?;
            // JitoSOL
            fixture
                .add_vaults_to_test_ncn(&mut test_ncn, 3, Some(mints[0].0.insecure_clone()))
                .await?;
            // JTO
            fixture
                .add_vaults_to_test_ncn(&mut test_ncn, 2, Some(mints[1].0.insecure_clone()))
                .await?;
            // BnSOL
            fixture
                .add_vaults_to_test_ncn(&mut test_ncn, 1, Some(mints[2].0.insecure_clone()))
                .await?;
            // nSol
            fixture
                .add_vaults_to_test_ncn(&mut test_ncn, 1, Some(mints[3].0.insecure_clone()))
                .await?;
        }

        // Add delegation
        {
            for (index, operator_root) in test_ncn
                .operators
                .iter()
                .take(OPERATOR_COUNT - 1)
                .enumerate()
            {
                for vault_root in test_ncn.vaults.iter() {
                    let delegation_amount = delegations[index % delegations.len()];

                    if delegation_amount > 0 {
                        vault_program_client
                            .do_add_delegation(
                                vault_root,
                                &operator_root.operator_pubkey,
                                delegation_amount,
                            )
                            .await
                            .unwrap();
                    }
                }
            }
        }

        // Register ST Mint
        {
            let restaking_config_address =
                Config::find_program_address(&jito_restaking_program::id()).0;
            let restaking_config = restaking_client
                .get_config(&restaking_config_address)
                .await?;

            let epoch_length = restaking_config.epoch_length();

            fixture
                .warp_slot_incremental(epoch_length * 2)
                .await
                .unwrap();

            for (mint, weight) in mints.iter() {
                tip_router_client
                    .do_admin_register_st_mint(ncn, mint.pubkey(), *weight)
                    .await?;
            }

            for vault in test_ncn.vaults.iter() {
                let vault = vault.vault_pubkey;
                let (ncn_vault_ticket, _, _) = NcnVaultTicket::find_program_address(
                    &jito_restaking_program::id(),
                    &ncn,
                    &vault,
                );

                tip_router_client
                    .do_register_vault(ncn, vault, ncn_vault_ticket)
                    .await?;
            }
        }

        fixture.add_epoch_state_for_test_ncn(&test_ncn).await?;
        fixture.add_admin_weights_for_test_ncn(&test_ncn).await?;

        fixture.add_epoch_snapshot_to_test_ncn(&test_ncn).await?;
        fixture
            .add_operator_snapshots_to_test_ncn(&test_ncn)
            .await?;
        fixture
            .add_vault_operator_delegation_snapshots_to_test_ncn(&test_ncn)
            .await?;
        fixture.add_ballot_box_to_test_ncn(&test_ncn).await?;

        // Cast votes
        {
            let epoch = fixture.clock().await.epoch;

            let zero_delegation_operator = test_ncn.operators.last().unwrap();
            let first_operator = &test_ncn.operators[0];
            let second_operator = &test_ncn.operators[1];
            let third_operator = &test_ncn.operators[2];

            for _ in 0..MAX_OPERATORS + 5 {
                let meta_merkle_root = Pubkey::new_unique().to_bytes();

                tip_router_client
                    .do_cast_vote(
                        ncn,
                        zero_delegation_operator.operator_pubkey,
                        &zero_delegation_operator.operator_admin,
                        meta_merkle_root,
                        epoch,
                    )
                    .await?;
            }

            let meta_merkle_root = Pubkey::new_unique().to_bytes();
            tip_router_client
                .do_cast_vote(
                    ncn,
                    zero_delegation_operator.operator_pubkey,
                    &zero_delegation_operator.operator_admin,
                    meta_merkle_root,
                    epoch,
                )
                .await?;
            tip_router_client
                .do_cast_vote(
                    ncn,
                    first_operator.operator_pubkey,
                    &first_operator.operator_admin,
                    meta_merkle_root,
                    epoch,
                )
                .await?;
            let meta_merkle_root = Pubkey::new_unique().to_bytes();
            tip_router_client
                .do_cast_vote(
                    ncn,
                    zero_delegation_operator.operator_pubkey,
                    &zero_delegation_operator.operator_admin,
                    meta_merkle_root,
                    epoch,
                )
                .await?;
            tip_router_client
                .do_cast_vote(
                    ncn,
                    second_operator.operator_pubkey,
                    &second_operator.operator_admin,
                    meta_merkle_root,
                    epoch,
                )
                .await?;
            tip_router_client
                .do_cast_vote(
                    ncn,
                    third_operator.operator_pubkey,
                    &third_operator.operator_admin,
                    meta_merkle_root,
                    epoch,
                )
                .await?;
            let meta_merkle_root = Pubkey::new_unique().to_bytes();
            tip_router_client
                .do_cast_vote(
                    ncn,
                    zero_delegation_operator.operator_pubkey,
                    &zero_delegation_operator.operator_admin,
                    meta_merkle_root,
                    epoch,
                )
                .await?;
            tip_router_client
                .do_cast_vote(
                    ncn,
                    first_operator.operator_pubkey,
                    &first_operator.operator_admin,
                    meta_merkle_root,
                    epoch,
                )
                .await?;
            tip_router_client
                .do_cast_vote(
                    ncn,
                    second_operator.operator_pubkey,
                    &second_operator.operator_admin,
                    meta_merkle_root,
                    epoch,
                )
                .await?;
            tip_router_client
                .do_cast_vote(
                    ncn,
                    third_operator.operator_pubkey,
                    &third_operator.operator_admin,
                    meta_merkle_root,
                    epoch,
                )
                .await?;
            let meta_merkle_root = Pubkey::new_unique().to_bytes();
            for operator_root in test_ncn.operators.iter().take(OPERATOR_COUNT - 1) {
                let operator = operator_root.operator_pubkey;

                tip_router_client
                    .do_cast_vote(
                        ncn,
                        operator,
                        &operator_root.operator_admin,
                        meta_merkle_root,
                        epoch,
                    )
                    .await?;
            }

            let ballot_box = tip_router_client.get_ballot_box(ncn, epoch).await?;
            assert!(ballot_box.has_winning_ballot());
            assert!(ballot_box.is_consensus_reached());
            assert_eq!(
                ballot_box.get_winning_ballot().unwrap().root(),
                meta_merkle_root
            );
        }

        stake_pool_client
            .update_stake_pool_balance(&pool_root)
            .await?;
        fixture.close_epoch_accounts_for_test_ncn(&test_ncn).await?;

        Ok(())
    }
}

#[cfg(test)]
mod fuzz_tests {
    use crate::fixtures::{test_builder::TestBuilder, TestResult};
    use jito_restaking_core::{config::Config, ncn_vault_ticket::NcnVaultTicket};
    use jito_tip_router_core::constants::{MAX_OPERATORS, WEIGHT, WEIGHT_PRECISION};
    use solana_sdk::{
        native_token::sol_to_lamports, pubkey::Pubkey, signature::Keypair, signer::Signer,
    };

    struct MintConfig {
        keypair: Keypair,
        weight: u128,
        vault_count: usize,
    }

    struct SimConfig {
        operator_count: usize,
        mints: Vec<MintConfig>,
        delegations: Vec<u64>,
        operator_fee_bps: u16,
    }

    async fn run_simulation(config: SimConfig) -> TestResult<()> {
        let mut fixture = TestBuilder::new().await;
        let mut stake_pool_client = fixture.stake_pool_client();
        let mut tip_router_client = fixture.tip_router_client();
        let mut vault_program_client = fixture.vault_client();
        let mut restaking_client = fixture.restaking_program_client();

        let total_vaults = config.mints.iter().map(|m| m.vault_count).sum::<usize>();
        assert_eq!(config.delegations.len(), total_vaults);

        // Setup NCN
        let mut test_ncn = fixture.create_test_ncn().await?;
        let ncn = test_ncn.ncn_root.ncn_pubkey;
        let pool_root = stake_pool_client.do_initialize_stake_pool().await?;

        // Add operators and vaults
        {
            fixture
                .add_operators_to_test_ncn(
                    &mut test_ncn,
                    config.operator_count,
                    Some(config.operator_fee_bps),
                )
                .await?;

            for mint_config in config.mints.iter() {
                fixture
                    .add_vaults_to_test_ncn(
                        &mut test_ncn,
                        mint_config.vault_count,
                        Some(mint_config.keypair.insecure_clone()),
                    )
                    .await?;
            }
        }

        // Add delegation
        {
            let seed = Pubkey::new_unique()
                .to_bytes()
                .iter()
                .enumerate()
                .fold(0u64, |acc, (i, &byte)| {
                    acc.wrapping_add((byte as u64) << (i % 8 * 8))
                });

            for (vault_index, vault_root) in test_ncn.vaults.iter().enumerate() {
                let total_vault_delegation = config.delegations[vault_index];

                // Create a shuffled list of operators
                let mut operators: Vec<_> = test_ncn.operators.iter().collect();
                let shuffle_index = seed.wrapping_add(vault_index as u64);

                // Fisher-Yates shuffle
                for i in (1..operators.len()).rev() {
                    let j = (shuffle_index.wrapping_mul(i as u64) % (i as u64 + 1)) as usize;
                    operators.swap(i, j);
                }

                // Skip the first operator (effectively excluding them from delegation)
                let selected_operators = operators.iter().skip(1).take(config.operator_count - 2);
                let operator_count = config.operator_count - 2; // Reduced by one more to account for exclusion

                // Calculate per-operator delegation amount
                let delegation_per_operator = total_vault_delegation / operator_count as u64;

                if delegation_per_operator > 0 {
                    for operator_root in selected_operators {
                        vault_program_client
                            .do_add_delegation(
                                vault_root,
                                &operator_root.operator_pubkey,
                                delegation_per_operator,
                            )
                            .await
                            .unwrap();
                    }
                }
            }
        }

        // Register ST Mint
        {
            let restaking_config_address =
                Config::find_program_address(&jito_restaking_program::id()).0;
            let restaking_config = restaking_client
                .get_config(&restaking_config_address)
                .await?;
            let epoch_length = restaking_config.epoch_length();

            fixture.warp_slot_incremental(epoch_length * 2).await?;

            for mint_config in config.mints.iter() {
                tip_router_client
                    .do_admin_register_st_mint(
                        ncn,
                        mint_config.keypair.pubkey(),
                        mint_config.weight,
                    )
                    .await?;
            }

            for vault in test_ncn.vaults.iter() {
                let vault = vault.vault_pubkey;
                let (ncn_vault_ticket, _, _) = NcnVaultTicket::find_program_address(
                    &jito_restaking_program::id(),
                    &ncn,
                    &vault,
                );

                tip_router_client
                    .do_register_vault(ncn, vault, ncn_vault_ticket)
                    .await?;
            }
        }

        fixture.add_epoch_state_for_test_ncn(&test_ncn).await?;
        fixture.add_admin_weights_for_test_ncn(&test_ncn).await?;

        {
            let epoch = fixture.clock().await.epoch;
            let epoch_state = tip_router_client.get_epoch_state(ncn, epoch).await?;
            assert!(epoch_state.set_weight_progress().is_complete())
        }

        fixture.add_epoch_snapshot_to_test_ncn(&test_ncn).await?;
        fixture
            .add_operator_snapshots_to_test_ncn(&test_ncn)
            .await?;
        fixture
            .add_vault_operator_delegation_snapshots_to_test_ncn(&test_ncn)
            .await?;
        fixture.add_ballot_box_to_test_ncn(&test_ncn).await?;

        // Cast votes
        {
            let epoch = fixture.clock().await.epoch;

            // Do some random voting first
            let zero_delegation_operator = test_ncn.operators.last().unwrap();
            let vote_operators = &test_ncn.operators.clone(); // Take first few operators for random voting

            for _ in 0..MAX_OPERATORS + 55 {
                // Generate random merkle root
                let random_merkle_root = Pubkey::new_unique().to_bytes();
                let offset = random_merkle_root
                    .iter()
                    .map(|&x| x as usize)
                    .sum::<usize>();

                // Random operator votes for it
                let random_operator = &vote_operators[offset % vote_operators.len()];
                tip_router_client
                    .do_cast_vote(
                        ncn,
                        random_operator.operator_pubkey,
                        &random_operator.operator_admin,
                        random_merkle_root,
                        epoch,
                    )
                    .await?;

                // Zero delegation operator also votes
                tip_router_client
                    .do_cast_vote(
                        ncn,
                        zero_delegation_operator.operator_pubkey,
                        &zero_delegation_operator.operator_admin,
                        random_merkle_root,
                        epoch,
                    )
                    .await?;
            }

            // Then do the consensus vote
            let meta_merkle_root = Pubkey::new_unique().to_bytes();
            // First create a mutable copy of the operators that we can shuffle
            let mut operators_to_shuffle = test_ncn.operators.clone();

            // Use the merkle root bytes to create a deterministic shuffle
            let shuffle_seed: u64 = meta_merkle_root
                .iter()
                .enumerate()
                .fold(0u64, |acc, (i, &byte)| {
                    acc.wrapping_add((byte as u64) << (i % 8 * 8))
                });

            // Fisher-Yates shuffle using the seed
            for i in (1..operators_to_shuffle.len()).rev() {
                // Use the seed to generate a deterministic index
                let j = (shuffle_seed.wrapping_mul(i as u64) % (i as u64 + 1)) as usize;
                operators_to_shuffle.swap(i, j);
            }

            // Now use the shuffled operators
            for operator_root in operators_to_shuffle.iter() {
                let operator = operator_root.operator_pubkey;
                let _ = tip_router_client
                    .do_cast_vote(
                        ncn,
                        operator,
                        &operator_root.operator_admin,
                        meta_merkle_root,
                        epoch,
                    )
                    .await;
            }

            let ballot_box = tip_router_client.get_ballot_box(ncn, epoch).await?;
            assert!(ballot_box.has_winning_ballot());
            assert!(ballot_box.is_consensus_reached());
            assert_eq!(
                ballot_box.get_winning_ballot().unwrap().root(),
                meta_merkle_root
            );
        }

        stake_pool_client
            .update_stake_pool_balance(&pool_root)
            .await?;
        fixture.close_epoch_accounts_for_test_ncn(&test_ncn).await?;

        Ok(())
    }

    #[ignore = "20-30 minute test"]
    #[tokio::test]
    async fn test_basic_simulation() -> TestResult<()> {
        let config = SimConfig {
            operator_count: 13,
            mints: vec![
                MintConfig {
                    keypair: Keypair::new(),
                    weight: WEIGHT,
                    vault_count: 3,
                },
                MintConfig {
                    keypair: Keypair::new(),
                    weight: WEIGHT,
                    vault_count: 2,
                },
                MintConfig {
                    keypair: Keypair::new(),
                    weight: WEIGHT,
                    vault_count: 1,
                },
                MintConfig {
                    keypair: Keypair::new(),
                    weight: WEIGHT_PRECISION,
                    vault_count: 1,
                },
            ],
            delegations: vec![
                // Need 7
                1,
                sol_to_lamports(1000.0),
                sol_to_lamports(10000.0),
                sol_to_lamports(100000.0),
                sol_to_lamports(1000000.0),
                sol_to_lamports(10000000.0),
                255,
            ],
            operator_fee_bps: 100,
        };

        run_simulation(config).await
    }

    // #[ignore = "20-30 minute test"]
    #[tokio::test]
    async fn test_high_operator_count_simulation() -> TestResult<()> {
        let config = SimConfig {
            operator_count: 50,
            mints: vec![MintConfig {
                keypair: Keypair::new(),
                weight: WEIGHT,
                vault_count: 2,
            }],
            delegations: vec![sol_to_lamports(1000.0), sol_to_lamports(1000.0)],
            operator_fee_bps: 100,
        };

        run_simulation(config).await
    }

    // #[ignore = "20-30 minute test"]
    #[tokio::test]
    async fn test_fuzz_simulation() -> TestResult<()> {
        // Create multiple test configurations with different parameters
        let test_configs = vec![
            // Test varying operator counts
            SimConfig {
                operator_count: 15, // Mid-size operator set
                mints: vec![
                    MintConfig {
                        keypair: Keypair::new(),
                        weight: WEIGHT,
                        vault_count: 2,
                    },
                    MintConfig {
                        keypair: Keypair::new(),
                        weight: WEIGHT,
                        vault_count: 1,
                    },
                ],
                delegations: vec![
                    sol_to_lamports(500.0),
                    sol_to_lamports(5000.0),
                    sol_to_lamports(50000.0),
                ],
                operator_fee_bps: 90,
            },
            // Test extreme delegation amounts
            SimConfig {
                operator_count: 20,
                mints: vec![MintConfig {
                    keypair: Keypair::new(),
                    weight: 2 * WEIGHT_PRECISION,
                    vault_count: 3,
                }],
                delegations: vec![
                    1, // Minimum delegation
                    sol_to_lamports(1.0),
                    sol_to_lamports(1_000_000.0), // Very large delegation
                ],
                operator_fee_bps: 150,
            },
            // Test mixed fee groups and feeds
            SimConfig {
                operator_count: 30,
                mints: vec![
                    MintConfig {
                        keypair: Keypair::new(),
                        weight: WEIGHT,
                        vault_count: 1,
                    },
                    MintConfig {
                        keypair: Keypair::new(),
                        weight: WEIGHT * 2,
                        vault_count: 1,
                    },
                    MintConfig {
                        keypair: Keypair::new(),
                        weight: WEIGHT_PRECISION / 2,
                        vault_count: 1,
                    },
                ],
                delegations: vec![
                    sol_to_lamports(100.0),
                    sol_to_lamports(1000.0),
                    sol_to_lamports(10000.0),
                ],
                operator_fee_bps: 80,
            },
        ];

        // Run all configurations
        for (i, config) in test_configs.into_iter().enumerate() {
            println!("Running fuzz test configuration {}", i + 1);
            run_simulation(config).await?;
        }

        Ok(())
    }
}
