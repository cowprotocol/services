use std::{collections::{BTreeSet, HashMap, HashSet}, default, fs::{self, File}, io::BufReader};

use anyhow::{Context, Ok};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use sqlx::{prelude::FromRow, PgConnection};
use toml::Value as TomlValue;

use crate::domain::eth::{Address, TokenAddress};

use super::{database::{bad_tokens::{cleanup, insert, load_deny_list_for_solver, load_token_list_for_solver}, Postgres}, Solver};

#[derive(Debug, Deserialize)]
pub struct Config {
    solver: Address,
    supported_tokens: Vec<TokenAddress>,
    unsupported_tokens: Vec<TokenAddress>,
    timespan: u32, // needs to be discussed
    heuristic: Heuristic,
    mode: Mode,
}

impl Config {
    pub fn supported_tokens(&self) -> Vec<TokenAddress> {
        self.supported_tokens.clone()
    }

    pub fn unsupported_tokens(&self) -> Vec<TokenAddress> {
        self.unsupported_tokens.clone()
    }

    pub fn mode(&self) -> Mode {
        self.mode.to_owned()
    }

    pub fn timespan(&self) -> u32 {
        self.timespan
    }
}

pub struct ConfigFile {
    solver: Option<String>,
    tokens: Option<TomlValue>,
    timespan: Option<u32>,
    heuristic: Option<TomlValue>,
    mode: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            supported_tokens: Vec::new(),
            unsupported_tokens: Vec::new(),
            timespan: 1,
            ..Default::default()
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum Heuristic {
    ThresholdBased(Threshold),
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum HeuristicState {
    ThresholdBased(ThresholdState)
}

impl HeuristicState {
    pub fn default(heuristic: Heuristic) -> Self {
        match heuristic {
            Heuristic::ThresholdBased(threshold) => {
                Self::ThresholdBased(ThresholdState::default())
            },
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct ThresholdState {
    count: u32,
}

impl Default for Heuristic {
    fn default() -> Self {
        Self::ThresholdBased(Threshold::default())
    }
}

#[derive(Debug, Deserialize)]
pub struct Threshold {
    threshold: u32,
}

impl From<u32> for Threshold {
    fn from(value: u32) -> Self {
        Self {
            threshold: value,
        }
    }
}

impl Default for Threshold {
    fn default() -> Self {
        Self { threshold: 10u32}
    }
}

#[derive(Debug, Deserialize, Clone)]
pub enum Mode {
    LogOnly,
    Enabled,    
}

impl Default for Mode {
    fn default() -> Self {
        Self::LogOnly
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct BadTokenMonitor {
    config: Config,
    deny_list: HashSet<TokenAddress>,
    explicit_deny_list: BTreeSet<TokenAddress>,
    explicit_allow_list: BTreeSet<TokenAddress>,
    heuristic_state: HashMap<TokenAddress, HeuristicState>
}

impl BadTokenMonitor {
    pub fn solver(&self) -> Address {
        self.config.solver.clone()
    }

    pub fn heuristic(&self) -> Heuristic {
        self.config.heuristic.clone()
    }

    pub fn timespan(&self) -> u32 {
        self.config.timespan()
    }

    pub fn deny_list(&self) -> Vec<TokenAddress> {
        self.deny_list.clone().into_iter().collect_vec()
    }
}

impl BadTokenMonitor {
    pub fn from_config(config: Config) -> Self {
        Self {
            config,
            deny_list: HashSet::new(),
            explicit_deny_list: BTreeSet::from_iter(config.unsupported_tokens().into_iter()),
            explicit_allow_list: BTreeSet::from_iter(config.supported_tokens().into_iter()),
            heuristic_state: HashMap::new(),
        }
    }

    pub fn new(solver: Address) -> Self {
        Self::default()
    }

    pub fn from_file_path(file_path: &str) -> Result<Self, String> {
        let file = fs::read_to_string(file_path)
            .map_err(|err| format!("Unable to read file {}\n{:?}", file_path, e))?;

        let config_file: ConfigFile  = match toml::from_str(&file) {
            Ok(s) => s,
            Err(e) => {
                return Err(format!("Config file malformatted {}", e));
            }
        };

        Self::from_config_file(config_file)
    }

    pub fn from_config_file(config_file: ConfigFile) -> Result<Self, String> {
        let solver = config_file.solver.map(|address| address.as_bytes().clone().into())
            .ok_or(format!("no solver associated with this config"))?;
        let supported_tokens = Vec::new();
        let unsupported_tokens = Vec::new();
        let timespan = config_file.timespan.unwrap_or(1u32);

        if let Some(table) = config_file.tokens {
            if let Some(TomlValue::Array(tokens)) = table.unsupported {
                for token in tokens {
                    address = token.address.map(|address| address.as_bytes().clone().into())
                        .ok_or(format!("address is not correct \n{}", err))?;
                    unsupported_tokens.push(address);
                }
            }

            if let Some(TomlValue::Array(tokens)) = table.supported {
                for token in tokens {
                    address = token.address.map(|address| address.as_bytes().clone().into())
                        .ok_or(format!("address {} is not correct \n{}", address, err))?;
                    supported_tokens.push(address);
                }
            }
        }

        let heuristic = match &config_file.heuristic {
            Some(TomlValue::Table(table)) => {
                if let Some(params) = table.get("ThresholdBased") {
                    Heuristic::ThresholdBased(params.threshold.into())
                } else {
                    Heuristic::default()
                }
            },
            _ => Heuristic::default(),
        };

        let mode = match &config_file.mode {
            Some(mode) if "LogOnly" == mode.as_str() => Mode::LogOnly,
            Some(mode) if "Enabled" == mode.as_str() => Mode::Enabled,
            _ => return Err(format!("unsupported mode")),
        };

        let config = Config {
            solver,
            supported_tokens,
            unsupported_tokens,
            timespan,
            heuristic,
            mode,
        };

        Ok(Self::from_config(config))
    }

    pub fn collect_from_path(file_path: &str, solvers: &Vec<Solver>) -> HashMap<Address, BadTokenMonitor> {
        let paths = fs::read_dir(file_path).unwrap();

        let mut bad_token_monitors = HashMap::new();
        for path in paths {
            if let Ok(file) = path {
                let monitor = match Self::from_file_path(file.path()) {
                    Ok(monitor) => monitor,
                    Err(_) => continue,
                };
                
                bad_token_monitors.insert(monitor.solver(), monitor);
            }
        }

        let result = HashMap::new();

        for solver in solvers {
            match bad_token_monitors.remove(&solver.address()) {
                Some(monitor) => result.insert(solver.address(), monitor),
                None => result.insert(solver.address(), BadTokenMonitor::new(solver.address())),
            }
        }

        result
    }

    pub async fn consolidate(&mut self, db: &Postgres, tokens: impl Iterator<Item = &TokenAddress>) -> Result<(), sqlx::Error> {
        if Mode::LogOnly = self.config.mode() {
            format!("implement logging");
            return;
        }

        let mut tokens_to_insert = Vec::new();
        let default_heuristic_state = HeuristicState::default(self.heuristic());
        for token in tokens {
            if self.explicit_allow_list.contains(token) {
                continue;
            }

            if self.explicit_deny_list.contains(token) {
                continue;
            }

            let mut heuristic_state = self
                .heuristic_state
                .get(token)
                .map(|state| state.to_owned())
                .unwrap_or(default_heuristic_state);

            match (self.heuristic(), &mut heuristic_state) {
                (
                    Heuristic::ThresholdBased(threshold), 
                    HeuristicState::ThresholdBased(threshold_state)
                ) => {
                    if threshold.threshold <= threshold_state.count + 1 {
                        self.deny_list.insert(token.to_owned())
                    }

                    threshold_state.count += 1;
                },

                (_, _) => format!("unimplimented"),
            }

            self.heuristic_state.insert(token.to_owned(), heuristic_state.to_owned());
            tokens_to_insert.push((token.to_owned(), heuristic_state));
        }

        let mut ex = db.pool.begin().await.context("begin")?;
        insert(&mut ex, self.solver(), tokens_to_insert).await;
        ex.commit().await.context("commit");
        
        Ok(())
    }

    pub async fn initialize(&mut self, db: &Postgres) -> Result<(), sqlx::Error> {
        if Mode::LogOnly = self.config.mode() {
            format!("implement logging");
            return;
        }

        let mut ex = db.pool.begin().await.context("begin")?;
        let heuristic_states = load_token_list_for_solver(&mut ex, &self.solver()).await;
        ex.commit().await.context("commit");

        let deny_list = HashSet::new();
        for (token, state) in heuristic_states {
            match (self.heuristic(), state) {
                (
                    Heuristic::ThresholdBased(threshold), 
                    HeuristicState::ThresholdBased(threshold_state)
                ) => {
                    if threshold.threshold <= threshold_state.count {
                        deny_list.insert(token)
                    }
                },

                (_, _) => format!("unimplimented"),
            }
        }

        self.deny_list = deny_list;
        self.heuristic_state = heuristic_states;
        
        Ok(())
    }

    async fn periodic_cleanup(&mut self, db: &Postgres) -> Result<(), sqlx::Error> {
        let mut ex = db.pool.begin().await.context("begin")?;
        let response = cleanup(&mut ex, &self.solver(), self.timespan(), self.deny_list()).await;
        ex.commit().await.context("commit");

        if let Ok(tokens) = response {
            for token in tokens {
                self.deny_list.remove(&token)
            }
        }

        // should the heuristic state be reseted??
        Ok(())
    }
}
