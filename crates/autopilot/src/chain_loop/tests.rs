use {
    super::{
        AlwaysLeader,
        AuctionInfo,
        AuctionLoop,
        AuctionProvider,
        Chain,
        CycleTrigger,
        Leadership,
        RankingInfo,
        SettlementExecutor,
        SettlementObserver,
        SolverCompetition,
        WinnerSelection,
        evm::{self, EvmChain},
        solana,
    },
    crate::{
        domain,
        domain::competition::{Bid, Unscored, winner_selection},
        run::Liveness,
        run_loop::Probes,
    },
    alloy::primitives::address,
    async_trait::async_trait,
    ethrpc::block_stream::BlockInfo,
    std::{
        collections::{HashSet, VecDeque},
        sync::{
            Arc,
            Mutex,
            atomic::{AtomicBool, Ordering},
        },
        time::Duration,
    },
};

type Log = Arc<Mutex<Vec<String>>>;

fn record(log: &Log, entry: impl Into<String>) {
    log.lock().unwrap().push(entry.into());
}

fn test_probes() -> Probes {
    Probes {
        liveness: Arc::new(Liveness::new(Duration::from_secs(1000))),
        startup: Arc::new(Some(AtomicBool::new(false))),
    }
}

fn sorted(uids: HashSet<u64>) -> Vec<u64> {
    let mut uids: Vec<_> = uids.into_iter().collect();
    uids.sort();
    uids
}

// third, test only chain proving the orchestration runs on anything
// satisfying the vocabulary

struct MockChain;

#[derive(Clone, Copy, PartialEq, Debug)]
struct MockTip(u64);

#[derive(Clone, Debug)]
struct MockAuction {
    id: i64,
    orders: Vec<u64>,
}

impl PartialEq for MockAuction {
    // dedupe ignores the allocated id, same as domain::Auction
    fn eq(&self, other: &Self) -> bool {
        self.orders == other.orders
    }
}

struct MockSolution;

struct MockRanking {
    winning: Vec<u64>,
    considered: Vec<u64>,
}

impl Chain for MockChain {
    type Auction = MockAuction;
    type AuctionId = i64;
    type OrderUid = u64;
    type Ranking = MockRanking;
    type Solution = MockSolution;
    type SubmissionDeadline = u64;
    type Tip = MockTip;
}

impl AuctionInfo<MockChain> for MockAuction {
    fn id(&self) -> i64 {
        self.id
    }
}

impl RankingInfo<MockChain> for MockRanking {
    fn winner_count(&self) -> usize {
        usize::from(!self.winning.is_empty())
    }

    fn winning_order_uids(&self) -> HashSet<u64> {
        self.winning.iter().copied().collect()
    }

    fn considered_order_uids(&self) -> HashSet<u64> {
        self.considered.iter().copied().collect()
    }
}

struct MockLeadership {
    log: Log,
    is_leader: VecDeque<bool>,
}

#[async_trait]
impl Leadership for MockLeadership {
    async fn try_acquire(&mut self) -> bool {
        let leader = self.is_leader.pop_front().unwrap_or(true);
        record(&self.log, format!("acquire leader={leader}"));
        leader
    }

    async fn release(&mut self) {
        record(&self.log, "release");
    }
}

struct MockTrigger {
    log: Log,
    tips: VecDeque<MockTip>,
    current: MockTip,
}

#[async_trait]
impl CycleTrigger<MockChain> for MockTrigger {
    async fn next_cycle(&mut self) -> MockTip {
        let tip = self.tips.pop_front().expect("test drove too many cycles");
        record(&self.log, format!("next_cycle tip={}", tip.0));
        tip
    }

    fn current_tip(&self) -> MockTip {
        record(&self.log, format!("current_tip tip={}", self.current.0));
        self.current
    }
}

struct MockProvider {
    log: Log,
    auctions: Mutex<VecDeque<Option<MockAuction>>>,
}

#[async_trait]
impl AuctionProvider<MockChain> for MockProvider {
    async fn sync_to_tip(&self, tip: &MockTip, is_leader: bool) -> anyhow::Result<()> {
        record(&self.log, format!("sync tip={} leader={is_leader}", tip.0));
        Ok(())
    }

    async fn cut_auction(&self, _tip: &MockTip) -> Option<MockAuction> {
        let auction = self
            .auctions
            .lock()
            .unwrap()
            .pop_front()
            .expect("test drove too many cuts");
        match &auction {
            Some(auction) => record(&self.log, format!("cut auction={}", auction.id)),
            None => record(&self.log, "cut none"),
        }
        auction
    }
}

struct MockCompetition {
    log: Log,
    solution_counts: Mutex<VecDeque<usize>>,
}

#[async_trait]
impl SolverCompetition<MockChain> for MockCompetition {
    async fn solve(&self, auction: &MockAuction) -> Vec<MockSolution> {
        let count = self
            .solution_counts
            .lock()
            .unwrap()
            .pop_front()
            .expect("test drove too many solves");
        record(
            &self.log,
            format!("solve auction={} solutions={count}", auction.id),
        );
        (0..count).map(|_| MockSolution).collect()
    }
}

struct MockWinnerSelection {
    log: Log,
}

impl WinnerSelection<MockChain> for MockWinnerSelection {
    fn arbitrate(&self, solutions: Vec<MockSolution>, auction: &MockAuction) -> MockRanking {
        record(
            &self.log,
            format!(
                "arbitrate auction={} solutions={}",
                auction.id,
                solutions.len()
            ),
        );
        MockRanking {
            winning: vec![1, 2],
            // order 2 also appears in a winning solution, the loop must
            // subtract it before marking Considered
            considered: vec![2, 3],
        }
    }
}

struct MockExecutor {
    log: Log,
}

#[async_trait]
impl SettlementExecutor<MockChain> for MockExecutor {
    fn submission_deadline(&self, tip: &MockTip) -> u64 {
        record(&self.log, format!("deadline tip={}", tip.0));
        tip.0 + 100
    }

    async fn execute(&self, auction_id: i64, _ranking: &MockRanking, deadline: u64) {
        record(
            &self.log,
            format!("execute auction={auction_id} deadline={deadline}"),
        );
    }
}

struct MockObserver {
    log: Log,
}

#[async_trait]
impl SettlementObserver<MockChain> for MockObserver {
    fn orders_ready(&self, auction: &MockAuction) {
        record(&self.log, format!("ready auction={}", auction.id));
    }

    async fn competition_ranked(
        &self,
        auction: &MockAuction,
        tip: &MockTip,
        _ranking: &MockRanking,
        deadline: u64,
    ) -> anyhow::Result<()> {
        record(
            &self.log,
            format!(
                "ranked auction={} tip={} deadline={deadline}",
                auction.id, tip.0
            ),
        );
        Ok(())
    }

    fn orders_matched(&self, executing: HashSet<u64>, considered: HashSet<u64>) {
        record(
            &self.log,
            format!(
                "matched executing={:?} considered={:?}",
                sorted(executing),
                sorted(considered)
            ),
        );
    }

    fn competition_ended(&self, auction: &MockAuction, _ranking: &MockRanking) {
        record(&self.log, format!("ended auction={}", auction.id));
    }
}

/// Drives the generic loop through five cycles over a mock chain and
/// asserts the exact phase sequence of run_loop.rs, including the dedupe
/// and its one cycle lag on the tip marker.
#[tokio::test]
async fn phases_run_in_the_order_of_the_real_loop() {
    let log = Log::default();
    let auction = |id| MockAuction {
        id,
        orders: vec![1, 2, 3],
    };

    let mut auction_loop: AuctionLoop<MockChain> = AuctionLoop::new(
        Box::new(MockTrigger {
            log: log.clone(),
            tips: [MockTip(1), MockTip(1), MockTip(1), MockTip(2), MockTip(3)].into(),
            current: MockTip(9),
        }),
        Box::new(MockProvider {
            log: log.clone(),
            auctions: Mutex::new(
                [
                    Some(auction(1)),
                    Some(auction(2)),
                    Some(auction(3)),
                    Some(auction(4)),
                ]
                .into(),
            ),
        }),
        Box::new(MockCompetition {
            log: log.clone(),
            solution_counts: Mutex::new([1, 0, 0].into()),
        }),
        Box::new(MockWinnerSelection { log: log.clone() }),
        Box::new(MockExecutor { log: log.clone() }),
        Box::new(MockObserver { log: log.clone() }),
        Box::new(MockLeadership {
            log: log.clone(),
            is_leader: [true, true, true, true, false].into(),
        }),
        test_probes(),
    );

    for _ in 0..5 {
        auction_loop.run_cycle().await;
    }

    let expected = vec![
        // cycle 1, fresh auction, full competition
        "acquire leader=true",
        "next_cycle tip=1",
        "sync tip=1 leader=true",
        "cut auction=1",
        "ready auction=1",
        "solve auction=1 solutions=1",
        "arbitrate auction=1 solutions=1",
        "current_tip tip=9",
        "deadline tip=9",
        "ranked auction=1 tip=9 deadline=109",
        "matched executing=[1, 2] considered=[3]",
        "execute auction=1 deadline=109",
        "ended auction=1",
        // cycle 2, identical auction on the same tip still runs once more
        // because the tip marker is written one cycle behind the auction
        // marker (run_loop.rs:291-295)
        "acquire leader=true",
        "next_cycle tip=1",
        "sync tip=1 leader=true",
        "cut auction=2",
        "ready auction=2",
        "solve auction=2 solutions=0",
        // cycle 3, now the dedupe kicks in right after cutting
        "acquire leader=true",
        "next_cycle tip=1",
        "sync tip=1 leader=true",
        "cut auction=3",
        // cycle 4, same auction but a new tip reruns the competition
        "acquire leader=true",
        "next_cycle tip=2",
        "sync tip=2 leader=true",
        "cut auction=4",
        "ready auction=4",
        "solve auction=4 solutions=0",
        // cycle 5, followers stop after warming the caches
        "acquire leader=false",
        "next_cycle tip=3",
        "sync tip=3 leader=false",
    ];
    assert_eq!(*log.lock().unwrap(), expected);
}

// EVM typed stub seams for constructing AuctionLoop<EvmChain> without a
// database or an RPC node. The vocabulary types are the real ones.

struct EvmStubTrigger;

#[async_trait]
impl CycleTrigger<EvmChain> for EvmStubTrigger {
    async fn next_cycle(&mut self) -> BlockInfo {
        BlockInfo::default()
    }

    fn current_tip(&self) -> BlockInfo {
        BlockInfo::default()
    }
}

struct EvmStubProvider {
    log: Log,
}

#[async_trait]
impl AuctionProvider<EvmChain> for EvmStubProvider {
    async fn sync_to_tip(&self, _tip: &BlockInfo, _is_leader: bool) -> anyhow::Result<()> {
        Ok(())
    }

    async fn cut_auction(&self, _tip: &BlockInfo) -> Option<domain::Auction> {
        record(&self.log, "cut");
        Some(domain::Auction {
            id: 1,
            block: 0,
            orders: vec![],
            prices: Default::default(),
            surplus_capturing_jit_order_owners: vec![],
        })
    }
}

struct EvmStubCompetition {
    log: Log,
}

#[async_trait]
impl SolverCompetition<EvmChain> for EvmStubCompetition {
    async fn solve(&self, _auction: &domain::Auction) -> Vec<Bid<Unscored>> {
        record(&self.log, "solve");
        vec![]
    }
}

struct EvmStubObserver {
    log: Log,
}

#[async_trait]
impl SettlementObserver<EvmChain> for EvmStubObserver {
    fn orders_ready(&self, _auction: &domain::Auction) {
        record(&self.log, "ready");
    }

    async fn competition_ranked(
        &self,
        _auction: &domain::Auction,
        _tip: &BlockInfo,
        _ranking: &winner_selection::Ranking,
        _deadline: u64,
    ) -> anyhow::Result<()> {
        record(&self.log, "ranked");
        Ok(())
    }

    fn orders_matched(
        &self,
        _executing: HashSet<domain::OrderUid>,
        _considered: HashSet<domain::OrderUid>,
    ) {
        record(&self.log, "matched");
    }

    fn competition_ended(&self, _auction: &domain::Auction, _ranking: &winner_selection::Ranking) {
        record(&self.log, "ended");
    }
}

/// Instantiates the loop for both chains in one scope. The EVM instance
/// uses the real vocabulary types plus the real winner selection and
/// executor adapters and actually runs cycles, the Solana instance proves
/// the abstraction admits a second chain.
#[tokio::test]
async fn instantiates_for_evm_and_solana() {
    let _solana: AuctionLoop<solana::SolanaChain> = AuctionLoop::new(
        Box::new(solana::SolanaCycleTrigger),
        Box::new(solana::SolanaAuctionProvider),
        Box::new(solana::SolanaSolverCompetition),
        Box::new(solana::SolanaWinnerSelection::new(
            10,
            ::winner_selection::solana::Pubkey([0xff; 32]),
        )),
        Box::new(solana::SolanaSettlementExecutor),
        Box::new(solana::SolanaSettlementObserver),
        Box::new(AlwaysLeader),
        test_probes(),
    );

    let log = Log::default();
    let probes = test_probes();
    let startup = probes.startup.clone();
    let mut evm_loop: AuctionLoop<EvmChain> = AuctionLoop::new(
        Box::new(EvmStubTrigger),
        Box::new(EvmStubProvider { log: log.clone() }),
        Box::new(EvmStubCompetition { log: log.clone() }),
        Box::new(evm::EvmWinnerSelection::new(
            10,
            address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").into(),
        )),
        Box::new(evm::EvmSettlementExecutor {
            submission_deadline_blocks: 5,
        }),
        Box::new(EvmStubObserver { log: log.clone() }),
        Box::new(AlwaysLeader),
        probes,
    );

    for _ in 0..3 {
        evm_loop.run_cycle().await;
    }

    // cycle 1 runs, cycle 2 repeats because the tip marker lags one cycle,
    // cycle 3 is deduped after cutting
    let expected = vec!["cut", "ready", "solve", "cut", "ready", "solve", "cut"];
    assert_eq!(*log.lock().unwrap(), expected);
    assert!(
        startup
            .as_ref()
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::Acquire))
    );
}

/// The composition proof: the WinnerSelection seam (spike 1) executes the
/// shared generic arbitrator (spike 2) over Solana types. Same algorithm
/// crate the EVM loop uses, no Solana copy of the logic.
#[test]
fn solana_seam_runs_the_shared_arbitrator() {
    use ::winner_selection::{
        solana::{IntentHash, Pubkey},
        solution::Solution as WsSolution,
        state::RankedItem,
    };

    let uid = IntentHash([1; 32]);
    let (mint_a, mint_b) = (Pubkey([1; 32]), Pubkey([2; 32]));
    let order = |executed_buy: u64| ::winner_selection::solution::Order {
        uid,
        sell_token: mint_a,
        buy_token: mint_b,
        sell_amount: 100,
        buy_amount: 90,
        executed_sell: 100,
        executed_buy,
        side: ::winner_selection::Side::Sell,
    };
    let auction = solana::SolanaAuction {
        id: 7,
        slot: 1000,
        orders: vec![solana::SolanaOrder {
            uid,
            owner: Pubkey([9; 32]),
        }],
        prices: [(mint_b, 1_000_000_000u64)].into(),
    };
    let solutions = vec![
        WsSolution::new(1, Pubkey([3; 32]), vec![order(95)]),
        WsSolution::new(2, Pubkey([4; 32]), vec![order(92)]),
    ];

    let seam = solana::SolanaWinnerSelection::new(10, Pubkey([0xff; 32]));
    let ranking = WinnerSelection::<solana::SolanaChain>::arbitrate(&seam, solutions, &auction);

    // The generic arbitrator picked one winner (uniform directional price)
    // and the loop-facing RankingInfo view feeds the bookkeeping sets.
    assert_eq!(
        ranking.winners().map(|s| s.id()).collect::<Vec<_>>(),
        vec![1]
    );
    assert_eq!(ranking.ranked[0].score(), 5);
    assert_eq!(
        RankingInfo::<solana::SolanaChain>::winning_order_uids(&ranking),
        HashSet::from([uid])
    );
    assert_eq!(
        RankingInfo::<solana::SolanaChain>::winner_count(&ranking),
        1
    );
}
