use crate::grammar::FirstSets;
use crate::lr0::Item;
use crate::lr1::{LR1Table, LRConflict};
use crate::{Grammar, Symbol};

use std::collections::BTreeSet;

/// Enum to represent three possible lookahead types when computing LALR(1) item kernels.
#[derive(Debug, Ord, PartialOrd, Eq, PartialEq)]
enum LR1Lookahead<'a, T> {
    /// A terminal from the grammar.
    Terminal(&'a T),
    /// The endmarker terminal.
    Endmarker,
    /// A symbol not in the grammar (#), used to determine which items lookaheads are propogated
    /// to.
    NonSymbol,
}

impl<T, N, A> Grammar<T, N, A>
where
    T: Ord,
    N: Ord,
{
    /// Construct an LALR(1) parse table for the grammar.
    ///
    /// Implements **Algorithm 4.63** to efficiently compute the kernels of the LALR(1) collection
    /// of item sets for a grammar.
    pub fn lalr1_table<'a>(&'a self) -> Result<LR1Table<'a, T, N, A>, LRConflict<'a, T, N, A>> {
        // Compute the LR(0) item set.
        let mut lr0_automaton = self.lr0_automaton();

        // Compute the first sets.
        let first_sets = self.first_sets();

        // Remove non-kernel items.
        for state in lr0_automaton.states.iter_mut() {
            state.items = state
                .items
                .iter()
                // Kernel items include the initial item, S' -> .S, and all items whose dots are
                // not at the left end.
                .filter(|item| *item.lhs == self.start || item.pos != 0)
                .cloned()
                .collect();
        }

        // Determine lookaheads spontaneously generated by items in I for kernel items in GOTO(I,X)
        // and the items in I from which lookaheads are propagated to kernel items in GOTO(I,X).
        for kernel in lr0_automaton.states {
            for item in kernel.items {
                // J := CLOSURE({[A -> α.β, #]})
                let closure = {
                    let mut init_set = BTreeSet::new();
                    init_set.insert((item.clone(), LR1Lookahead::NonSymbol));
                    self.lr1_closure(&mut init_set, &first_sets);
                    init_set
                };

                for (new_item, new_la) in closure {
                    let next_symbol = match new_item.next_symbol() {
                        Some(sy) => sy,
                        None => continue,
                    };

                    match new_la {
                        // If [B -> γ·Xδ, a] is in J, and a is not #, conclude that lookahead a is
                        // generated spontaneously for item B -> γX·δ in GOTO(I, X).
                        LR1Lookahead::Terminal(t) => {}
                        LR1Lookahead::Endmarker => {}
                        // If [B -> γ·Xδ, #] is in J,  conclude that lookahead a is generated
                        // spontaneously for item B -> γX·δ in GOTO(I, X).
                        LR1Lookahead::NonSymbol => {}
                    }
                }
            }
        }

        Ok(LR1Table {
            states: Vec::new(),
            initial: 0,
        })
    }

    fn lr1_closure<'a>(
        &'a self,
        items: &mut BTreeSet<(Item<'a, T, N, A>, LR1Lookahead<'a, T>)>,
        first_sets: &FirstSets<'a, T, N>,
    ) {
        let mut added = BTreeSet::new();
        // For each item [A -> α.Bβ, a] in I
        for (item, lookahead) in items.iter() {
            // For each production B -> γ in G'
            let next_symbol = match item.next_symbol() {
                Some(sy) => match sy {
                    Symbol::Nonterminal(ref n) => n,
                    Symbol::Terminal(_) => continue,
                },
                None => continue,
            };

            // rhs = γ
            for rhs in self.rules.get(next_symbol).unwrap() {
                // For each terminal t in FIRST(βa).
                // TODO: Memoize this.

                // Extract β from item rhs.
                let beta = &rhs.body[(item.pos + 1)..];
                let mut first_set = BTreeSet::new();

                // Flag to determine when to stop computing FIRST.
                let mut beta_nullable = true;
                for sy in beta {
                    if !beta_nullable {
                        break;
                    }

                    match sy {
                        // For nonterminal n, add FIRST(n) to the total set.
                        Symbol::Nonterminal(ref n) => {
                            let (to_add, nullable) = first_sets.get(n).unwrap();
                            first_set
                                .extend(to_add.into_iter().map(|&t| LR1Lookahead::Terminal(t)));

                            // No ε, so break from loop to stop adding to FIRST set.
                            // Also do not add the lookahead to FIRST.
                            if !nullable {
                                beta_nullable = false;
                            }
                        }
                        // For terminal t, add t to the FIRST set.
                        // Stop looping and do not add the lookahead to FIRST.
                        Symbol::Terminal(ref t) => {
                            first_set.insert(LR1Lookahead::Terminal(t));
                            beta_nullable = false;
                        }
                    }
                }

                // Only add lookahead a to first if β was nullable.
                if beta_nullable {
                    match lookahead {
                        // If lookahead is not $, add it.
                        LR1Lookahead::Terminal(t) => {
                            first_set.insert(LR1Lookahead::Terminal(t));
                        }
                        // Otherwise, set endmarker flag to true.
                        LR1Lookahead::Endmarker => {
                            first_set.insert(LR1Lookahead::Endmarker);
                        }
                        LR1Lookahead::NonSymbol => {
                            first_set.insert(LR1Lookahead::NonSymbol);
                        }
                    }
                }

                for t in first_set {
                    added.insert((
                        Item {
                            lhs: next_symbol,
                            rhs,
                            pos: 0,
                        },
                        t,
                    ));
                }
            }
        }

        if !added.is_empty() {
            items.extend(added);
            self.lr1_closure(items, first_sets);
        }
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
        let mut initial_set = BTreeSet::new();
        initial_set.insert((
            Item {
                lhs: &E,
                rhs: &start_rhs,
                pos: 0,
            },
            LR1Lookahead::NonSymbol,
        ));

        grammar.lr1_closure(&mut initial_set, &grammar.first_sets());
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
