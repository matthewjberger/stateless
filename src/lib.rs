use std::collections::BTreeSet;

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{
    Error, Ident, Result, Token,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    token::Comma,
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
    Named(Vec<(Ident, bool)>),
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

        Ok(StatePattern::Named(states))
    }
}

fn validate_no_duplicate_transitions(transitions: &[Transition]) -> Result<()> {
    let mut seen = BTreeSet::new();
    let mut wildcard_seen = BTreeSet::new();

    for transition in transitions {
        let state_idents: Vec<String> = match &transition.states {
            StatePattern::Named(states) => {
                states.iter().map(|(ident, _)| ident.to_string()).collect()
            }
            StatePattern::Wildcard => {
                for event in &transition.events {
                    let event_str = event.to_string();
                    if !wildcard_seen.insert(event_str.clone()) {
                        return Err(Error::new(
                            event.span(),
                            format!(
                                "duplicate wildcard transition: '_ + {}' is already defined",
                                event_str
                            ),
                        ));
                    }
                }
                continue;
            }
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

    if state_machine.transitions.is_empty() {
        return Error::new(
            Span::call_site(),
            "state machine must have at least one transition",
        )
        .to_compile_error()
        .into();
    }

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

    let mut all_states = Vec::new();
    let mut all_events = Vec::new();
    let mut seen_states = BTreeSet::new();
    let mut seen_events = BTreeSet::new();
    let mut initial_state = None;

    for transition in &state_machine.transitions {
        if let StatePattern::Named(states) = &transition.states {
            for (ident, initial) in states {
                if seen_states.insert(ident.to_string()) {
                    all_states.push(ident.clone());
                }
                if *initial {
                    if initial_state.is_some() {
                        return Error::new(
                            ident.span(),
                            "multiple initial states: only one state can be marked with '*'",
                        )
                        .to_compile_error()
                        .into();
                    }
                    initial_state = Some(ident.clone());
                }
            }
        }

        if let TargetState::State(ref target) = transition.target
            && seen_states.insert(target.to_string())
        {
            all_states.push(target.clone());
        }

        for event in &transition.events {
            if seen_events.insert(event.to_string()) {
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
        Ident::new("Copy", Span::call_site()),
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
    let mut wildcard_checks = TokenStream2::new();

    for transition in &state_machine.transitions {
        let events = &transition.events;

        let target_state = match &transition.target {
            TargetState::State(state) => quote! { #state_name::#state },
            TargetState::Internal => quote! {
                match *self {
                    #(#state_name::#all_states => #state_name::#all_states),*
                }
            },
        };

        let is_wildcard = matches!(&transition.states, StatePattern::Wildcard);

        let state_patterns: Vec<_> = match &transition.states {
            StatePattern::Named(states) => states
                .iter()
                .map(|(ident, _)| quote! { #state_name::#ident })
                .collect(),
            StatePattern::Wildcard => {
                vec![quote! { _ }]
            }
        };

        let state_condition = if is_wildcard {
            quote! { true }
        } else if state_patterns.len() == 1 {
            let pattern = &state_patterns[0];
            quote! { matches!(*self, #pattern) }
        } else {
            quote! { #(matches!(*self, #state_patterns))||* }
        };

        let dest = if is_wildcard {
            &mut wildcard_checks
        } else {
            &mut transition_checks
        };

        for event in events {
            let event_condition = quote! { matches!(event, #event_name::#event) };

            dest.extend(quote! {
                if #state_condition && #event_condition {
                    return ::core::option::Option::Some(#target_state);
                }
            });
        }
    }

    transition_checks.extend(wildcard_checks);

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
