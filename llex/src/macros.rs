use std::collections::HashMap;

use automata::{
    dfa::{DFAFromNFA, Transition},
    DFA, NFA,
};
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use regexp2::{
    class::{CharClass, CharRange},
    parser::{NFAParser, Parser},
};
use syn::{
    parenthesized,
    parse::{Parse, ParseStream},
    parse_macro_input, token, Expr, Ident, LitStr, Token, Type, Visibility,
};

pub fn lexer(tok: TokenStream) -> TokenStream {
    let Lexer {
        vis,
        name,
        str_ident,
        return_type,
        rules,
    } = parse_macro_input!(tok as Lexer);
    let (nfa, action_mapping) = parse_combined_nfa(&rules);
    let DFAFromNFA { dfa, nfa_mapping }: DFAFromNFA<_> = nfa.into();

    let dfa_static = lazy_static_dfa(&dfa);

    let action_match: Vec<_> = nfa_mapping
        .iter()
        .filter_map(|(dfa_state, nfa_states)| {
            action_mapping
                .iter()
                .filter(|(nfa_state, _)| nfa_states.contains(nfa_state))
                .min_by_key(|(_, (_, precedence))| precedence)
                .and_then(|(_, (action, _))| Some(quote!(#dfa_state => #action)))
        })
        .collect();

    (quote! {
        #dfa_static

        #vis fn #name(input: &str) -> std::option::Option<(#return_type, std::string::String)> {
            let (m, final_state) = match LEXER_DFA.find(&input.chars()) {
                std::option::Option::Some(m) => m,
                std::option::Option::None => return std::option::Option::None,
            };

            // No match, should initiate error handling
            if m.end() == m.start() {
                return std::option::Option::None;
            }

            let #str_ident: std::string::String = input.chars().take(m.end()).collect();
            let token = match final_state {
                #( #action_match ),*,
                _ => std::option::Option::None,
            };

            match token {
                std::option::Option::Some(t) => {
                    let remaining = input.chars().skip(m.end()).collect();
                    std::option::Option::Some((t, remaining))
                }
                std::option::Option::None => {
                    let remaining: std::string::String = input.chars().skip(1).collect();
                    #name(&remaining)
                }
            }
        }

    })
    .into()
}

pub struct Lexer {
    pub vis: Visibility,
    pub name: Ident,

    pub str_ident: Ident,
    pub return_type: Type,

    pub rules: Vec<Rule>,
}

impl Parse for Lexer {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let vis = input.parse()?;
        let name = {
            input.parse::<Token![fn]>()?;
            input.parse()?
        };

        let str_ident = {
            let inner;
            parenthesized!(inner in input);
            let str_ident = inner.parse()?;
            if !inner.is_empty() {
                return Err(inner.error("unexpected token after token string identifier"));
            }
            str_ident
        };

        let return_type = {
            input.parse::<Token![->]>()?;
            let ty = input.parse()?;
            input.parse::<Token![;]>()?;
            ty
        };

        let rules = {
            let mut rules = Vec::new();
            while !input.is_empty() {
                let regexp = input.parse()?;
                input.parse::<Token![=>]>()?;

                let optional_comma = input.peek(token::Brace);

                let action = input.parse()?;
                let rule = Rule::new(regexp, action);

                match input.parse::<Token![,]>() {
                    Ok(_) => {}
                    Err(e) => {
                        if !input.is_empty() && !optional_comma {
                            return Err(e);
                        }
                    }
                }

                rules.push(rule);
            }
            rules
        };

        Ok(Lexer {
            vis,
            name,
            str_ident,
            return_type,
            rules,
        })
    }
}

pub struct Rule {
    pub regexp: LitStr,
    pub action: Expr,
}

impl Rule {
    fn new(regexp: LitStr, action: Expr) -> Self {
        Self { regexp, action }
    }
}

const INVALID_REGEXP_ERROR: &str = "invalid regular expression";

// Parse the rules into a single NFA and a map of final states to action expressions.
fn parse_combined_nfa(rules: &Vec<Rule>) -> (NFA<CharClass>, HashMap<usize, (&Expr, usize)>) {
    let nfa_parser = NFAParser::new();
    // Parse regular expression strings into NFAs.
    let nfa_sub: Vec<_> = rules
        .into_iter()
        .filter_map(
            |Rule { regexp, action }| match nfa_parser.parse(&regexp.value()) {
                // Throw errors if failed to parse.
                Ok(op) => match op {
                    Some(n) => Some((n, action)),
                    // None returned means error.
                    None => {
                        regexp.span().unstable().error(INVALID_REGEXP_ERROR).emit();
                        None
                    }
                },
                Err(e) => {
                    regexp
                        .span()
                        .unstable()
                        .error(format!("{}: {}", INVALID_REGEXP_ERROR, e))
                        .emit();
                    None
                }
            },
        )
        .collect();

    // Combine NFAs into a single NFA.
    let mut action_mapping = HashMap::new();
    let mut nfa = NFA::new();
    let mut offset = nfa.total_states;
    for (precedence, (sub, action)) in nfa_sub.iter().enumerate() {
        NFA::copy_into(&mut nfa, sub);
        nfa.add_epsilon_transition(nfa.initial_state, sub.initial_state + offset);
        // Map new, offsetted final states to their original action.
        for sub_final in sub.final_states.iter() {
            nfa.final_states.insert(*sub_final + offset);
            action_mapping.insert(*sub_final + offset, (*action, precedence));
        }

        offset += sub.total_states;
    }

    (nfa, action_mapping)
}

fn lazy_static_dfa(dfa: &DFA<CharClass>) -> TokenStream2 {
    let initial_state = dfa.initial_state;
    let total_states = dfa.total_states;
    let final_states: Vec<_> = dfa.final_states.iter().collect();
    let transition_sets: Vec<_> = dfa
        .transition
        .clone()
        .into_iter()
        .map(|(src, Transition(tr), dest)| {
            let ranges: Vec<_> = tr
                .ranges
                .iter()
                .map(|CharRange { start, end }| quote!(regexp2::class::CharRange::new(#start, #end)))
                .collect();
            quote! { dfa.transition.set(#src, automata::dfa::Transition(vec![ #( #ranges ),* ].into()), #dest); }
        })
        .collect();

    quote! {
        lazy_static::lazy_static! {
            static ref LEXER_DFA: automata::DFA<regexp2::class::CharClass> = {
                let mut dfa = automata::DFA::new();
                dfa.initial_state = #initial_state;
                dfa.total_states = #total_states;
                dfa.final_states = std::collections::HashSet::new();
                dfa.final_states.extend(&[ #( #final_states ),* ]);

                dfa.transition = automata::table::Table::new();
                #( #transition_sets )*

                dfa
            };
        }
    }
}