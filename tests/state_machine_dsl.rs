use stateless::statemachine;

#[test]
fn test_state_machine_dsl() {
    statemachine! {
        derive_states: [Debug, Clone, PartialEq, Eq, Hash],
        derive_events: [Debug, Clone, PartialEq],
        transitions: {
            *Idle + Start = Running,
            Running + Pause | Stop = Idle,
            Idle | Running + Connect = Connected,
            Connected + Disconnect = Idle,
            Connected + Tick = _,
            _ + Reset = Idle,
        }
    }

    struct Machine {
        state: State,
        battery: u32,
        connection_id: u32,
        tick_count: u32,
        max_connections: u32,
    }

    impl Machine {
        fn new() -> Self {
            Self {
                state: State::default(),
                battery: 100,
                connection_id: 0,
                tick_count: 0,
                max_connections: 5,
            }
        }

        fn start(&mut self) {
            let Some(new_state) = self.state.process_event(Event::Start) else {
                return;
            };

            if self.battery < 20 {
                return;
            }

            self.battery -= 10;
            self.state = new_state;
        }

        fn pause(&mut self) {
            if let Some(new_state) = self.state.process_event(Event::Pause) {
                self.state = new_state;
            }
        }

        fn stop(&mut self) {
            if let Some(new_state) = self.state.process_event(Event::Stop) {
                self.state = new_state;
            }
        }

        fn connect(&mut self, id: u32) {
            let Some(new_state) = self.state.process_event(Event::Connect) else {
                return;
            };

            if id > self.max_connections {
                return;
            }

            if self.battery < 5 {
                return;
            }

            self.connection_id = id;
            self.battery -= 5;
            self.state = new_state;
        }

        fn disconnect(&mut self) {
            if let Some(new_state) = self.state.process_event(Event::Disconnect) {
                self.connection_id = 0;
                self.state = new_state;
            }
        }

        fn tick(&mut self) {
            if let Some(new_state) = self.state.process_event(Event::Tick) {
                self.tick_count += 1;
                self.state = new_state;
            }
        }

        fn reset(&mut self) {
            if let Some(new_state) = self.state.process_event(Event::Reset) {
                self.battery = 100;
                self.connection_id = 0;
                self.tick_count = 0;
                self.state = new_state;
            }
        }
    }

    let mut machine = Machine::new();

    assert_eq!(machine.state, State::Idle);
    assert_eq!(machine.battery, 100);

    machine.start();
    assert_eq!(machine.state, State::Running);
    assert_eq!(machine.battery, 90);

    machine.pause();
    assert_eq!(machine.state, State::Idle);

    machine.start();
    assert_eq!(machine.state, State::Running);
    assert_eq!(machine.battery, 80);

    machine.stop();
    assert_eq!(machine.state, State::Idle);

    machine.connect(3);
    assert_eq!(machine.state, State::Connected);
    assert_eq!(machine.connection_id, 3);
    assert_eq!(machine.battery, 75);

    machine.tick();
    assert_eq!(machine.state, State::Connected);
    assert_eq!(machine.tick_count, 1);

    machine.tick();
    assert_eq!(machine.tick_count, 2);

    machine.disconnect();
    assert_eq!(machine.state, State::Idle);
    assert_eq!(machine.connection_id, 0);

    machine.start();
    machine.connect(4);
    assert_eq!(machine.state, State::Connected);

    machine.reset();
    assert_eq!(machine.state, State::Idle);
    assert_eq!(machine.battery, 100);
    assert_eq!(machine.connection_id, 0);
    assert_eq!(machine.tick_count, 0);

    machine.battery = 10;
    machine.start();
    assert_eq!(machine.state, State::Idle);

    machine.battery = 100;
    machine.start();
    machine.connect(10);
    assert_eq!(machine.state, State::Running);
    assert_eq!(machine.connection_id, 0);
}

#[test]
fn namespace_control() {
    statemachine! {
        name: Player,
        transitions: {
            *Idle + Move = Walking,
            Walking + Stop = Idle,
        }
    }

    statemachine! {
        name: Enemy,
        transitions: {
            *Patrol + Spot = Chasing,
            Chasing + Lose = Patrol,
        }
    }

    let mut player = PlayerState::default();
    assert_eq!(player, PlayerState::Idle);

    if let Some(new_state) = player.process_event(PlayerEvent::Move) {
        player = new_state;
    }
    assert_eq!(player, PlayerState::Walking);

    if let Some(new_state) = player.process_event(PlayerEvent::Stop) {
        player = new_state;
    }
    assert_eq!(player, PlayerState::Idle);

    let mut enemy = EnemyState::default();
    assert_eq!(enemy, EnemyState::Patrol);

    if let Some(new_state) = enemy.process_event(EnemyEvent::Spot) {
        enemy = new_state;
    }
    assert_eq!(enemy, EnemyState::Chasing);

    if let Some(new_state) = enemy.process_event(EnemyEvent::Lose) {
        enemy = new_state;
    }
    assert_eq!(enemy, EnemyState::Patrol);
}

#[test]
fn invalid_transitions_return_none() {
    statemachine! {
        name: Invalid,
        transitions: {
            *Idle + Start = Running,
            Running + Stop = Idle,
        }
    }

    let idle = InvalidState::Idle;
    assert!(idle.process_event(InvalidEvent::Stop).is_none());

    let running = InvalidState::Running;
    assert!(running.process_event(InvalidEvent::Start).is_none());
}

#[test]
fn wildcard_priority() {
    statemachine! {
        name: Priority,
        transitions: {
            *A + Go = B,
            B + Go = C,
            _ + Go = A,
        }
    }

    assert_eq!(
        PriorityState::A.process_event(PriorityEvent::Go),
        Some(PriorityState::B)
    );
    assert_eq!(
        PriorityState::B.process_event(PriorityEvent::Go),
        Some(PriorityState::C)
    );
    assert_eq!(
        PriorityState::C.process_event(PriorityEvent::Go),
        Some(PriorityState::A)
    );
}

#[test]
fn wildcard_internal_transition() {
    statemachine! {
        name: WildInternal,
        transitions: {
            *A + Next = B,
            B + Next = A,
            _ + Ping = _,
        }
    }

    assert_eq!(
        WildInternalState::A.process_event(WildInternalEvent::Ping),
        Some(WildInternalState::A)
    );
    assert_eq!(
        WildInternalState::B.process_event(WildInternalEvent::Ping),
        Some(WildInternalState::B)
    );
    assert_eq!(
        WildInternalState::A.process_event(WildInternalEvent::Next),
        Some(WildInternalState::B)
    );
}

#[test]
fn default_initial_state_without_marker() {
    statemachine! {
        name: NoMarker,
        transitions: {
            First + Go = Second,
            Second + Go = First,
        }
    }

    assert_eq!(NoMarkerState::default(), NoMarkerState::First);
    assert_eq!(
        NoMarkerState::First.process_event(NoMarkerEvent::Go),
        Some(NoMarkerState::Second)
    );
}

#[test]
fn default_derives() {
    statemachine! {
        name: Defaults,
        transitions: {
            *On + Toggle = Off,
            Off + Toggle = On,
        }
    }

    let state = DefaultsState::On;
    let copied = state;
    assert_eq!(state, copied);
    assert_eq!(format!("{:?}", state), "On");

    let result = state.process_event(DefaultsEvent::Toggle);
    assert_eq!(result, Some(DefaultsState::Off));

    let event = DefaultsEvent::Toggle;
    let copied_event = event;
    assert_eq!(event, copied_event);
    assert_eq!(format!("{:?}", event), "Toggle");
}

#[test]
fn copy_after_process_event() {
    statemachine! {
        name: CopyTest,
        transitions: {
            *A + Go = B,
        }
    }

    let state = CopyTestState::A;
    let event = CopyTestEvent::Go;
    let result = state.process_event(event);
    assert_eq!(state, CopyTestState::A);
    assert_eq!(event, CopyTestEvent::Go);
    assert_eq!(result, Some(CopyTestState::B));
}

#[test]
fn state_pattern_with_single_state() {
    statemachine! {
        name: SinglePat,
        transitions: {
            *A + Go = B,
            B + Go = A,
        }
    }

    assert_eq!(
        SinglePatState::A.process_event(SinglePatEvent::Go),
        Some(SinglePatState::B)
    );
    assert_eq!(
        SinglePatState::B.process_event(SinglePatEvent::Go),
        Some(SinglePatState::A)
    );
}

#[test]
fn multiple_event_patterns() {
    statemachine! {
        name: MultiEvent,
        transitions: {
            *Active + Pause | Stop | Cancel = Idle,
            Idle + Start = Active,
        }
    }

    assert_eq!(
        MultiEventState::Active.process_event(MultiEventEvent::Pause),
        Some(MultiEventState::Idle)
    );
    assert_eq!(
        MultiEventState::Active.process_event(MultiEventEvent::Stop),
        Some(MultiEventState::Idle)
    );
    assert_eq!(
        MultiEventState::Active.process_event(MultiEventEvent::Cancel),
        Some(MultiEventState::Idle)
    );
    assert!(
        MultiEventState::Idle
            .process_event(MultiEventEvent::Pause)
            .is_none()
    );
    assert_eq!(
        MultiEventState::Idle.process_event(MultiEventEvent::Start),
        Some(MultiEventState::Active)
    );
}

#[test]
fn multiple_state_patterns() {
    statemachine! {
        name: MultiState,
        transitions: {
            *A | B | C + Reset = A,
            A + Next = B,
            B + Next = C,
        }
    }

    assert_eq!(
        MultiStateState::A.process_event(MultiStateEvent::Reset),
        Some(MultiStateState::A)
    );
    assert_eq!(
        MultiStateState::B.process_event(MultiStateEvent::Reset),
        Some(MultiStateState::A)
    );
    assert_eq!(
        MultiStateState::C.process_event(MultiStateEvent::Reset),
        Some(MultiStateState::A)
    );
    assert_eq!(
        MultiStateState::A.process_event(MultiStateEvent::Next),
        Some(MultiStateState::B)
    );
    assert_eq!(
        MultiStateState::B.process_event(MultiStateEvent::Next),
        Some(MultiStateState::C)
    );
}

#[test]
fn internal_transition_preserves_state() {
    statemachine! {
        name: Internal,
        transitions: {
            *A + Tick = _,
            A + Go = B,
            B + Tick = _,
        }
    }

    assert_eq!(
        InternalState::A.process_event(InternalEvent::Tick),
        Some(InternalState::A)
    );
    assert_eq!(
        InternalState::B.process_event(InternalEvent::Tick),
        Some(InternalState::B)
    );
    assert_eq!(
        InternalState::A.process_event(InternalEvent::Go),
        Some(InternalState::B)
    );
}

#[test]
fn target_only_state_is_reachable() {
    statemachine! {
        name: TargetOnly,
        transitions: {
            *Start + Go = End,
        }
    }

    assert_eq!(
        TargetOnlyState::Start.process_event(TargetOnlyEvent::Go),
        Some(TargetOnlyState::End)
    );
    assert!(
        TargetOnlyState::End
            .process_event(TargetOnlyEvent::Go)
            .is_none()
    );
}

#[test]
fn wildcard_with_no_specific_overlap() {
    statemachine! {
        name: WildNoOverlap,
        transitions: {
            *A + Step = B,
            B + Step = C,
            _ + Reset = A,
        }
    }

    assert_eq!(
        WildNoOverlapState::A.process_event(WildNoOverlapEvent::Reset),
        Some(WildNoOverlapState::A)
    );
    assert_eq!(
        WildNoOverlapState::B.process_event(WildNoOverlapEvent::Reset),
        Some(WildNoOverlapState::A)
    );
    assert_eq!(
        WildNoOverlapState::C.process_event(WildNoOverlapEvent::Reset),
        Some(WildNoOverlapState::A)
    );
    assert_eq!(
        WildNoOverlapState::A.process_event(WildNoOverlapEvent::Step),
        Some(WildNoOverlapState::B)
    );
}

#[test]
fn initial_marker_on_non_first_state() {
    statemachine! {
        name: LateInit,
        transitions: {
            A + Go = B,
            *B + Go = A,
        }
    }

    assert_eq!(LateInitState::default(), LateInitState::B);
    assert_eq!(
        LateInitState::B.process_event(LateInitEvent::Go),
        Some(LateInitState::A)
    );
}
