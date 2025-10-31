#![no_std]

extern crate alloc;

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    token::Comma,
    Error, Ident, Result, Token,
};

struct StateMachine {
    name: Option<Ident>,
    derive_states: Option<Vec<Ident>>,
    derive_events: Option<Vec<Ident>>,
    transitions: Vec<Transition>,
}

struct Transition {
    states: StatePattern,
    events: Vec<Ident>,
    target: TargetState,
}

enum StatePattern {
    Single { ident: Ident, initial: bool },
    Multiple { states: Vec<(Ident, bool)> },
    Wildcard,
}

enum TargetState {
    State(Ident),
    Internal,
}

impl Parse for StateMachine {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut name = None;
        let mut derive_states = None;
        let mut derive_events = None;

        while !input.peek(syn::Ident) || input.peek2(Token![:]) {
            let lookahead = input.lookahead1();
            if lookahead.peek(syn::Ident) {
                let ident: Ident = input.parse()?;
                input.parse::<Token![:]>()?;

                if ident == "name" {
                    name = Some(input.parse::<Ident>()?);
                    if input.peek(Token![,]) {
                        input.parse::<Token![,]>()?;
                    }
                } else if ident == "derive_states" {
                    let content;
                    syn::bracketed!(content in input);
                    let derives = Punctuated::<Ident, Comma>::parse_terminated(&content)?;
                    derive_states = Some(derives.into_iter().collect());
                    if input.peek(Token![,]) {
                        input.parse::<Token![,]>()?;
                    }
                } else if ident == "derive_events" {
                    let content;
                    syn::bracketed!(content in input);
                    let derives = Punctuated::<Ident, Comma>::parse_terminated(&content)?;
                    derive_events = Some(derives.into_iter().collect());
                    if input.peek(Token![,]) {
                        input.parse::<Token![,]>()?;
                    }
                } else if ident == "transitions" {
                    let transitions_content;
                    syn::braced!(transitions_content in input);
                    let transition_list =
                        Punctuated::<Transition, Comma>::parse_terminated(&transitions_content)?;
                    let transitions = transition_list.into_iter().collect();
                    return Ok(StateMachine {
                        name,
                        derive_states,
                        derive_events,
                        transitions,
                    });
                } else {
                    return Err(Error::new(
                        ident.span(),
                        "Expected 'name', 'derive_states', 'derive_events', or 'transitions'",
                    ));
                }
            } else {
                return Err(lookahead.error());
            }
        }

        Err(Error::new(input.span(), "Expected 'transitions' block"))
    }
}

impl Parse for Transition {
    fn parse(input: ParseStream) -> Result<Self> {
        let states = input.parse::<StatePattern>()?;
        input.parse::<Token![+]>()?;

        let mut events = Vec::new();
        events.push(input.parse::<Ident>()?);

        while input.peek(Token![|]) && !input.peek2(Token![*]) {
            input.parse::<Token![|]>()?;
            events.push(input.parse::<Ident>()?);
        }

        let target = if input.peek(Token![=]) {
            input.parse::<Token![=]>()?;
            if input.peek(Token![_]) {
                input.parse::<Token![_]>()?;
                TargetState::Internal
            } else {
                TargetState::State(input.parse::<Ident>()?)
            }
        } else {
            TargetState::Internal
        };

        Ok(Transition {
            states,
            events,
            target,
        })
    }
}

impl Parse for StatePattern {
    fn parse(input: ParseStream) -> Result<Self> {
        if input.peek(Token![_]) {
            input.parse::<Token![_]>()?;
            return Ok(StatePattern::Wildcard);
        }

        let mut states = Vec::new();
        let initial = input.peek(Token![*]);
        if initial {
            input.parse::<Token![*]>()?;
        }

        let first_ident = input.parse::<Ident>()?;
        states.push((first_ident.clone(), initial));

        while input.peek(Token![|]) {
            input.parse::<Token![|]>()?;
            let next_initial = input.peek(Token![*]);
            if next_initial {
                input.parse::<Token![*]>()?;
            }
            states.push((input.parse::<Ident>()?, next_initial));
        }

        if states.len() == 1 {
            Ok(StatePattern::Single {
                ident: states[0].0.clone(),
                initial,
            })
        } else {
            Ok(StatePattern::Multiple { states })
        }
    }
}

fn validate_no_duplicate_transitions(transitions: &[Transition]) -> Result<()> {
    let mut seen = BTreeSet::new();

    for transition in transitions {
        let state_idents: Vec<String> = match &transition.states {
            StatePattern::Single { ident, .. } => {
                alloc::vec![ident.to_string()]
            }
            StatePattern::Multiple { states } => {
                states.iter().map(|(ident, _)| ident.to_string()).collect()
            }
            StatePattern::Wildcard => continue,
        };

        for state_str in state_idents {
            for event in &transition.events {
                let key = (state_str.clone(), event.to_string());

                if !seen.insert(key.clone()) {
                    return Err(Error::new(
                        event.span(),
                        format!(
                            "duplicate transition: state '{}' + event '{}' is already defined\n\
                             help: each combination of source state and event can only appear once\n\
                             note: if you need conditional behavior, use different events or handle logic in your wrapper",
                            key.0, key.1
                        ),
                    ));
                }
            }
        }
    }

    Ok(())
}

#[proc_macro]
pub fn statemachine(input: TokenStream) -> TokenStream {
    let state_machine = parse_macro_input!(input as StateMachine);

    if let Err(e) = validate_no_duplicate_transitions(&state_machine.transitions) {
        return e.to_compile_error().into();
    }

    let state_name = if let Some(ref name) = state_machine.name {
        Ident::new(&format!("{}State", name), name.span())
    } else {
        Ident::new("State", Span::call_site())
    };

    let event_name = if let Some(ref name) = state_machine.name {
        Ident::new(&format!("{}Event", name), name.span())
    } else {
        Ident::new("Event", Span::call_site())
    };

    let mut all_states = alloc::vec::Vec::new();
    let mut all_events = alloc::vec::Vec::new();
    let mut initial_state = None;

    for transition in &state_machine.transitions {
        match &transition.states {
            StatePattern::Single { ident, initial } => {
                if !all_states.iter().any(|s| s == ident) {
                    all_states.push(ident.clone());
                }
                if *initial && initial_state.is_none() {
                    initial_state = Some(ident.clone());
                }
            }
            StatePattern::Multiple { states } => {
                for (ident, initial) in states {
                    if !all_states.iter().any(|s| s == ident) {
                        all_states.push(ident.clone());
                    }
                    if *initial && initial_state.is_none() {
                        initial_state = Some(ident.clone());
                    }
                }
            }
            StatePattern::Wildcard => {}
        }

        if let TargetState::State(ref target) = transition.target {
            if !all_states.iter().any(|s| s == target) {
                all_states.push(target.clone());
            }
        }

        for event in &transition.events {
            if !all_events.iter().any(|e| e == event) {
                all_events.push(event.clone());
            }
        }
    }

    let initial_state = initial_state.unwrap_or_else(|| {
        Ident::new(
            &all_states
                .first()
                .map(|s| s.to_string())
                .unwrap_or_else(|| "Initial".to_string()),
            Span::call_site(),
        )
    });

    let default_derives = vec![
        Ident::new("Debug", Span::call_site()),
        Ident::new("Clone", Span::call_site()),
        Ident::new("PartialEq", Span::call_site()),
        Ident::new("Eq", Span::call_site()),
    ];

    let state_derives = state_machine
        .derive_states
        .as_ref()
        .unwrap_or(&default_derives);

    let event_derives = state_machine
        .derive_events
        .as_ref()
        .unwrap_or(&default_derives);

    let state_enum = quote! {
        #[derive(#(#state_derives),*)]
        pub enum #state_name {
            #(#all_states),*
        }
    };

    let event_enum_variants = all_events.iter().map(|event| {
        quote! { #event }
    });

    let event_enum = quote! {
        #[derive(#(#event_derives),*)]
        pub enum #event_name {
            #(#event_enum_variants),*
        }
    };

    let mut transition_checks = TokenStream2::new();

    for transition in &state_machine.transitions {
        let events = &transition.events;

        let target_state = match &transition.target {
            TargetState::State(state) => quote! { #state_name::#state },
            TargetState::Internal => quote! { self.clone() },
        };

        let state_patterns: Vec<_> = match &transition.states {
            StatePattern::Single { ident, .. } => {
                alloc::vec![quote! { #state_name::#ident }]
            }
            StatePattern::Multiple { states } => states
                .iter()
                .map(|(ident, _)| quote! { #state_name::#ident })
                .collect(),
            StatePattern::Wildcard => {
                alloc::vec![quote! { _ }]
            }
        };

        let state_condition = if state_patterns.len() == 1 && state_patterns[0].to_string() == "_" {
            quote! { true }
        } else if state_patterns.len() == 1 {
            let pattern = &state_patterns[0];
            quote! { matches!(*self, #pattern) }
        } else {
            quote! { #(matches!(*self, #state_patterns))||* }
        };

        for event in events {
            let event_condition = quote! { matches!(event, #event_name::#event) };

            transition_checks.extend(quote! {
                if #state_condition && #event_condition {
                    return ::core::option::Option::Some(#target_state);
                }
            });
        }
    }

    let expanded = quote! {
        #state_enum
        #event_enum

        impl ::core::default::Default for #state_name {
            fn default() -> Self {
                #state_name::#initial_state
            }
        }

        impl #state_name {
            pub fn process_event(&self, event: #event_name) -> ::core::option::Option<#state_name> {
                #transition_checks
                ::core::option::Option::None
            }
        }
    };

    TokenStream::from(expanded)
}
