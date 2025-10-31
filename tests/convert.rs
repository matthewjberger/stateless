use stateless::statemachine;

#[test]
fn comprehensive_state_machine_features() {
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
