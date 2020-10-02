#![feature(iterator_fold_self)]
#![feature(proc_macro_diagnostic)]

use std::collections::HashMap;

use automata::{
    dfa::{DFAFromNFA, Transition},
    DFA, NFA,
};
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use regexp2::{
    class::{CharClass, CharRange},
    parser::{NFAParser, Parser},
};
use syn::{
    parenthesized,
    parse::{Parse, ParseStream},
    parse_macro_input, token, Expr, Ident, LitStr, Token, Type, Visibility,
};

#[proc_macro]
pub fn lexer(tok: TokenStream) -> TokenStream {
    let Lexer {
        vis,
        name,
        span_id,
        return_type,
        error_type,
        no_match_error,
        rules,
    } = parse_macro_input!(tok as Lexer);
    let (nfa, action_mapping) = parse_combined_nfa(&rules);
    let DFAFromNFA { dfa, nfa_mapping }: DFAFromNFA<_> = nfa.into();

    let dfa_rebuilt = dfa_rebuilt(&dfa);

    let dfa_actions: Vec<_> = nfa_mapping
        .iter()
        .filter_map(|(dfa_state, nfa_states)| {
            action_mapping
                .iter()
                .filter(|(nfa_state, _)| nfa_states.contains(nfa_state))
                .min_by_key(|(_, (_, precedence))| precedence)
                .map(|(_, (action, _))| (dfa_state, action))
        })
        .collect();

    let action_fns: Vec<_> = dfa_actions.iter().map(|(dfa_state, action)| {
        let fn_name = format_ident!("action_{}", dfa_state);
        quote! {
            fn #fn_name(#span_id: std::string::String) -> Result<Option<#return_type>, #error_type> {
                #action
            }
        }
    }).collect();

    let action_match: Vec<_> = dfa_actions
        .iter()
        .map(|(dfa_state, _)| {
            let fn_call = format_ident!("action_{}", dfa_state);
            quote!(#dfa_state => #fn_call(#span_id))
        })
        .collect();

    (quote! {
        #vis struct #name {
            dfa: automata::DFA<regexp2::class::CharClass>,
        }

        impl #name {
            pub fn new() -> Self {
                Self {
                    dfa: #dfa_rebuilt,
                }
            }

            pub fn advance(&self, input: &str) -> (std::result::Result<Option<#return_type>, #error_type>, std::string::String) {
                #(
                    #action_fns
                )*

                // Step through DFA to the find the longest match.
                let (m, final_state) = match self.dfa.find(&input.chars()) {
                    std::option::Option::Some(m) => m,
                    std::option::Option::None => return (std::result::Result::Err(#no_match_error), std::string::String::from(input))
                };

                // Execute the action expression corresponding to the final state.
                let #span_id: std::string::String = input.chars().take(m.end()).collect();
                let token_res = match final_state {
                    #( #action_match ),*,
                    // Catch-all branch should never execute?
                    _ => std::panic!(),
                };

                let remaining = input.chars().skip(m.end()).collect();
                match token_res {
                    std::result::Result::Ok(token_op) => match token_op {
                        // If a token was returned, return the token and the remaining input.
                        std::option::Option::Some(t) => {
                            (std::result::Result::Ok(Some(t)), remaining)
                        }
                        // If no token was returned, one input symbol should be consumed and the process
                        // restarted.
                        std::option::Option::None => {
                            let remaining: std::string::String = input.chars().skip(1).collect();
                            if remaining.len() == 0 {
                                (std::result::Result::Ok(None), std::string::String::new())
                            } else {
                                self.advance(&remaining)
                            }
                        }
                    }
                    // If action expression returned an error, return the error and remaining
                    // input?
                    std::result::Result::Err(err) => (std::result::Result::Err(err), remaining),
                }
            }
        }
    })
    .into()
}

struct Lexer {
    vis: Visibility,
    name: Ident,

    span_id: Ident,
    return_type: Type,

    error_type: Type,
    no_match_error: Expr,

    rules: Vec<Rule>,
}

impl Parse for Lexer {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let vis = input.parse()?;
        let name = input.parse()?;

        let span_id = {
            let inner;
            parenthesized!(inner in input);
            let span_id = inner.parse()?;
            if !inner.is_empty() {
                return Err(inner.error("unexpected token after token string identifier"));
            }
            span_id
        };

        input.parse::<Token![->]>()?;
        let return_type = input.parse()?;
        input.parse::<Token![,]>()?;

        let error_type = input.parse()?;
        input.parse::<Token![;]>()?;
        let no_match_error = input.parse()?;
        input.parse::<Token![;]>()?;

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

        Ok(Self {
            vis,
            name,
            span_id,
            return_type,
            error_type,
            no_match_error,
            rules,
        })
    }
}

struct Rule {
    regexp: LitStr,
    action: Expr,
}

impl Rule {
    fn new(regexp: LitStr, action: Expr) -> Self {
        Self { regexp, action }
    }
}

const INVALID_REGEXP_ERROR: &str = "invalid regular expression";

// Parse the rules into a single NFA and a map of final states to action expressions.
fn parse_combined_nfa(rules: &[Rule]) -> (NFA<CharClass>, HashMap<usize, (&Expr, usize)>) {
    let nfa_parser = NFAParser::new();
    // Parse regular expression strings into NFAs.
    let nfa_sub: Vec<_> = rules
        .iter()
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

fn dfa_rebuilt(dfa: &DFA<CharClass>) -> TokenStream2 {
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
        {
            let mut dfa = automata::DFA::new();
            dfa.initial_state = #initial_state;
            dfa.total_states = #total_states;
            dfa.final_states = std::collections::HashSet::new();
            dfa.final_states.extend(&[ #( #final_states ),* ]);

            dfa.transition = automata::table::Table::new();
            #( #transition_sets )*

            dfa
        }
    }
}