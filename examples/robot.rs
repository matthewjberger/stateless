use nightshade::tui::prelude::*;
use stateless::statemachine;

statemachine! {
    name: Robot,
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

const ARENA_LEFT: i32 = 1;
const ARENA_TOP: i32 = 3;
const ARENA_WIDTH: i32 = 50;
const ARENA_HEIGHT: i32 = 20;
const MOVE_INTERVAL: f64 = 0.08;

struct Obstacle {
    column: i32,
    row: i32,
    entity: Entity,
}

struct RobotDemo {
    state: RobotState,
    robot_entity: Entity,
    target_entity: Entity,
    hud_entities: EntityGroup,
    border_entities: Vec<Entity>,
    obstacle_entities: Vec<Obstacle>,
    trail_entities: Vec<Entity>,
    robot_column: i32,
    robot_row: i32,
    target_column: i32,
    target_row: i32,
    move_timer: f64,
    battery: u32,
    obstacles_cleared: u32,
}

impl Default for RobotDemo {
    fn default() -> Self {
        Self {
            state: RobotState::default(),
            robot_entity: Entity::default(),
            target_entity: Entity::default(),
            hud_entities: EntityGroup::new(),
            border_entities: Vec::new(),
            obstacle_entities: Vec::new(),
            trail_entities: Vec::new(),
            robot_column: ARENA_LEFT + 2,
            robot_row: ARENA_TOP + ARENA_HEIGHT / 2,
            target_column: ARENA_LEFT + ARENA_WIDTH - 3,
            target_row: ARENA_TOP + ARENA_HEIGHT / 2,
            move_timer: 0.0,
            battery: 100,
            obstacles_cleared: 0,
        }
    }
}

impl RobotDemo {
    fn draw_border(&mut self, world: &mut World) {
        let right = ARENA_LEFT + ARENA_WIDTH;
        let bottom = ARENA_TOP + ARENA_HEIGHT;

        for column in ARENA_LEFT - 1..=right {
            self.spawn_border(world, column, ARENA_TOP - 1, '═');
            self.spawn_border(world, column, bottom, '═');
        }

        for row in ARENA_TOP - 1..=bottom {
            self.spawn_border(world, ARENA_LEFT - 1, row, '║');
            self.spawn_border(world, right, row, '║');
        }

        self.spawn_border(world, ARENA_LEFT - 1, ARENA_TOP - 1, '╔');
        self.spawn_border(world, right, ARENA_TOP - 1, '╗');
        self.spawn_border(world, ARENA_LEFT - 1, bottom, '╚');
        self.spawn_border(world, right, bottom, '╝');
    }

    fn spawn_border(&mut self, world: &mut World, column: i32, row: i32, character: char) {
        let entity = world.spawn_entities(POSITION | SPRITE | Z_INDEX, 1)[0];
        world.set_position(
            entity,
            Position {
                column: column as f64,
                row: row as f64,
            },
        );
        world.set_sprite(
            entity,
            Sprite {
                character,
                foreground: TermColor::Grey,
                background: TermColor::Black,
            },
        );
        world.set_z_index(entity, ZIndex(1));
        self.border_entities.push(entity);
    }

    fn spawn_obstacles(&mut self, world: &mut World) {
        let positions = [
            (ARENA_LEFT + 12, ARENA_TOP + 5),
            (ARENA_LEFT + 12, ARENA_TOP + 6),
            (ARENA_LEFT + 12, ARENA_TOP + 7),
            (ARENA_LEFT + 20, ARENA_TOP + 10),
            (ARENA_LEFT + 20, ARENA_TOP + 11),
            (ARENA_LEFT + 20, ARENA_TOP + 12),
            (ARENA_LEFT + 20, ARENA_TOP + 13),
            (ARENA_LEFT + 30, ARENA_TOP + 3),
            (ARENA_LEFT + 30, ARENA_TOP + 4),
            (ARENA_LEFT + 30, ARENA_TOP + 5),
            (ARENA_LEFT + 38, ARENA_TOP + 8),
            (ARENA_LEFT + 38, ARENA_TOP + 9),
            (ARENA_LEFT + 38, ARENA_TOP + 10),
            (ARENA_LEFT + 38, ARENA_TOP + 11),
        ];

        for (column, row) in positions {
            let entity = world.spawn_entities(POSITION | SPRITE | Z_INDEX, 1)[0];
            world.set_position(
                entity,
                Position {
                    column: column as f64,
                    row: row as f64,
                },
            );
            world.set_sprite(
                entity,
                Sprite {
                    character: '█',
                    foreground: TermColor::DarkRed,
                    background: TermColor::Black,
                },
            );
            world.set_z_index(entity, ZIndex(2));
            self.obstacle_entities.push(Obstacle {
                column,
                row,
                entity,
            });
        }
    }

    fn update_robot_sprite(&self, world: &mut World) {
        let (character, color) = match self.state {
            RobotState::Off => ('○', TermColor::DarkGrey),
            RobotState::Idle => ('●', TermColor::Green),
            RobotState::Moving => ('▶', TermColor::Cyan),
            RobotState::Waiting => ('◆', TermColor::Yellow),
            RobotState::EmergencyStopped => ('✕', TermColor::Red),
        };

        if let Some(sprite) = world.get_sprite_mut(self.robot_entity) {
            sprite.character = character;
            sprite.foreground = color;
        }
    }

    fn update_robot_position(&self, world: &mut World) {
        if let Some(position) = world.get_position_mut(self.robot_entity) {
            position.column = self.robot_column as f64;
            position.row = self.robot_row as f64;
        }
    }

    fn update_target_visibility(&self, world: &mut World) {
        let visible = self.state == RobotState::Moving || self.state == RobotState::Waiting;
        if let Some(sprite) = world.get_sprite_mut(self.target_entity) {
            sprite.character = if visible { '◎' } else { ' ' };
        }
        if let Some(position) = world.get_position_mut(self.target_entity) {
            position.column = self.target_column as f64;
            position.row = self.target_row as f64;
        }
    }

    fn leave_trail(&mut self, world: &mut World) {
        let entity = world.spawn_entities(POSITION | SPRITE | Z_INDEX, 1)[0];
        world.set_position(
            entity,
            Position {
                column: self.robot_column as f64,
                row: self.robot_row as f64,
            },
        );
        world.set_sprite(
            entity,
            Sprite {
                character: '·',
                foreground: TermColor::DarkCyan,
                background: TermColor::Black,
            },
        );
        world.set_z_index(entity, ZIndex(1));
        self.trail_entities.push(entity);
    }

    fn obstacle_at(&self, column: i32, row: i32) -> bool {
        self.obstacle_entities
            .iter()
            .any(|obstacle| obstacle.column == column && obstacle.row == row)
    }

    fn clear_obstacle_at(&mut self, world: &mut World, column: i32, row: i32) {
        if let Some(index) = self
            .obstacle_entities
            .iter()
            .position(|obstacle| obstacle.column == column && obstacle.row == row)
        {
            let obstacle = self.obstacle_entities.remove(index);
            world.despawn_entities(&[obstacle.entity]);
            self.obstacles_cleared += 1;
        }
    }

    fn step_toward_target(&self) -> (i32, i32) {
        let delta_column = (self.target_column - self.robot_column).signum();
        let delta_row = (self.target_row - self.robot_row).signum();

        if delta_column != 0 {
            (delta_column, 0)
        } else {
            (0, delta_row)
        }
    }

    fn power_on(&mut self) {
        if let Some(new_state) = self.state.process_event(RobotEvent::PowerOn) {
            self.battery = 100;
            self.state = new_state;
        }
    }

    fn power_off(&mut self) {
        if let Some(new_state) = self.state.process_event(RobotEvent::PowerOff) {
            self.state = new_state;
        }
    }

    fn move_to(&mut self, column: i32, row: i32) {
        let Some(new_state) = self.state.process_event(RobotEvent::MoveTo) else {
            return;
        };

        if self.battery < 10 {
            return;
        }

        self.target_column = column.clamp(ARENA_LEFT, ARENA_LEFT + ARENA_WIDTH - 1);
        self.target_row = row.clamp(ARENA_TOP, ARENA_TOP + ARENA_HEIGHT - 1);
        self.state = new_state;
    }

    fn tick_movement(&mut self, world: &mut World) {
        let Some(new_state) = self.state.process_event(RobotEvent::Tick) else {
            return;
        };

        if self.robot_column == self.target_column && self.robot_row == self.target_row {
            if let Some(arrived) = self.state.process_event(RobotEvent::Arrive) {
                self.state = arrived;
            }
            return;
        }

        let (delta_column, delta_row) = self.step_toward_target();
        let next_column = self.robot_column + delta_column;
        let next_row = self.robot_row + delta_row;

        if self.obstacle_at(next_column, next_row) {
            if let Some(waiting) = self.state.process_event(RobotEvent::ObstacleDetected) {
                self.state = waiting;
            }
            return;
        }

        self.leave_trail(world);
        self.robot_column = next_column;
        self.robot_row = next_row;
        self.battery = self.battery.saturating_sub(1);
        self.state = new_state;
    }

    fn try_clear_obstacle(&mut self, world: &mut World) {
        let Some(new_state) = self.state.process_event(RobotEvent::ObstacleClear) else {
            return;
        };

        let (delta_column, delta_row) = self.step_toward_target();
        let obstacle_column = self.robot_column + delta_column;
        let obstacle_row = self.robot_row + delta_row;
        self.clear_obstacle_at(world, obstacle_column, obstacle_row);
        self.battery = self.battery.saturating_sub(5);
        self.state = new_state;
    }

    fn emergency_stop(&mut self) {
        if let Some(new_state) = self.state.process_event(RobotEvent::EmergencyStop) {
            self.state = new_state;
        }
    }

    fn reset(&mut self) {
        let Some(new_state) = self.state.process_event(RobotEvent::Reset) else {
            return;
        };

        if self.battery < 5 {
            return;
        }

        self.state = new_state;
    }

    fn clear_trail(&mut self, world: &mut World) {
        for &entity in &self.trail_entities {
            world.despawn_entities(&[entity]);
        }
        self.trail_entities.clear();
    }

    fn update_hud(&mut self, world: &mut World) {
        self.hud_entities.despawn_all(world);

        let state_label = match self.state {
            RobotState::Off => "OFF",
            RobotState::Idle => "IDLE",
            RobotState::Moving => "MOVING",
            RobotState::Waiting => "OBSTACLE",
            RobotState::EmergencyStopped => "E-STOP",
        };

        let state_color = match self.state {
            RobotState::Off => TermColor::DarkGrey,
            RobotState::Idle => TermColor::Green,
            RobotState::Moving => TermColor::Cyan,
            RobotState::Waiting => TermColor::Yellow,
            RobotState::EmergencyStopped => TermColor::Red,
        };

        let line1 = format!(
            "State: {:<10} Battery: {:>3}%  Cleared: {}",
            state_label, self.battery, self.obstacles_cleared
        );

        let entity = self
            .hud_entities
            .spawn_one(world, POSITION | LABEL | Z_INDEX);
        world.set_position(
            entity,
            Position {
                column: 1.0,
                row: 0.0,
            },
        );
        world.set_label(
            entity,
            Label {
                text: line1,
                foreground: state_color,
                background: TermColor::Black,
            },
        );
        world.set_z_index(entity, ZIndex(10));

        let line2 = match self.state {
            RobotState::Off => "P: power on  Q: quit".to_string(),
            RobotState::Idle => "WASD: move to  P: power off  E: e-stop  Q: quit".to_string(),
            RobotState::Moving => "E: e-stop  P: power off  Q: quit".to_string(),
            RobotState::Waiting => {
                "C: clear obstacle  E: e-stop  P: power off  Q: quit".to_string()
            }
            RobotState::EmergencyStopped => "R: reset  P: power off  Q: quit".to_string(),
        };

        let entity2 = self
            .hud_entities
            .spawn_one(world, POSITION | LABEL | Z_INDEX);
        world.set_position(
            entity2,
            Position {
                column: 1.0,
                row: 1.0,
            },
        );
        world.set_label(
            entity2,
            Label {
                text: line2,
                foreground: TermColor::Grey,
                background: TermColor::Black,
            },
        );
        world.set_z_index(entity2, ZIndex(10));
    }
}

impl State for RobotDemo {
    fn title(&self) -> &str {
        "Stateless Robot Demo"
    }

    fn initialize(&mut self, world: &mut World) {
        world.resources.timing.target_fps = 60;

        self.draw_border(world);
        self.spawn_obstacles(world);

        self.robot_entity = world.spawn_entities(POSITION | SPRITE | Z_INDEX, 1)[0];
        world.set_z_index(self.robot_entity, ZIndex(5));
        self.update_robot_position(world);
        self.update_robot_sprite(world);

        self.target_entity = world.spawn_entities(POSITION | SPRITE | Z_INDEX, 1)[0];
        world.set_sprite(
            self.target_entity,
            Sprite {
                character: ' ',
                foreground: TermColor::DarkGreen,
                background: TermColor::Black,
            },
        );
        world.set_z_index(self.target_entity, ZIndex(3));

        self.update_hud(world);
    }

    fn on_keyboard_input(&mut self, world: &mut World, key: KeyCode, pressed: bool) {
        if !pressed {
            return;
        }

        match key {
            KeyCode::Escape | KeyCode::Char('q') => {
                world.resources.should_exit = true;
            }
            KeyCode::Char('p') => {
                if self.state == RobotState::Off {
                    self.power_on();
                } else {
                    self.power_off();
                    self.clear_trail(world);
                }
            }
            KeyCode::Char('e') => {
                self.emergency_stop();
            }
            KeyCode::Char('r') => {
                self.reset();
            }
            KeyCode::Char('c') => {
                self.try_clear_obstacle(world);
            }
            KeyCode::Char('w') | KeyCode::Up => {
                self.move_to(self.robot_column, ARENA_TOP);
            }
            KeyCode::Char('s') | KeyCode::Down => {
                self.move_to(self.robot_column, ARENA_TOP + ARENA_HEIGHT - 1);
            }
            KeyCode::Char('a') | KeyCode::Left => {
                self.move_to(ARENA_LEFT, self.robot_row);
            }
            KeyCode::Char('d') | KeyCode::Right => {
                self.move_to(ARENA_LEFT + ARENA_WIDTH - 1, self.robot_row);
            }
            _ => {}
        }
    }

    fn run_systems(&mut self, world: &mut World) {
        if self.state == RobotState::Moving {
            self.move_timer += world.resources.timing.delta_seconds;
            if self.move_timer >= MOVE_INTERVAL {
                self.move_timer = 0.0;
                self.tick_movement(world);
            }
        }

        self.update_robot_position(world);
        self.update_robot_sprite(world);
        self.update_target_visibility(world);
        self.update_hud(world);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    launch(Box::new(RobotDemo::default()))
}
