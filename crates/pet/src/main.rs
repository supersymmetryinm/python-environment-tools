// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use jsonrpc::start_jsonrpc_server;
use pet::find_and_report_envs_stdio;

mod find;
mod jsonrpc;
mod locators;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Finds the environments and reports them to the standard output.
    Find {
        /// Directory for caching the environments.
        #[arg(short, long, value_name = "cache_dir")]
        cache_dir: Option<PathBuf>,
        /// Directory for caching the environments.
        #[arg(short, long)]
        list: Option<bool>,
    },
    /// Starts the JSON RPC Server.
    Server,
}

fn main() {
    let cli = Cli::parse();

    match cli.command.unwrap_or(Commands::Find { cache_dir: None, list: Some(true)}) {
        Commands::Find { list, cache_dir } => find_and_report_envs_stdio(list.unwrap_or(true), true, cache_dir),
        Commands::Server => start_jsonrpc_server(),
    }
}
