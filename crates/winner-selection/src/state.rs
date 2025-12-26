//! Shared scoring and ranking state markers.

/// Solution/participant that hasn't been scored yet.
#[derive(Debug, Clone, Copy)]
pub struct Unscored;

/// Solution/participant with a computed score.
#[derive(Debug, Clone, Copy)]
pub struct Scored<Score> {
    pub score: Score,
}

/// Solution/participant with ranking information.
#[derive(Debug, Clone, Copy)]
pub struct Ranked<Score> {
    pub rank_type: RankType,
    pub score: Score,
}

/// The type of ranking assigned to a solution/participant.
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

pub trait WithState {
    type State;
    type WithState<NewState>;

    fn with_state<NewState>(self, state: NewState) -> Self::WithState<NewState>;
    fn state(&self) -> &Self::State;
}

pub trait UnscoredItem<Score>: WithState<State = Unscored> {
    fn with_score(self, score: Score) -> Self::WithState<Scored<Score>>;
}

impl<T, Score> UnscoredItem<Score> for T
where
    T: WithState<State = Unscored>,
{
    fn with_score(self, score: Score) -> Self::WithState<Scored<Score>> {
        self.with_state(Scored { score })
    }
}

pub trait ScoredItem<Score: Copy>: WithState<State = Scored<Score>> {
    fn score(&self) -> Score;
    fn rank(self, rank_type: RankType) -> Self::WithState<Ranked<Score>>
    where
        Self: Sized;
}

impl<T, Score> ScoredItem<Score> for T
where
    Score: Copy,
    T: WithState<State = Scored<Score>>,
{
    fn score(&self) -> Score {
        self.state().score
    }

    fn rank(self, rank_type: RankType) -> Self::WithState<Ranked<Score>> {
        let score = self.state().score;
        self.with_state(Ranked { rank_type, score })
    }
}

pub trait RankedItem<Score: Copy>: WithState<State = Ranked<Score>> {
    fn score(&self) -> Score;
    fn is_winner(&self) -> bool;
    fn is_filtered_out(&self) -> bool;
    fn filtered_out(&self) -> bool {
        self.is_filtered_out()
    }
}

impl<T, Score> RankedItem<Score> for T
where
    Score: Copy,
    T: WithState<State = Ranked<Score>>,
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
