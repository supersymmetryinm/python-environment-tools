// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use find::find_and_report_envs;
use locators::create_locators;
use pet_cache::Cache;
use pet_conda::Conda;
use pet_core::{os_environment::EnvironmentApi, Configuration};
use pet_reporter::{self, cache::CacheReporter, stdio};
use std::{collections::BTreeMap, env, path::PathBuf, sync::Arc, time::SystemTime};

pub mod find;
pub mod locators;
pub mod resolve;

pub fn find_and_report_envs_stdio(
    print_list: bool,
    print_summary: bool,
    cache_dir: Option<PathBuf>,
) {
    stdio::initialize_logger(log::LevelFilter::Info);
    let now = SystemTime::now();

    let cache = cache_dir.map(|cache_dir| Arc::new(Cache::new(cache_dir)));
    let reporter = Arc::new(stdio::create_reporter(print_list));
    let cache_reporter = CacheReporter::new(reporter.clone(), cache.clone());
    let environment = EnvironmentApi::new();
    let conda_locator = Arc::new(Conda::from(&environment));

    let mut config = Configuration::default();
    if let Ok(cwd) = env::current_dir() {
        config.search_paths = Some(vec![cwd]);
    }
    let summary = find_and_report_envs(
        &cache_reporter,
        config,
        &create_locators(conda_locator.clone()),
        conda_locator,
        cache.clone(),
    );
    if print_summary {
        let summary = reporter.get_summary();
        if !summary.managers.is_empty() {
            println!("Managers:");
            println!("---------");
            for (k, v) in summary
                .managers
                .clone()
                .into_iter()
                .map(|(k, v)| (format!("{:?}", k), v))
                .collect::<BTreeMap<String, u16>>()
            {
                println!("{:<20} : {:?}", k, v);
            }
            println!()
        }
        if !summary.environments.is_empty() {
            println!("Environments:");
            println!("-------------");
            for (k, v) in summary
                .environments
                .clone()
                .into_iter()
                .map(|(k, v)| (format!("{:?}", k), v))
                .collect::<BTreeMap<String, u16>>()
            {
                println!("{:<20} : {:?}", k, v);
            }
            println!()
        }
    }

    if cache.is_some() {
        println!(
            "Refresh completed in {}ms ({}ms cache + {}ms search)",
            now.elapsed().unwrap().as_millis(),
            summary.validation_time.as_millis(),
            summary.search_time.as_millis(),
        )
    } else {
        println!(
            "Refresh completed in {}ms",
            now.elapsed().unwrap().as_millis()
        )
    }
}
