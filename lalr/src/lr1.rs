use crate::grammar::{FirstSets, Grammar, Rhs, Symbol};

use std::collections::{btree_set, BTreeMap, BTreeSet};
use std::iter::FromIterator;

#[derive(Debug)]
pub struct LR1Parser<'g, T: 'g, N: 'g, A: 'g> {
    table: LR1Table<'g, T, N, A>,
}

#[derive(Debug)]
pub struct LR1Table<'g, T: 'g, N: 'g, A: 'g> {
    pub states: Vec<LR1State<'g, T, N, A>>,
    pub initial: usize,
}

/// State in an LR(1) automaton.
#[derive(Debug)]
pub struct LR1State<'g, T: 'g, N: 'g, A: 'g> {
    /// Map of actions to be taken on terminals. Terminals with no action have no map entry.
    pub actions: BTreeMap<&'g T, LR1Action<'g, T, N, A>>,
    /// Action to taken when lookahead is endmarker symbol.
    pub endmarker: Option<LR1Action<'g, T, N, A>>,
    /// Map of GOTO transitions to other states. Nonterminals with no GOTO have no map entry.
    pub goto: BTreeMap<&'g N, usize>,
}

#[derive(Debug)]
/// LR(1) action to be taken for some terminal.
pub enum LR1Action<'g, T: 'g, N: 'g, A: 'g> {
    /// Reduce a production.
    Reduce(&'g N, &'g Rhs<T, N, A>),
    /// Shift to some state.
    Shift(usize),
    /// Accept the input.
    Accept,
}

/// A conflict encountered when constructing an LR(1) parse table.
#[derive(Debug, Clone)]
pub enum LRConflict<'g, T: 'g, N: 'g, A: 'g> {
    /// Shift-reduce conflict
    ShiftReduce {
        /// Shift action involved in the conflict.
        /// 0: Terminal to shift on; endmarker terminal if [`None`].
        /// 1: Destination state of the shift.
        shift: (Option<&'g T>, usize),
        /// Reduce rule involved in the conflict.
        reduce: (&'g N, &'g Rhs<T, N, A>),
    },
    /// Reduce-reduce conflict
    ReduceReduce {
        r1: (&'g N, &'g Rhs<T, N, A>),
        r2: (&'g N, &'g Rhs<T, N, A>),
    },
}

impl<'g, T: 'g, N: 'g, A: 'g> LR1State<'g, T, N, A> {
    /// Insert an action for a symbol, returning an [`LRConflict`] error some action already
    /// exists for that symbol.
    ///
    /// If `sy` is [`None`], it is interpreted as the endmarker terminal.
    pub fn set_action(
        &mut self,
        sy: Option<&'g T>,
        action: LR1Action<'g, T, N, A>,
    ) -> Result<(), LRConflict<'g, T, N, A>>
    where
        T: Ord,
    {
        match sy {
            Some(sy) => {
                // Check for existing action; if there is one, there is a conflict.
                // If no existing, set the action.
                match self.actions.get(sy) {
                    Some(existing) => {
                        // Only reduce-reduce and shift-reduce should occur.
                        let conflict =
                            Self::determine_conflict(existing, &action, Some(sy)).unwrap();
                        Err(conflict)
                    }
                    None => {
                        self.actions.insert(sy, action);
                        Ok(())
                    }
                }
            }
            // sy is endmarker terminal.
            None => match &self.endmarker {
                Some(existing) => {
                    let conflict = Self::determine_conflict(&existing, &action, None).unwrap();
                    Err(conflict)
                }
                None => {
                    self.endmarker = Some(action);
                    Ok(())
                }
            },
        }
    }

    fn determine_conflict(
        a1: &LR1Action<'g, T, N, A>,
        a2: &LR1Action<'g, T, N, A>,
        sy: Option<&'g T>,
    ) -> Option<LRConflict<'g, T, N, A>> {
        match *a1 {
            LR1Action::Reduce(n1, rhs1) => match *a2 {
                LR1Action::Reduce(n2, rhs2) => Some(LRConflict::ReduceReduce {
                    r1: (n1, rhs1),
                    r2: (n2, rhs2),
                }),
                LR1Action::Shift(dest2) => Some(LRConflict::ShiftReduce {
                    shift: (sy, dest2),
                    reduce: (n1, rhs1),
                }),
                _ => None,
            },
            LR1Action::Shift(dest1) => match *a2 {
                LR1Action::Reduce(n2, rhs2) => Some(LRConflict::ShiftReduce {
                    shift: (sy, dest1),
                    reduce: (n2, rhs2),
                }),
                _ => None,
            },
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct LR1Item<'g, T: 'g, N: 'g, A: 'g> {
    pub lhs: &'g N,
    pub rhs: &'g Rhs<T, N, A>,

    /// Position of item, equal to the index of the next symbol.
    pub pos: usize,
    pub lookahead: Option<&'g T>,
}

comparators!(LR1Item('g, T, N, A), (T, N), (lhs, rhs, pos, lookahead));

impl<'g, T: 'g, N: 'g, A: 'g> LR1Item<'g, T, N, A> {
    /// Retrieves B for A -> a.Bb, or None if A -> a.
    pub fn next_symbol(&self) -> Option<&'g Symbol<T, N>> {
        self.rhs.body.get(self.pos)
    }
}

impl<'g, T: 'g, N: 'g, A: 'g> Clone for LR1Item<'g, T, N, A> {
    fn clone(&self) -> Self {
        Self {
            lhs: self.lhs,
            rhs: self.rhs,
            pos: self.pos,
            lookahead: self.lookahead,
        }
    }
}

#[derive(Debug)]
pub struct LR1ItemSet<'g, T: 'g, N: 'g, A: 'g> {
    pub items: BTreeSet<LR1Item<'g, T, N, A>>,
}

comparators!(LR1ItemSet('g, T, N, A), (T, N), (items));

impl<'g, T: 'g, N: 'g, A: 'g> LR1ItemSet<'g, T, N, A>
where
    LR1Item<'g, T, N, A>: Ord,
{
    pub fn new() -> Self {
        Self {
            items: BTreeSet::new(),
        }
    }

    pub fn insert(&mut self, item: LR1Item<'g, T, N, A>) -> bool {
        self.items.insert(item)
    }

    pub fn append(&mut self, set: &mut Self) {
        self.items.append(&mut set.items);
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Iterate through the items in this LR1ItemSet.
    pub fn iter(&self) -> btree_set::Iter<LR1Item<'g, T, N, A>> {
        self.items.iter()
    }
}

impl<'g, T: 'g, N: 'g, A: 'g> Clone for LR1ItemSet<'g, T, N, A> {
    fn clone(&self) -> Self {
        Self {
            items: self.items.clone(),
        }
    }
}

impl<'g, T: 'g, N: 'g, A: 'g> IntoIterator for LR1ItemSet<'g, T, N, A> {
    type Item = LR1Item<'g, T, N, A>;
    type IntoIter = std::collections::btree_set::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl<'g, T: 'g, N: 'g, A: 'g> FromIterator<LR1Item<'g, T, N, A>> for LR1ItemSet<'g, T, N, A>
where
    T: Ord,
    N: Ord,
{
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = LR1Item<'g, T, N, A>>,
    {
        let mut items = BTreeSet::new();
        items.extend(iter);
        Self { items }
    }
}

impl<T, N, A> Grammar<T, N, A>
where
    T: Ord,
    N: Ord,
{
    pub fn lr1_goto<'g>(
        &'g self,
        set: &LR1ItemSet<'g, T, N, A>,
        x: &'g Symbol<T, N>,
        first_sets: &FirstSets<'g, T, N>,
    ) -> LR1ItemSet<'g, T, N, A> {
        let mut new_set = LR1ItemSet::new();

        // For each item [A -> α·Xβ, a] in I, add item [A -> aX·β, a] to set J.
        for item in set.iter() {
            let post_dot = match item.next_symbol() {
                Some(sy) => sy,
                None => continue,
            };

            if *post_dot != *x {
                continue;
            }

            new_set.insert(LR1Item {
                pos: item.pos + 1,
                ..*item
            });
        }

        self.lr1_closure(&mut new_set, first_sets);
        return new_set;
    }

    /// Compute the LR(1) closure set for the given LR(1) item set.
    pub fn lr1_closure<'g>(
        &'g self,
        set: &mut LR1ItemSet<'g, T, N, A>,
        first_sets: &FirstSets<'g, T, N>,
    ) {
        let mut changed = true;
        while changed {
            changed = false;
            let mut added = BTreeSet::new();
            // For each item [A -> α·Bβ, a] in I where B is a nonterminal.
            for item in set.iter() {
                // Extract B.
                let b = match item.next_symbol() {
                    Some(sy) => match sy {
                        Symbol::Nonterminal(ref n) => n,
                        Symbol::Terminal(_) => continue,
                    },
                    None => continue,
                };

                // For each production B -> γ in G'.
                let b_productions = self.rules.get(b).unwrap();

                if !b_productions.is_empty() {
                    // Compute FIRST(βa).
                    let first_beta_a = {
                        // Extract β (all symbols after B).
                        let a = item.lookahead;
                        let beta = &item.rhs.body[(item.pos + 1)..];

                        let mut first_set = BTreeSet::new();
                        let mut nullable = true;
                        // Add to FIRST set until the current symbol's FIRST set is not nullable.
                        for sy in beta {
                            if !nullable {
                                break;
                            }

                            match sy {
                                // FIRST(t) where t is a terminal is never nullable, add to set and
                                // stop.
                                Symbol::Terminal(ref t) => {
                                    first_set.insert(t);
                                    nullable = false;
                                }
                                Symbol::Nonterminal(ref n) => {
                                    // Get FIRST(n) of the nonterminal n and add its terminals to
                                    // the total FIRST set.
                                    let (sy_first, sy_nullable) = first_sets.get(n).unwrap();
                                    first_set.extend(sy_first);
                                    if !sy_nullable {
                                        nullable = false;
                                    }
                                }
                            }
                        }

                        let mut first_set: BTreeSet<_> =
                            first_set.into_iter().map(|t| Some(t)).collect();

                        // If all of β was nullable, consider the terminal a.
                        if nullable {
                            first_set.insert(a);
                        }

                        first_set
                    };

                    for rhs in b_productions {
                        // For each terminal b in FIRST(βa), add [B -> ·γ, b] to set I.
                        for bt in &first_beta_a {
                            added.insert(LR1Item {
                                lhs: b,
                                rhs,
                                pos: 0,
                                lookahead: bt.clone(),
                            });
                        }
                    }
                }
            }

            if !added.is_empty() {
                for item in added.into_iter() {
                    if set.insert(item) {
                        changed = true;
                    }
                }
            }
        }
    }

    /// Construct an SLR(1) parse table for the grammar.
    pub fn slr1_table<'g>(&'g self) -> Result<LR1Table<'g, T, N, A>, LRConflict<'g, T, N, A>> {
        let lr0_automaton = self.lr0_automaton();
        let follow_sets = self.follow_sets(None);

        // New states in the LR(1) table.
        let mut states = Vec::new();

        for lr0_state in lr0_automaton.states {
            let mut lr1_state = LR1State {
                actions: BTreeMap::new(),
                endmarker: None,
                goto: BTreeMap::new(),
            };

            for (sy, dest) in lr0_state.transitions {
                match *sy {
                    // If [A -> α.aβ] is in I_i and GOTO(I_i, a) = I_j and a is a terminal, then
                    // set ACTION[i, a] to "shift j".
                    Symbol::Terminal(ref t) => {
                        lr1_state.set_action(Some(t), LR1Action::Shift(dest))?;
                    }
                    // If GOTO(I_i, A) = I_j for nonterminal A, then GOTO[i, A] = j.
                    Symbol::Nonterminal(ref n) => {
                        lr1_state.goto.insert(n, dest);
                    }
                }
            }

            for item in lr0_state.items {
                // If [A -> α.] is in I_i, then set ACTION[i, a] to "reduce A -> α" for all a in
                // FOLLOW(A), unless A is S'.
                if item.pos == item.rhs.body.len() {
                    if *item.lhs != self.start {
                        let (follow_set, endmarker) = follow_sets.get(item.lhs).unwrap();
                        for sy in follow_set {
                            lr1_state
                                .set_action(Some(sy), LR1Action::Reduce(item.lhs, item.rhs))?;
                        }

                        if *endmarker {
                            lr1_state.set_action(None, LR1Action::Reduce(item.lhs, item.rhs))?;
                        }
                    } else {
                        // If [S' -> S.] is in I_i, then set ACTION[i, $] to "accept".
                        lr1_state.set_action(None, LR1Action::Accept)?;
                    }
                }
            }

            states.push(lr1_state);
        }

        Ok(LR1Table {
            states,
            // The initial state of the parser is the one constructed from the set of items
            // containing [S' -> .S].
            initial: lr0_automaton.start,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        Grammar, Rhs,
        Symbol::{Nonterminal as NT, Terminal as TT},
    };

    use std::collections::BTreeMap;

    use Nonterminal::*;
    use Terminal::*;

    #[test]
    fn test_lr1_closure() {
        let mut rules = BTreeMap::new();

        // E -> S
        let start_rhs = Rhs::noop(vec![NT(S)]);
        rules.insert(E, vec![start_rhs.clone()]);

        // S -> L = R
        //    | R
        let l_eq_r = Rhs::noop(vec![NT(L), TT(Equ), NT(R)]);
        let r = Rhs::noop(vec![NT(R)]);
        rules.insert(S, vec![l_eq_r, r]);

        // L -> * R
        //    | id
        let deref_r = Rhs::noop(vec![TT(Deref), NT(R)]);
        let id = Rhs::noop(vec![TT(Id)]);
        rules.insert(L, vec![deref_r, id]);

        // R -> L
        let l = Rhs::noop(vec![NT(L)]);
        rules.insert(R, vec![l]);

        let grammar = Grammar::new(E, rules).unwrap();

        // Compute CLOSURE({[E -> ·S, #]})
        let mut initial_set = LR1ItemSet::new();
        initial_set.insert(LR1Item {
            lhs: &E,
            rhs: &start_rhs,
            pos: 0,
            lookahead: None,
        });

        grammar.lr1_closure(&mut initial_set, &grammar.first_sets());

        assert_eq!(8, initial_set.items.len());
    }

    #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
    enum Nonterminal {
        E,
        S,
        L,
        R,
    }

    #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
    enum Terminal {
        Equ,
        Deref,
        Id,
    }
}