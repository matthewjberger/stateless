use stateless::statemachine;

statemachine! {
    derive_states: [Debug, Clone, PartialEq, Eq],
    derive_events: [Debug, Clone, PartialEq, Eq],
    transitions: {
        *Off + PowerOn = Idle,
        Idle + MoveTo = Moving,
        Moving + Tick = _,
        Moving + Arrive = Idle,
        Moving + ObstacleDetected = Waiting,
        Waiting + ObstacleClear = Moving,
        Idle | Moving | Waiting + EmergencyStop = EmergencyStopped,
        EmergencyStopped + Reset = Idle,
        _ + PowerOff = Off,
    }
}

struct Robot {
    state: State,
    current_position: u32,
    target_position: Option<u32>,
    obstacle_count: u32,
    battery: u32,
    movement_ticks: u32,
}

impl Robot {
    fn new() -> Self {
        Self {
            state: State::default(),
            current_position: 0,
            target_position: None,
            obstacle_count: 0,
            battery: 100,
            movement_ticks: 0,
        }
    }

    fn power_on(&mut self) {
        let Some(new_state) = self.state.process_event(Event::PowerOn) else {
            return;
        };

        println!("  [Action] Engaging motors (battery: {}%)", self.battery);
        println!("  [State] Robot ready");
        self.state = new_state;
    }

    fn power_off(&mut self) {
        let Some(new_state) = self.state.process_event(Event::PowerOff) else {
            return;
        };

        println!("  [State] Robot powered off");
        self.current_position = 0;
        self.target_position = None;
        self.obstacle_count = 0;
        self.state = new_state;
    }

    fn move_to(&mut self, position: u32) {
        let Some(new_state) = self.state.process_event(Event::MoveTo) else {
            return;
        };

        self.target_position = Some(position);
        self.movement_ticks = 0;
        println!("  [State] Moving to position {}", position);
        println!(
            "  [Action] Moving to position {} from {}",
            position, self.current_position
        );
        self.state = new_state;
    }

    fn tick(&mut self) {
        let Some(new_state) = self.state.process_event(Event::Tick) else {
            return;
        };

        self.movement_ticks += 1;
        println!("  [Internal] Movement tick {} (still Moving)", self.movement_ticks);
        self.state = new_state;
    }

    fn check_position(&mut self) {
        let Some(target) = self.target_position else {
            return;
        };

        if self.current_position == target {
            let Some(new_state) = self.state.process_event(Event::Arrive) else {
                return;
            };

            println!("  [State] Robot ready");
            println!("  [Info] Position reached: {}", self.current_position);
            self.target_position = None;
            self.state = new_state;
        } else {
            println!("  [Info] Target not reached yet");
        }
    }

    fn obstacle_detected(&mut self) {
        let Some(new_state) = self.state.process_event(Event::ObstacleDetected) else {
            return;
        };

        self.obstacle_count += 1;
        println!(
            "  [State] Obstacle detected, waiting... (count: {})",
            self.obstacle_count
        );
        self.state = new_state;
    }

    fn try_clear_obstacle(&mut self) {
        let Some(new_state) = self.state.process_event(Event::ObstacleClear) else {
            return;
        };

        if self.obstacle_count >= 3 {
            println!("  [Guard] Too many obstacles, cannot continue");
            return;
        }

        println!("  [State] Resuming movement");
        self.state = new_state;
    }

    fn emergency_stop(&mut self) {
        let Some(new_state) = self.state.process_event(Event::EmergencyStop) else {
            return;
        };

        println!("  [State] EMERGENCY STOP ACTIVATED");
        self.state = new_state;
    }

    fn try_reset(&mut self) {
        let Some(new_state) = self.state.process_event(Event::Reset) else {
            return;
        };

        if self.battery <= 10 {
            println!("  [Guard] Insufficient power to reset");
            return;
        }

        println!("  [State] Robot ready");
        self.state = new_state;
    }
}

fn main() {
    let mut robot = Robot::new();

    println!("═══ Startup Sequence ═══");
    println!("Current state: {:?}\n", robot.state);

    println!("Command: PowerOn");
    robot.power_on();
    println!();

    println!("═══ Normal Operation ═══");
    println!("Command: MoveTo(100)");
    robot.move_to(100);
    println!();

    println!("Command: Tick (internal transition - stays in Moving state)");
    robot.tick();
    robot.tick();
    robot.tick();
    println!();

    println!("Command: Check position");
    robot.current_position = 100;
    robot.check_position();
    println!();

    println!("═══ Obstacle Handling ═══");
    println!("Command: MoveTo(200)");
    robot.move_to(200);
    println!();

    println!("Command: ObstacleDetected");
    robot.obstacle_detected();
    println!();

    println!("Command: ObstacleClear");
    robot.try_clear_obstacle();
    println!();

    println!("Command: Check position");
    robot.current_position = 200;
    robot.check_position();
    println!();

    println!("═══ Emergency Stop ═══");
    println!("Command: MoveTo(300)");
    robot.move_to(300);
    println!();

    println!("Command: EmergencyStop (can be triggered from any active state)");
    robot.emergency_stop();
    println!();

    println!("Command: Reset (requires power)");
    robot.try_reset();
    println!();

    println!("═══ Wildcard Transition (from any state) ═══");
    println!("Command: PowerOff");
    robot.power_off();
    println!();

    println!("Final position: {}", robot.current_position);
    println!("Total obstacles encountered: {}", robot.obstacle_count);
}
