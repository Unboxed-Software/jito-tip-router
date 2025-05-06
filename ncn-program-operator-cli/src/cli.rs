use std::path::PathBuf;

use clap::Parser;
use solana_sdk::pubkey::Pubkey;

use crate::OperatorState;

#[derive(Clone, Parser)]
#[command(author, version, about)]
pub struct Cli {
    #[arg(short, long, env)]
    pub keypair_path: String,

    #[arg(short, long, env)]
    pub operator_address: String,

    #[arg(short, long, env, default_value = "http://localhost:8899")]
    pub rpc_url: String,

    #[arg(short, long, env)]
    pub ledger_path: PathBuf,

    #[arg(short, long, env)]
    pub full_snapshots_path: Option<PathBuf>,

    #[arg(short, long, env)]
    pub backup_snapshots_dir: PathBuf,

    #[arg(short, long, env)]
    pub snapshot_output_dir: PathBuf,

    #[arg(long, env, default_value = "false")]
    pub submit_as_memo: bool,

    /// The price to pay for priority fee
    #[arg(long, env, default_value_t = 1)]
    pub micro_lamports: u64,

    #[command(subcommand)]
    pub command: Commands,
}

impl Cli {
    pub fn get_snapshot_paths(&self) -> SnapshotPaths {
        let ledger_path = self.ledger_path.clone();
        let account_paths = None;
        let account_paths = account_paths.map_or_else(|| vec![ledger_path.clone()], |paths| paths);
        let full_snapshots_path = self.full_snapshots_path.clone();
        let full_snapshots_path = full_snapshots_path.map_or(ledger_path.clone(), |path| path);
        let incremental_snapshots_path = self.backup_snapshots_dir.clone();
        SnapshotPaths {
            ledger_path,
            account_paths,
            full_snapshots_path,
            incremental_snapshots_path,
            backup_snapshots_dir: self.backup_snapshots_dir.clone(),
        }
    }

    pub fn force_different_backup_snapshot_dir(&self) {
        let snapshot_paths = self.get_snapshot_paths();
        assert_ne!(
            snapshot_paths.full_snapshots_path,
            snapshot_paths.backup_snapshots_dir
        );
    }
}

pub struct SnapshotPaths {
    pub ledger_path: PathBuf,
    pub account_paths: Vec<PathBuf>,
    pub full_snapshots_path: PathBuf,
    pub incremental_snapshots_path: PathBuf,
    /// Used when storing or loading snapshots that the operator CLI is workign with
    pub backup_snapshots_dir: PathBuf,
}

#[derive(clap::Subcommand, Clone)]
pub enum Commands {
    Run {
        #[arg(short, long, env)]
        ncn_address: Pubkey,

        #[arg(long, env)]
        ncn_program_id: Pubkey,

        #[arg(long, env, default_value = "3")]
        num_monitored_epochs: u64,

        #[arg(long, env)]
        override_target_slot: Option<u64>,

        #[arg(long, env, default_value = "wait-for-next-epoch")]
        starting_stage: OperatorState,

        #[arg(
            long,
            env,
            alias = "enable-snapshots",
            help = "Flag to enable storing created snapshots (formerly enable-snapshots)",
            default_value = "false"
        )]
        save_snapshot: bool,
    },
    SnapshotSlot {
        #[arg(long, env)]
        slot: u64,
    },
    SubmitEpoch {
        #[arg(short, long, env)]
        ncn_address: Pubkey,

        #[arg(long, env)]
        ncn_program_id: Pubkey,

        #[arg(long, env)]
        epoch: u64,
    },
}
