//! Shared scoring and ranking state markers.
//!
//! This module provides the type-state pattern infrastructure for tracking
//! solutions through the winner selection process:
//!
//! ```text
//! Unscored → Scored → Ranked
//! ```

/// This is the initial state when a solution enters the winner selection
/// process.
#[derive(Debug, Clone, Copy)]
pub struct Unscored;

/// This is the intermediate state after computing surplus and fees but before
/// ranking.
#[derive(Debug, Clone, Copy)]
pub struct Scored<Score> {
    pub score: Score,
}

/// This is the final state with complete information about whether the
/// solution is a winner or was filtered out.
#[derive(Debug, Clone, Copy)]
pub struct Ranked<Score> {
    pub rank_type: RankType,
    pub score: Score,
}

/// The type of ranking assigned to a state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankType {
    Winner,
    NonWinner,
    FilteredOut,
}

impl RankType {
    pub fn is_winner(self) -> bool {
        matches!(self, RankType::Winner)
    }

    pub fn is_filtered_out(self) -> bool {
        matches!(self, RankType::FilteredOut)
    }
}

/// Base trait for types that use the type-state pattern.
///
/// This trait provides the foundation for state transitions at compile time.
/// Types implementing this trait can transition between different states
/// (Unscored → Scored → Ranked) with compile-time guarantees.
///
/// # Type Parameters
///
/// - `State`: The current state of the type
/// - `Next<NewState>`: The type after transitioning to a new state
pub trait HasState {
    type State;
    type Next<NewState>;

    fn with_state<NewState>(self, state: NewState) -> Self::Next<NewState>;
    fn state(&self) -> &Self::State;
}

/// Trait for items in the Unscored state.
///
/// Items with this trait haven't been scored yet. This represents the initial
/// state when a solution enters the winner selection process.
///
/// # State Transition
///
/// Unscored → Scored (via `with_score()`)
pub trait UnscoredItem<Score>: HasState<State = Unscored> {
    fn with_score(self, score: Score) -> Self::Next<Scored<Score>>;
}

impl<T, Score> UnscoredItem<Score> for T
where
    T: HasState<State = Unscored>,
{
    fn with_score(self, score: Score) -> Self::Next<Scored<Score>> {
        self.with_state(Scored { score })
    }
}

/// Trait for items in the Scored state.
///
/// Items with this trait have been scored but not yet ranked. This is the
/// intermediate state after computing surplus/fees but before winner selection.
///
/// # State Transition
///
/// Scored → Ranked (via `with_rank()`)
///
/// # Methods
///
/// - `score()`: Access the computed score
/// - `with_rank()`: Transition to Ranked state with a rank type
pub trait ScoredItem<Score: Copy>: HasState<State = Scored<Score>> {
    fn score(&self) -> Score;
    fn with_rank(self, rank_type: RankType) -> Self::Next<Ranked<Score>>
    where
        Self: Sized;
}

impl<T, Score> ScoredItem<Score> for T
where
    Score: Copy,
    T: HasState<State = Scored<Score>>,
{
    fn score(&self) -> Score {
        self.state().score
    }

    fn with_rank(self, rank_type: RankType) -> Self::Next<Ranked<Score>> {
        let score = self.state().score;
        self.with_state(Ranked { rank_type, score })
    }
}

/// Trait for items in the Ranked state.
///
/// Items with this trait have been scored and ranked in the winner selection
/// process. This is the final state with complete information about whether
/// the solution is a winner or was filtered out.
///
/// # Methods
///
/// - `score()`: Access the computed score
/// - `is_winner()`: Check if this is a winning solution
/// - `is_filtered_out()`: Check if this was filtered out (unfair)
pub trait RankedItem<Score: Copy>: HasState<State = Ranked<Score>> {
    fn score(&self) -> Score;
    fn is_winner(&self) -> bool;
    fn is_filtered_out(&self) -> bool;
}

impl<T, Score> RankedItem<Score> for T
where
    Score: Copy,
    T: HasState<State = Ranked<Score>>,
{
    fn score(&self) -> Score {
        self.state().score
    }

    fn is_winner(&self) -> bool {
        self.state().rank_type.is_winner()
    }

    fn is_filtered_out(&self) -> bool {
        self.state().rank_type.is_filtered_out()
    }
}
