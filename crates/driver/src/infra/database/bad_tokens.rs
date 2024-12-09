use std::collections::{BTreeMap, HashMap};

use database::bad_tokens::{BadTokenIn, BadTokenOut};
use ethcontract::H160;
use itertools::Itertools;
use num::range;
use serde::{Deserialize, Serialize};
use sqlx::{types::Json, PgConnection, QueryBuilder};

use crate::{domain::eth::{Address, ContractAddress, TokenAddress}, infra::bad_token::{Heuristic, HeuristicState}};

pub async fn insert(ex: &mut PgConnection, solver: Address, tokens: Vec<(TokenAddress, HeuristicState)>) {
    let mut bad_tokens_to_insert = Vec::new();
    for token in tokens {
        bad_tokens_to_insert.push(BadToken{
            solver: database::Address(solver.0.0),
            token: database::Address(token.0.0.0.0),
            heuristic_state: Json(token.1),
        });
    }
    insert_to_db(ex, &bad_tokens_to_insert[..]).await;
}

pub async fn load(ex: &mut PgConnection) ->  sqlx::Result<Vec<BadTokenOut>> {
    fetch_all(ex).await
}

pub async fn load_token_list_for_solver(ex: &mut PgConnection, solver: &Address) -> HashMap<TokenAddress, HeuristicState> {
    let mut tokens = HashMap::new();

    if let Ok(token_list) = load_token_list(ex, database::Address(solver.0.0)).await {
        for token in token_list {
            tokens.insert(TokenAddress(ContractAddress(H160(token.0.0))), token.1);
        }
    }

    tokens
}

pub async fn cleanup(ex: &mut PgConnection, solver: &Address, timespan: u32, tokens: Vec<TokenAddress>) -> sqlx::Result<Vec<TokenAddress>>{
    let response = rejuvenate_bad_tokens(
        ex, 
        database::Address(solver.0.0), 
        timespan, 
        tokens
            .into_iter()
            .map(|x| database::Address(x.0.0))
            .collect_vec()
            .as_slice()
    ).await?
    .into_iter()
    .map(|x| TokenAddress(ContractAddress(H160(x.0))))
    .collect_vec();

    response
}


#[allow(dead_code)]
#[derive(sqlx::FromRow, Debug, Serialize, Deserialize)]
pub struct BadToken {
    pub solver: database::Address,
    pub token: database::Address,
    pub heuristic_state: Json<HeuristicState>
}


pub async fn fetch_all(ex: &mut PgConnection) -> Result<Vec<BadToken>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT solver, token, heuristic
FROM bad_tokens;
"#;

    sqlx::query_as(QUERY).fetch_all(ex).await
}

pub async fn load_token_list(
    ex: &mut PgConnection, 
    solver: database::Address
) -> sqlx::Result<BTreeMap<database::byte_array::ByteArray<20>, HeuristicState>> {
    const QUERY: &str = r#"
SELECT token, heuristic_state
FROM bad_tokens
where solver = $1;
"#;

    #[derive(sqlx::FromRow, Debug, Serialize, Deserialize)]
    struct Row {
        token: database::Address,
        heuristic_state: Json<HeuristicState>
    }

    let rows: Vec<Row> = sqlx::query_as(QUERY).bind(solver).fetch_all(ex).await?;

    let response = BTreeMap::new();
    for row in rows {
        response.insert(row.token, row.heuristic_state.0)
    } 

    response
}

pub async fn insert_to_db(ex: &mut PgConnection, tokens: &[BadToken]) -> sqlx::Result<()> {
    const QUERY: &str = r#"
INSERT INTO bad_tokens (solver, token, heuristic_state, time_stamp)
"#;
    let mut query_builder = QueryBuilder::new(QUERY);
    query_builder.push_values(tokens, |mut builder, token| {
        builder.push_bind(token.solver)
            .push_bind(token.token)
            .push_bind(token.heuristic_state)
            .push("NOW()");  
    });
    
    query_builder.push(" ON CONFLICT (solver, token) DO UPDATE SET heuristic_state = EXCLUDED.heuristic_state, time_stamp = NOW()");

    query_builder.build().execute(ex).await?;

    Ok(())
}

pub async fn rejuvenate_bad_tokens(
    ex: &mut PgConnection, 
    solver: database::Address, 
    timespan: u32, 
    tokens: &[database::Address]
) -> sqlx::Result<Vec<database::Address>> {
    let placeholder_vector = tokens
        .iter()
        .enumerate()
        .map(|(i, _)| format!("${}", i + 3))
        .collect::<Vec<_>>()
        .join(", ");

    let query = format!("
DELETE FROM bad_tokens
where solver = $1 
AND time_stamp < NOW() - INTERVAL '1 day' * $2 
AND token in ({})
RETURNING token;", placeholders);

    let sql_query = sqlx::query_as(&query).bind(solver).bind(timespan);
    
    for token in tokens {
        sql_query = sql_query.bind(*token)
    }

    sql_query.fetch_all(ex).await?
}