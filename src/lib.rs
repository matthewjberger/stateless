//! A zero-cost state machine macro that separates structure from behavior.
//!
//! Most state machine libraries couple behavior to the state machine itself — guards, actions,
//! and context structs all get tangled into the DSL. `stateless` takes the opposite approach:
//! the macro is a pure transition table. It generates two enums and a lookup function. Guards,
//! side effects, and error handling live in your own code, using normal Rust patterns.
//!
//! # Quick Start
//!
//! ```
//! use stateless::statemachine;
//!
//! statemachine! {
//!     transitions: {
//!         *Idle + Start = Running,
//!         Running + Stop = Idle,
//!         _ + Reset = Idle,
//!     }
//! }
//!
//! let mut state = State::default(); // Idle (marked with *)
//! assert_eq!(state, State::Idle);
//!
//! if let Some(new_state) = state.process_event(Event::Start) {
//!     state = new_state;
//! }
//! assert_eq!(state, State::Running);
//! ```
//!
//! See [`statemachine!`] for the full DSL reference and all features.

use std::collections::{BTreeMap, BTreeSet};

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

/// Generates a state machine from a declarative transition table.
///
/// The macro produces two enums (`State` and `Event`) and a `process_event` method
/// that returns `Some(new_state)` for valid transitions and `None` otherwise.
/// Guards, actions, and side effects live in your code — the macro is purely structural.
///
/// # Basic Usage
///
/// ```
/// use stateless::statemachine;
///
/// statemachine! {
///     transitions: {
///         *Idle + Start = Running,
///         Running + Stop = Idle,
///     }
/// }
///
/// let mut state = State::default(); // Idle (marked with *)
///
/// if let Some(new_state) = state.process_event(Event::Start) {
///     // insert guards and actions here
///     state = new_state;
/// }
/// assert_eq!(state, State::Running);
/// ```
///
/// # Generated API
///
/// For a state machine with states `Idle` and `Running` and events `Start` and `Stop`:
///
/// ```
/// # use stateless::statemachine;
/// # statemachine! {
/// #     transitions: {
/// #         *Idle + Start = Running,
/// #         Running + Stop = Idle,
/// #     }
/// # }
/// // Enums derive Debug, Copy, Clone, PartialEq, Eq, Hash by default
/// let s: State = State::Idle;
/// let e: Event = Event::Start;
///
/// // process_event: check if a transition is valid
/// assert_eq!(s.process_event(e), Some(State::Running));
/// assert_eq!(s.process_event(Event::Stop), None); // invalid from Idle
///
/// // ALL: every variant as a static slice
/// assert_eq!(State::ALL, &[State::Idle, State::Running]);
/// assert_eq!(Event::ALL, &[Event::Start, Event::Stop]);
///
/// // valid_events: which events produce transitions from this state
/// assert_eq!(State::Idle.valid_events(), &[Event::Start]);
/// assert_eq!(State::Running.valid_events(), &[Event::Stop]);
///
/// // DOT: Graphviz representation of the transition table
/// assert!(State::DOT.contains("\"Idle\" -> \"Running\""));
/// ```
///
/// # Guards and Actions
///
/// Call `process_event` to check validity, verify guards, perform side effects,
/// then apply the state:
///
/// ```
/// # use stateless::statemachine;
/// # statemachine! {
/// #     transitions: {
/// #         *Idle + Connect = Connected,
/// #         Connected + Disconnect = Idle,
/// #     }
/// # }
/// struct Server {
///     state: State,
///     battery: u32,
///     connection_id: u32,
/// }
///
/// impl Server {
///     fn connect(&mut self, id: u32) {
///         let Some(new_state) = self.state.process_event(Event::Connect) else {
///             return; // not valid from current state
///         };
///
///         // guards
///         if self.battery < 5 {
///             return;
///         }
///
///         // actions
///         self.connection_id = id;
///         self.battery -= 5;
///
///         // apply
///         self.state = new_state;
///     }
/// }
/// ```
///
/// # Initial State
///
/// Mark the initial state with `*`. This state is used for the `Default` implementation.
/// If no state is marked, the first state in declaration order is used.
///
/// ```
/// # use stateless::statemachine;
/// statemachine! {
///     transitions: {
///         *Idle + Start = Running,
///         Running + Stop = Idle,
///     }
/// }
/// assert_eq!(State::default(), State::Idle);
/// ```
///
/// # State Patterns
///
/// Multiple source states can share a transition:
///
/// ```
/// # use stateless::statemachine;
/// statemachine! {
///     transitions: {
///         *Ready | Waiting + Start = Active,
///         Active + Stop = Ready,
///     }
/// }
/// assert_eq!(State::Ready.process_event(Event::Start), Some(State::Active));
/// assert_eq!(State::Waiting.process_event(Event::Start), Some(State::Active));
/// ```
///
/// # Event Patterns
///
/// Multiple events can trigger the same transition:
///
/// ```
/// # use stateless::statemachine;
/// statemachine! {
///     transitions: {
///         *Active + Pause | Stop = Idle,
///         Idle + Start = Active,
///     }
/// }
/// assert_eq!(State::Active.process_event(Event::Pause), Some(State::Idle));
/// assert_eq!(State::Active.process_event(Event::Stop), Some(State::Idle));
/// ```
///
/// # Wildcard Transitions
///
/// Transition from any state. Specific transitions always take priority over wildcards:
///
/// ```
/// # use stateless::statemachine;
/// statemachine! {
///     transitions: {
///         *A + Go = B,
///         B + Go = C,
///         _ + Go = A, // only applies to states without a specific Go transition
///     }
/// }
/// assert_eq!(State::A.process_event(Event::Go), Some(State::B)); // specific
/// assert_eq!(State::B.process_event(Event::Go), Some(State::C)); // specific
/// assert_eq!(State::C.process_event(Event::Go), Some(State::A)); // wildcard
/// ```
///
/// # Internal Transitions
///
/// Stay in the current state (useful for side effects without state change):
///
/// ```
/// # use stateless::statemachine;
/// statemachine! {
///     transitions: {
///         *Moving + Tick = _,
///         Moving + Arrive = Idle,
///     }
/// }
/// assert_eq!(State::Moving.process_event(Event::Tick), Some(State::Moving));
/// ```
///
/// # Custom Derives
///
/// Default derives are `Debug, Copy, Clone, PartialEq, Eq, Hash`.
/// Override per enum with `derive_states` and `derive_events`:
///
/// ```
/// # use stateless::statemachine;
/// statemachine! {
///     derive_states: [Debug, Clone, PartialEq, Eq, Hash],
///     derive_events: [Debug, Clone, PartialEq],
///     transitions: {
///         *Idle + Start = Running,
///         Running + Stop = Idle,
///     }
/// }
/// ```
///
/// # Multiple State Machines
///
/// Use `name` for namespacing when you need multiple state machines in the same scope.
/// Generates `{Name}State` and `{Name}Event`:
///
/// ```
/// # use stateless::statemachine;
/// statemachine! {
///     name: Player,
///     transitions: {
///         *Idle + Move = Walking,
///         Walking + Stop = Idle,
///     }
/// }
///
/// statemachine! {
///     name: Enemy,
///     transitions: {
///         *Patrol + Spot = Chasing,
///         Chasing + Lose = Patrol,
///     }
/// }
///
/// let p = PlayerState::default();
/// let e = EnemyState::default();
/// ```
///
/// # Variant Enumeration
///
/// `State::ALL` and `Event::ALL` list every variant as static slices:
///
/// ```
/// # use stateless::statemachine;
/// statemachine! {
///     transitions: {
///         *Idle + Start = Running,
///         Running + Stop = Idle,
///     }
/// }
///
/// for state in State::ALL {
///     println!("{:?} accepts {:?}", state, state.valid_events());
/// }
/// ```
///
/// # Terminal State Detection
///
/// States with no outgoing transitions return an empty slice from `valid_events`:
///
/// ```
/// # use stateless::statemachine;
/// statemachine! {
///     transitions: {
///         *Start + Go = End,
///     }
/// }
/// assert!(State::End.valid_events().is_empty());
/// ```
///
/// # DOT Graph Output
///
/// `State::DOT` contains a [Graphviz DOT](https://graphviz.org/) representation
/// of the transition table. It's a `const` — zero binary footprint if unused.
///
/// ```
/// # use stateless::statemachine;
/// statemachine! {
///     transitions: {
///         *Idle + Start = Running,
///         Running + Stop = Idle,
///     }
/// }
/// // pipe to: dot -Tpng -o states.png
/// println!("{}", State::DOT);
/// ```
///
/// # Compile-Time Validation
///
/// The macro rejects invalid definitions at compile time:
///
/// - Duplicate transitions (same state + event pair)
/// - Multiple initial states (more than one `*`)
/// - Empty transition blocks
/// - Duplicate wildcard events
///
/// # DSL Reference
///
/// ```text
/// statemachine! {
///     name: MyMachine,                          // Optional: generates MyMachineState, MyMachineEvent
///     derive_states: [Debug, Clone, PartialEq], // Optional: custom derives for State
///     derive_events: [Debug, Clone, PartialEq], // Optional: custom derives for Event
///
///     transitions: {
///         *Idle + Start = Running,              // Initial state marked with *
///         Ready | Waiting + Start = Active,     // State patterns (multiple source states)
///         Active + Stop | Pause = Idle,         // Event patterns (multiple trigger events)
///         _ + Reset = Idle,                     // Wildcard (from any state, lowest priority)
///         Active + Tick = _,                    // Internal transition (stay in same state)
///     }
/// }
/// ```
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

    let mut state_valid_events: BTreeMap<String, Vec<Ident>> = BTreeMap::new();
    let mut wildcard_event_idents: Vec<Ident> = Vec::new();
    let mut specific_pairs: BTreeSet<(String, String)> = BTreeSet::new();
    let mut dot_string = String::from("digraph {\n  rankdir=LR;\n  node [shape=circle];\n");
    dot_string.push_str(&format!(
        "  \"{}\" [shape=doublecircle];\n",
        initial_state
    ));
    let mut wildcard_transitions: Vec<&Transition> = Vec::new();

    for transition in &state_machine.transitions {
        match &transition.states {
            StatePattern::Named(states) => {
                for (state_ident, _) in states {
                    let entry = state_valid_events
                        .entry(state_ident.to_string())
                        .or_default();
                    for event in &transition.events {
                        if !entry.contains(event) {
                            entry.push(event.clone());
                        }
                        specific_pairs
                            .insert((state_ident.to_string(), event.to_string()));
                        let target = match &transition.target {
                            TargetState::State(t) => t.to_string(),
                            TargetState::Internal => state_ident.to_string(),
                        };
                        dot_string.push_str(&format!(
                            "  \"{}\" -> \"{}\" [label=\"{}\"];\n",
                            state_ident, target, event
                        ));
                    }
                }
            }
            StatePattern::Wildcard => {
                for event in &transition.events {
                    if !wildcard_event_idents.contains(event) {
                        wildcard_event_idents.push(event.clone());
                    }
                }
                wildcard_transitions.push(transition);
            }
        }
    }

    for transition in &wildcard_transitions {
        for state in &all_states {
            for event in &transition.events {
                if !specific_pairs.contains(&(state.to_string(), event.to_string())) {
                    let target = match &transition.target {
                        TargetState::State(t) => t.to_string(),
                        TargetState::Internal => state.to_string(),
                    };
                    dot_string.push_str(&format!(
                        "  \"{}\" -> \"{}\" [label=\"{}\"];\n",
                        state, target, event
                    ));
                }
            }
        }
    }
    dot_string.push('}');

    for state in &all_states {
        let entry = state_valid_events.entry(state.to_string()).or_default();
        for event in &wildcard_event_idents {
            if !entry.contains(event) {
                entry.push(event.clone());
            }
        }
    }

    let valid_events_arms: Vec<TokenStream2> = all_states
        .iter()
        .map(|state| {
            let events = state_valid_events
                .get(&state.to_string())
                .map(|v| v.as_slice())
                .unwrap_or(&[]);
            quote! {
                #state_name::#state => &[#(#event_name::#events),*]
            }
        })
        .collect();

    let default_derives = vec![
        Ident::new("Debug", Span::call_site()),
        Ident::new("Copy", Span::call_site()),
        Ident::new("Clone", Span::call_site()),
        Ident::new("PartialEq", Span::call_site()),
        Ident::new("Eq", Span::call_site()),
        Ident::new("Hash", Span::call_site()),
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
            quote! { (#(matches!(*self, #state_patterns))||*) }
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
            pub const ALL: &[#state_name] = &[#(#state_name::#all_states),*];

            pub const DOT: &str = #dot_string;

            pub fn process_event(&self, event: #event_name) -> ::core::option::Option<#state_name> {
                #transition_checks
                ::core::option::Option::None
            }

            pub fn valid_events(&self) -> &'static [#event_name] {
                match self {
                    #(#valid_events_arms),*
                }
            }
        }

        impl #event_name {
            pub const ALL: &[#event_name] = &[#(#event_name::#all_events),*];
        }
    };

    TokenStream::from(expanded)
}
