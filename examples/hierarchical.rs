use stateless::statemachine;

statemachine! {
    name: Player,
    transitions: {
        *Idle + StartWalking = Walking,
        Walking + StopWalking = Idle,
        Idle | Walking + StartRunning = Running,
        Running + StopRunning = Idle,
        _ + PickUpItem = Idle,
        _ + DropItem = Idle,
    }
}

statemachine! {
    name: Item,
    transitions: {
        *Holstered + Draw = Ready,
        Ready + Holster = Holstered,
        Ready + Fire = Firing,
        Firing + CooldownComplete = Ready,
    }
}

struct Player {
    state: PlayerState,
    position: (f32, f32),
    speed: f32,
    item: Option<LaserGun>,
}

struct LaserGun {
    state: ItemState,
    charge: u32,
    max_charge: u32,
}

impl Player {
    fn new() -> Self {
        Self {
            state: PlayerState::default(),
            position: (0.0, 0.0),
            speed: 0.0,
            item: None,
        }
    }

    fn start_walking(&mut self) {
        let Some(new_state) = self.state.process_event(PlayerEvent::StartWalking) else {
            return;
        };

        self.speed = 1.0;
        println!("Player starts walking (speed: {})", self.speed);
        self.state = new_state;
    }

    fn stop_walking(&mut self) {
        let Some(new_state) = self.state.process_event(PlayerEvent::StopWalking) else {
            return;
        };

        self.speed = 0.0;
        println!("Player stops walking");
        self.state = new_state;
    }

    fn start_running(&mut self) {
        let Some(new_state) = self.state.process_event(PlayerEvent::StartRunning) else {
            return;
        };

        self.speed = 2.5;
        println!("Player starts running (speed: {})", self.speed);
        self.state = new_state;
    }

    fn stop_running(&mut self) {
        let Some(new_state) = self.state.process_event(PlayerEvent::StopRunning) else {
            return;
        };

        self.speed = 0.0;
        println!("Player stops running");
        self.state = new_state;
    }

    fn update_position(&mut self, delta_time: f32) {
        if self.speed > 0.0 {
            self.position.0 += self.speed * delta_time;
            println!(
                "Player position: ({:.1}, {:.1})",
                self.position.0, self.position.1
            );
        }
    }

    fn pick_up_item(&mut self) {
        let Some(new_state) = self.state.process_event(PlayerEvent::PickUpItem) else {
            return;
        };

        if self.item.is_some() {
            println!("Already holding an item");
            return;
        }

        self.item = Some(LaserGun::new());
        println!("Player picks up laser gun");
        self.state = new_state;
    }

    fn drop_item(&mut self) {
        let Some(new_state) = self.state.process_event(PlayerEvent::DropItem) else {
            return;
        };

        if self.item.is_none() {
            println!("Not holding any item");
            return;
        }

        self.item = None;
        println!("Player drops item");
        self.state = new_state;
    }

    fn draw_weapon(&mut self) {
        if let Some(gun) = &mut self.item {
            gun.draw();
        } else {
            println!("No item to draw");
        }
    }

    fn holster_weapon(&mut self) {
        if let Some(gun) = &mut self.item {
            gun.holster();
        } else {
            println!("No item to holster");
        }
    }

    fn fire_weapon(&mut self) {
        if let Some(gun) = &mut self.item {
            gun.fire();
        } else {
            println!("No weapon equipped");
        }
    }

    fn weapon_cooldown_complete(&mut self) {
        if let Some(gun) = &mut self.item {
            gun.cooldown_complete();
        } else {
            println!("No weapon equipped");
        }
    }

    fn recharge_weapon(&mut self) {
        if let Some(gun) = &mut self.item {
            gun.recharge();
        }
    }
}

impl LaserGun {
    fn new() -> Self {
        Self {
            state: ItemState::default(),
            charge: 100,
            max_charge: 100,
        }
    }

    fn draw(&mut self) {
        let Some(new_state) = self.state.process_event(ItemEvent::Draw) else {
            return;
        };

        println!("Laser gun drawn (charge: {}%)", self.charge);
        self.state = new_state;
    }

    fn holster(&mut self) {
        let Some(new_state) = self.state.process_event(ItemEvent::Holster) else {
            return;
        };

        println!("Laser gun holstered");
        self.state = new_state;
    }

    fn fire(&mut self) {
        let Some(new_state) = self.state.process_event(ItemEvent::Fire) else {
            return;
        };

        if self.charge < 20 {
            println!("Insufficient charge to fire");
            return;
        }

        self.charge -= 20;
        println!("Laser gun fires! (charge: {}%)", self.charge);
        self.state = new_state;
    }

    fn cooldown_complete(&mut self) {
        let Some(new_state) = self.state.process_event(ItemEvent::CooldownComplete) else {
            return;
        };

        println!("Weapon cooling complete, ready to fire");
        self.state = new_state;
    }

    fn recharge(&mut self) {
        if self.charge < self.max_charge {
            self.charge = (self.charge + 10).min(self.max_charge);
            println!("Laser gun recharging... (charge: {}%)", self.charge);
        }
    }
}

fn main() {
    let mut player = Player::new();

    println!("═══ Movement Test ═══");
    player.start_walking();
    player.update_position(1.0);
    player.stop_walking();
    println!();

    println!("═══ Item Pickup ═══");
    player.pick_up_item();
    println!();

    println!("═══ Weapon Usage ═══");
    player.draw_weapon();
    player.fire_weapon();
    player.weapon_cooldown_complete();
    player.fire_weapon();
    player.weapon_cooldown_complete();
    println!();

    println!("═══ Movement with Weapon ═══");
    player.start_running();
    player.update_position(0.5);
    println!();

    println!("═══ Combat While Moving ═══");
    player.fire_weapon();
    player.weapon_cooldown_complete();
    player.update_position(0.5);
    println!();

    println!("═══ Recharge ═══");
    player.recharge_weapon();
    player.recharge_weapon();
    player.recharge_weapon();
    println!();

    println!("═══ Stop Running ═══");
    player.stop_running();
    println!();

    println!("═══ Holster and Drop ═══");
    player.holster_weapon();
    player.drop_item();
    println!();

    println!("═══ Final State ═══");
    println!("Player state: {:?}", player.state);
    println!(
        "Player position: ({:.1}, {:.1})",
        player.position.0, player.position.1
    );
    println!("Has item: {}", player.item.is_some());
}
