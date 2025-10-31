# stateless

A lightweight, zero-cost state machine library for Rust that separates structure from behavior.

[![Crates.io](https://img.shields.io/crates/v/stateless.svg)](https://crates.io/crates/stateless)
[![Documentation](https://docs.rs/stateless/badge.svg)](https://docs.rs/stateless)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## Philosophy

**State machines should define structure, not behavior.**

This library provides a declarative DSL for defining state transitions. You handle all guards, actions, and business logic in clean, idiomatic Rust wrapper code.

## Why Use This?

- **Zero coupling**: State machine knows nothing about your types
- **Idiomatic Rust**: Use `Result`, methods, and proper error handling
- **Zero cost**: Compiles to efficient sequential checks
- **Type safe**: Leverages Rust's type system fully
- **No dependencies**: `no_std` compatible
- **Clear code**: Business logic lives in one place, not scattered

## Installation

```toml
[dependencies]
stateless = "0.1.0"
```

## Quick Start

```rust
use stateless::statemachine;

// Define the state machine structure
statemachine! {
    transitions: {
        *Idle + Start = Running,
        Running + Stop = Idle,
    }
}

// Wrap it with your business logic
struct MyMachine {
    state: State,
    battery: u32,
}

impl MyMachine {
    fn new() -> Self {
        Self {
            state: State::default(),
            battery: 100,
        }
    }

    fn start(&mut self) {
        // Check if transition is valid
        let Some(new_state) = self.state.process_event(Event::Start) else {
            return;
        };

        // Guard: check preconditions
        if self.battery < 20 {
            return;
        }

        // Action: side effects
        self.battery -= 10;

        // Apply transition
        self.state = new_state;
    }

    fn stop(&mut self) {
        if let Some(new_state) = self.state.process_event(Event::Stop) {
            self.state = new_state;
        }
    }
}
```

## Features

### Guards and Actions

Guards and actions live in your wrapper code, not the DSL.

`state.process_event(event)` returns `Option<State>` - the new state if the transition is valid. You check guards, perform actions, then apply the state:

```rust
fn connect(&mut self, id: u32) {
    // Check if transition is valid for current state
    let Some(new_state) = self.state.process_event(Event::Connect) else {
        return;
    };

    // Guard: check preconditions
    if id > self.max_connections {
        return;
    }

    // Guard: check resources
    if self.battery < 5 {
        return;
    }

    // Actions: side effects
    self.connection_id = id;
    self.battery -= 5;

    // Apply transition
    self.state = new_state;
}
```

This approach gives you:
- Full control over when to apply transitions
- Multiple guards with early returns
- Actions only happen if all guards pass
- Zero coupling between state machine structure and business logic
- Clean, idiomatic Rust

### State Patterns

Multiple states can share transitions:

```rust
statemachine! {
    transitions: {
        *Ready | Waiting + Start = Active,
        Active + Stop = Ready,
    }
}
```

### Event Patterns

Multiple events can trigger the same transition:

```rust
statemachine! {
    transitions: {
        *Active + Pause | Stop = Idle,
    }
}
```

### Wildcard Transitions

Transition from any state:

```rust
statemachine! {
    transitions: {
        *Idle + Start = Running,
        _ + Reset = Idle,  // From any state
    }
}
```

### Internal Transitions

Stay in the same state while performing side effects:

```rust
statemachine! {
    transitions: {
        Moving + Tick = _,  // Stays in Moving
    }
}

impl Robot {
    fn tick(&mut self) {
        let Some(new_state) = self.state.process_event(Event::Tick) else {
            return;
        };

        self.movement_ticks += 1;  // Side effect without changing state
        self.state = new_state;
    }
}
```

Internal transitions are useful for periodic updates, counters, or logging while remaining in the current state.

### Custom Derives

```rust
statemachine! {
    derive_states: [Debug, Clone, PartialEq, Eq, Hash],
    derive_events: [Debug, Clone, PartialEq],
    transitions: {
        *Idle + Start = Running,
    }
}
```

### Multiple State Machines

Use namespacing for multiple state machines:

```rust
statemachine! {
    name: Player,
    transitions: {
        *Idle + Move = Walking,
    }
}

statemachine! {
    name: Enemy,
    transitions: {
        *Patrol + Spot = Chasing,
    }
}

// Generates: PlayerState, PlayerEvent with PlayerState::process_event()
// Generates: EnemyState, EnemyEvent with EnemyState::process_event()
```

## DSL Syntax

```rust
statemachine! {
    // Optional: namespace for multiple state machines
    name: MyMachine,

    // Optional: custom derives for State enum
    derive_states: [Debug, Clone, PartialEq],

    // Optional: custom derives for Event enum
    derive_events: [Debug, Clone, PartialEq],

    // Required: transition definitions
    transitions: {
        // Basic transition (initial state marked with *)
        *Idle + Start = Running,

        // State patterns (multiple source states)
        Ready | Waiting + Start = Active,

        // Event patterns (multiple trigger events)
        Active + Stop | Pause = Idle,

        // Wildcard (from any state)
        _ + Reset = Idle,

        // Internal transition (stay in same state)
        Active + Tick = _,
    }
}
```

## Generated Code

The macro generates:

```rust
// State enum
pub enum State {
    Idle,
    Running,
}

impl Default for State {
    fn default() -> Self {
        State::Idle  // First state marked with *
    }
}

// Event enum
pub enum Event {
    Start,
    Stop,
}

// Transition method on State
impl State {
    pub fn process_event(&self, event: Event) -> Option<State> {
        // Returns Some(new_state) if transition is valid
        // Returns None if no valid transition
    }
}
```

## Error Handling

`process_event` returns `Option<State>`:
- `Some(new_state)`: Transition is valid
- `None`: No valid transition for current state + event

You control when to apply the transition:

```rust
impl Machine {
    fn try_transition(&mut self, event: Event) {
        let Some(new_state) = self.state.process_event(event) else {
            println!("Invalid transition");
            return;
        };

        // Guards and actions here

        self.state = new_state;
    }
}
```

## Compile Time Validation

The macro validates your state machine at compile time.

### Duplicate Transitions

```rust
statemachine! {
    transitions: {
        *A + Event = B,
        A + Event = C,  // ERROR: duplicate transition
    }
}
```

Error message:
```
error: duplicate transition: state 'A' + event 'Event' is already defined
       help: each combination of source state and event can only appear once
       note: if you need conditional behavior, use different events or handle logic in your wrapper
```

## Performance

- **Zero cost**: Compiles to sequential `if` checks with early returns
- **No allocations**: All operations are stack based
- **Optimal codegen**: Uses `matches!()` macro for efficient pattern matching
- **No runtime overhead**: All validation happens at compile time

## FAQ

**Q: How do I write guards and actions?**

A: Call `state.process_event(event)` to get the new state if valid. Check your guards, perform actions, then apply the state. This gives you full control, zero coupling, and clean code. See the [Guards and Actions](#guards-and-actions) section.

**Q: Can I use this in `no_std` environments?**

A: Yes! The library uses `#![no_std]` and only requires `alloc` for compilation (not at runtime).

**Q: How do I handle conditional transitions?**

A: `state.process_event(event)` returns `Option<State>`. Get the new state, check your guards with early returns, perform actions, then assign `self.state = new_state`. All guards must pass before the state changes.

## Examples

See the [examples](examples/) directory for complete working examples:
- `demo.rs`: Comprehensive robot control demonstrating all DSL features including guards, actions, state patterns, internal transitions, and wildcard transitions
- `hierarchical.rs`: Hierarchical state machines using composition (player movement + weapon states)

Run examples with:
```bash
cargo run -r --example demo
cargo run -r --example hierarchical
```

## License

This project is licensed under the MIT License. See the [MIT.md](MIT.md) file for details.
