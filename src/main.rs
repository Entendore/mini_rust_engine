use macroquad::prelude::*;
use macroquad::audio::*;
use rand::Rng;
use std::fs;

// --- ENGINE CORE TYPES ---
type Vector2 = macroquad::math::Vec2;

#[derive(Clone, PartialEq, Debug)]
pub enum Tag { Player, Enemy, FlyingEnemy, Platform, Coin, MovingPlatform, Projectile, PowerUp, Key, Door, Checkpoint, Hazard, Bouncer, BlockPickup, PlayerBlock, Crate, Lever, HealthPickup, VerticalPlatform, Portal, Breakable }

#[derive(Clone, PartialEq, Debug)]
pub enum AIState { Patrol, Chase, Attack, Flee }

fn sample_mdp(choices: &[(AIState, f32)]) -> AIState {
    let total: f32 = choices.iter().map(|(_, p)| p).sum();
    let mut rng = rand::thread_rng();
    let mut r = rng.gen_range(0.0..total);
    for &(state, prob) in choices {
        r -= prob;
        if r <= 0.0 { return state; }
    }
    choices.last().unwrap().0.clone()
}

pub struct Entity {
    pub position: Vector2, pub size: Vector2, pub velocity: Vector2, pub color: Color,
    pub tag: Option<Tag>, pub active: bool, pub is_static: bool, pub is_grounded: bool, pub direction: f32,
    pub start_pos: Vector2, pub move_range: f32, pub move_speed: f32,
    pub ai_state: AIState, pub ai_decision_timer: f32, pub vision_range: f32,
    pub frames: Vec<Texture2D>, pub current_frame: usize, pub frame_timer: f32,
    pub lifespan: Option<f32>,
    pub max_jumps: i32, pub current_jumps: i32,
    pub is_activated: bool,
    pub touching_wall: i8,
    pub pushable: bool,
    pub dash_timer: f32, pub dash_cooldown: f32,
    pub health: i32,
    pub portal_cooldown: f32,
}

impl Entity {
    pub fn new(x: f32, y: f32, w: f32, h: f32, color: Color) -> Self {
        Self {
            position: Vector2::new(x, y), size: Vector2::new(w, h), velocity: Vector2::ZERO,
            color, tag: None, active: true, is_static: false, is_grounded: false, direction: 1.0,
            start_pos: Vector2::ZERO, move_range: 0.0, move_speed: 0.0,
            ai_state: AIState::Patrol, ai_decision_timer: 0.0, vision_range: 150.0,
            frames: Vec::new(), current_frame: 0, frame_timer: 0.0, lifespan: None,
            max_jumps: 1, current_jumps: 0, is_activated: false, touching_wall: 0, pushable: false,
            dash_timer: 0.0, dash_cooldown: 0.0,
            health: 1, portal_cooldown: 0.0,
        }
    }
    pub fn with_tag(mut self, tag: Tag) -> Self { self.tag = Some(tag); self }
    pub fn make_static(mut self) -> Self { self.is_static = true; self }
    pub fn make_moving(mut self, range: f32, speed: f32) -> Self { self.start_pos = self.position; self.move_range = range; self.move_speed = speed; self }
    pub fn make_pushable(mut self) -> Self { self.pushable = true; self }
    pub fn bounds(&self) -> Rect { Rect::new(self.position.x, self.position.y, self.size.x, self.size.y) }
}

// --- ASSET MANAGER ---
pub struct Assets {
    player_frames: Vec<Texture2D>, enemy_frames: Vec<Texture2D>,
    platform_tex: Texture2D, coin_tex: Texture2D, moving_tex: Texture2D, v_move_tex: Texture2D, projectile_tex: Texture2D, powerup_tex: Texture2D,
    key_tex: Texture2D, door_tex: Texture2D, checkpoint_tex: Texture2D, flying_enemy_tex: Texture2D,
    hazard_tex: Texture2D, bouncer_tex: Texture2D, block_pickup_tex: Texture2D, crate_tex: Texture2D, lever_tex: Texture2D, health_tex: Texture2D,
    portal_tex: Texture2D, breakable_tex: Texture2D, heart_full_tex: Texture2D, heart_empty_tex: Texture2D,
    jump_sound: Sound, coin_sound: Sound, stomp_sound: Sound, hurt_sound: Sound, shoot_sound: Sound, powerup_sound: Sound, unlock_sound: Sound, checkpoint_sound: Sound, bounce_sound: Sound, place_sound: Sound, lever_sound: Sound, heal_sound: Sound, music_sound: Sound, dash_sound: Sound, teleport_sound: Sound, break_sound: Sound,
}

impl Assets {
    fn gen_texture(c1: Color, c2: Color, size: usize) -> Texture2D {
        let mut bytes = Vec::with_capacity(size * size * 4);
        for y in 0..size {
            for x in 0..size {
                let color = if (x + y) % 2 == 0 { c1 } else { c2 };
                bytes.push((color.r * 255.0) as u8); bytes.push((color.g * 255.0) as u8);
                bytes.push((color.b * 255.0) as u8); bytes.push((color.a * 255.0) as u8);
            }
        }
        Texture2D::from_rgba8(size as u16, size as u16, &bytes)
    }
    
    fn gen_heart_tex(filled: bool) -> Texture2D {
        let color = if filled { RED } else { Color::new(0.3, 0.3, 0.3, 1.0) };
        let bg = Color::new(0.0, 0.0, 0.0, 0.0);
        let mut bytes = Vec::with_capacity(8 * 8 * 4);
        let pattern = [
            [0,1,1,0,0,1,1,0],
            [1,1,1,1,1,1,1,1],
            [1,1,1,1,1,1,1,1],
            [1,1,1,1,1,1,1,1],
            [0,1,1,1,1,1,1,0],
            [0,0,1,1,1,1,0,0],
            [0,0,0,1,1,0,0,0],
            [0,0,0,0,0,0,0,0],
        ];
        for y in 0..8 {
            for x in 0..8 {
                let c = if pattern[y][x] == 1 { color } else { bg };
                bytes.push((c.r * 255.0) as u8); bytes.push((c.g * 255.0) as u8);
                bytes.push((c.b * 255.0) as u8); bytes.push((c.a * 255.0) as u8);
            }
        }
        Texture2D::from_rgba8(8, 8, &bytes)
    }

    fn gen_anim_frames(c1: Color, c2: Color) -> Vec<Texture2D> {
        vec![Self::gen_texture(c1, c2, 8), Self::gen_texture(c2, c1, 8)]
    }
    fn generate_music_wav(notes: &[(f32, f32)], sample_rate: u32) -> Vec<u8> {
        let total_duration: f32 = notes.iter().map(|(_, d)| *d).sum();
        let num_samples = (total_duration * sample_rate as f32) as usize;
        let mut data = Vec::with_capacity(44 + num_samples * 2);
        data.extend_from_slice(b"RIFF"); data.extend_from_slice(&(36 + (num_samples * 2) as u32).to_le_bytes());
        data.extend_from_slice(b"WAVEfmt "); data.extend_from_slice(&16u32.to_le_bytes());
        data.extend_from_slice(&1u16.to_le_bytes()); data.extend_from_slice(&1u16.to_le_bytes());
        data.extend_from_slice(&sample_rate.to_le_bytes()); data.extend_from_slice(&(sample_rate * 2).to_le_bytes());
        data.extend_from_slice(&2u16.to_le_bytes()); data.extend_from_slice(&16u16.to_le_bytes());
        data.extend_from_slice(b"data"); data.extend_from_slice(&((num_samples * 2) as u32).to_le_bytes());
        for &(freq, dur) in notes {
            let note_samples = (dur * sample_rate as f32) as usize;
            for i in 0..note_samples {
                let t = i as f32 / sample_rate as f32;
                let sine = (t * freq * 2.0 * std::f32::consts::PI).sin();
                let env = (1.0 - (i as f32 / note_samples as f32)).max(0.0);
                let sample = (sine * env * 0.3 * i16::MAX as f32) as i16;
                data.extend_from_slice(&sample.to_le_bytes());
            }
        }
        data
    }
    pub async fn load() -> Self {
        let music_notes = [(261.63, 0.2), (329.63, 0.2), (392.00, 0.2), (523.25, 0.4), (392.00, 0.2), (329.63, 0.2), (261.63, 0.4)];
        Self {
            player_frames: Self::gen_anim_frames(BLUE, Color::from_hex(0x0000aa)),
            enemy_frames: Self::gen_anim_frames(RED, Color::from_hex(0x880000)),
            platform_tex: Self::gen_texture(DARKGRAY, GRAY, 16),
            coin_tex: Self::gen_texture(YELLOW, Color::from_hex(0x888800), 8),
            moving_tex: Self::gen_texture(PURPLE, Color::from_hex(0x550055), 8),
            v_move_tex: Self::gen_texture(Color::from_hex(0x6a0dad), Color::from_hex(0x330066), 8),
            projectile_tex: Self::gen_texture(ORANGE, Color::from_hex(0x884400), 8),
            powerup_tex: Self::gen_texture(GREEN, Color::from_hex(0x008800), 8),
            key_tex: Self::gen_texture(GOLD, Color::from_hex(0x886600), 8),
            door_tex: Self::gen_texture(BROWN, Color::from_hex(0x553300), 16),
            checkpoint_tex: Self::gen_texture(WHITE, BLACK, 8),
            flying_enemy_tex: Self::gen_texture(PINK, Color::from_hex(0x880066), 8),
            hazard_tex: Self::gen_texture(RED, Color::from_hex(0x330000), 8),
            bouncer_tex: Self::gen_texture(LIGHTGREEN, Color::from_hex(0x008844), 8),
            block_pickup_tex: Self::gen_texture(WHITE, GRAY, 8),
            crate_tex: Self::gen_texture(Color::from_hex(0x8B4513), Color::from_hex(0x5C2C0C), 16),
            lever_tex: Self::gen_texture(GRAY, DARKGRAY, 8),
            health_tex: Self::gen_texture(RED, PINK, 8),
            portal_tex: Self::gen_texture(CYAN, Color::from_hex(0x005577), 16),
            breakable_tex: Self::gen_texture(Color::from_hex(0x8B4513), Color::from_hex(0x222222), 16),
            heart_full_tex: Self::gen_heart_tex(true),
            heart_empty_tex: Self::gen_heart_tex(false),
            jump_sound: load_sound_from_bytes(&generate_wav(440.0, 0.15, 44100)).await.unwrap(),
            coin_sound: load_sound_from_bytes(&generate_wav(880.0, 0.1, 44100)).await.unwrap(),
            stomp_sound: load_sound_from_bytes(&generate_wav(150.0, 0.2, 44100)).await.unwrap(),
            hurt_sound: load_sound_from_bytes(&generate_wav(200.0, 0.3, 44100)).await.unwrap(),
            shoot_sound: load_sound_from_bytes(&generate_wav(660.0, 0.08, 44100)).await.unwrap(),
            powerup_sound: load_sound_from_bytes(&generate_wav(1320.0, 0.3, 44100)).await.unwrap(),
            unlock_sound: load_sound_from_bytes(&generate_wav(1100.0, 0.4, 44100)).await.unwrap(),
            checkpoint_sound: load_sound_from_bytes(&generate_wav(1600.0, 0.2, 44100)).await.unwrap(),
            bounce_sound: load_sound_from_bytes(&generate_wav(980.0, 0.15, 44100)).await.unwrap(),
            place_sound: load_sound_from_bytes(&generate_wav(300.0, 0.1, 44100)).await.unwrap(),
            lever_sound: load_sound_from_bytes(&generate_wav(500.0, 0.05, 44100)).await.unwrap(),
            heal_sound: load_sound_from_bytes(&generate_wav(1000.0, 0.2, 44100)).await.unwrap(),
            music_sound: load_sound_from_bytes(&Self::generate_music_wav(&music_notes, 44100)).await.unwrap(),
            dash_sound: load_sound_from_bytes(&generate_wav(1200.0, 0.1, 44100)).await.unwrap(),
            teleport_sound: load_sound_from_bytes(&generate_wav(1760.0, 0.2, 44100)).await.unwrap(),
            break_sound: load_sound_from_bytes(&generate_wav(250.0, 0.15, 44100)).await.unwrap(),
        }
    }
}

// --- INPUT MANAGER ---
pub struct Input;
impl Input {
    pub fn left(&self) -> bool { is_key_down(KeyCode::Left) || is_key_down(KeyCode::A) || is_gamepad_down(0, GamepadButton::Left) || is_gamepad_down(0, GamepadButton::Left2) }
    pub fn right(&self) -> bool { is_key_down(KeyCode::Right) || is_key_down(KeyCode::D) || is_gamepad_down(0, GamepadButton::Right) || is_gamepad_down(0, GamepadButton::Right2) }
    pub fn jump_pressed(&self) -> bool { is_key_pressed(KeyCode::Space) || is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::W) || is_gamepad_pressed(0, GamepadButton::A) || is_gamepad_pressed(0, GamepadButton::Up) }
    pub fn shoot_pressed(&self) -> bool { is_key_pressed(KeyCode::X) || is_key_pressed(KeyCode::J) || is_gamepad_pressed(0, GamepadButton::X) || is_gamepad_pressed(0, GamepadButton::RightTrigger) }
    pub fn place_block_pressed(&self) -> bool { is_key_pressed(KeyCode::B) || is_mouse_button_pressed(MouseButton::Right) || is_gamepad_pressed(0, GamepadButton::LeftTrigger) }
    pub fn interact_pressed(&self) -> bool { is_key_pressed(KeyCode::E) || is_gamepad_pressed(0, GamepadButton::Y) }
    pub fn dash_pressed(&self) -> bool { is_key_pressed(KeyCode::Shift) || is_gamepad_pressed(0, GamepadButton::RightTrigger2) || is_gamepad_pressed(0, GamepadButton::RightBumper) }
    pub fn enter(&self) -> bool { is_key_pressed(KeyCode::Enter) || is_gamepad_pressed(0, GamepadButton::Start) }
}

// --- PARTICLE SYSTEM ---
pub struct Particle { pos: Vector2, vel: Vector2, life: f32, max_life: f32, color: Color, size: f32 }
pub struct ParticleSystem { particles: Vec<Particle> }

impl ParticleSystem {
    pub fn new() -> Self { Self { particles: Vec::new() } }
    pub fn emit(&mut self, pos: Vector2, count: usize, color: Color) {
        let mut rng = rand::thread_rng();
        for _ in 0..count {
            let angle = rng.gen_range(0.0..std::f32::consts::TAU);
            let speed = rng.gen_range(50.0..200.0);
            self.particles.push(Particle {
                pos, vel: Vector2::new(angle.cos(), angle.sin()) * speed,
                life: rng.gen_range(0.3..0.8), max_life: 0.8, color, size: rng.gen_range(2.0..5.0),
            });
        }
    }
    pub fn update(&mut self, dt: f32) {
        for p in &mut self.particles { p.life -= dt; p.pos += p.vel * dt; p.vel.y += 300.0 * dt; p.vel *= 0.96; }
        self.particles.retain(|p| p.life > 0.0);
    }
    pub fn render(&self) {
        for p in &self.particles {
            let alpha = (p.life / p.max_life).clamp(0.0, 1.0);
            let mut c = p.color; c.a = alpha;
            draw_rectangle(p.pos.x, p.pos.y, p.size, p.size, c);
        }
    }
}

pub fn generate_wav(freq: f32, duration: f32, sample_rate: u32) -> Vec<u8> {
    let num_samples = (duration * sample_rate as f32) as usize;
    let mut data = Vec::with_capacity(44 + num_samples * 2);
    data.extend_from_slice(b"RIFF"); data.extend_from_slice(&(36 + (num_samples * 2) as u32).to_le_bytes());
    data.extend_from_slice(b"WAVEfmt "); data.extend_from_slice(&16u32.to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes()); data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&sample_rate.to_le_bytes()); data.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    data.extend_from_slice(&2u16.to_le_bytes()); data.extend_from_slice(&16u16.to_le_bytes());
    data.extend_from_slice(b"data"); data.extend_from_slice(&((num_samples * 2) as u32).to_le_bytes());
    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        let sine = (t * freq * 2.0 * std::f32::consts::PI).sin();
        let env = (1.0 - (i as f32 / num_samples as f32)).max(0.0);
        let sample = (sine * env * 0.5 * i16::MAX as f32) as i16;
        data.extend_from_slice(&sample.to_le_bytes());
    }
    data
}

#[derive(PartialEq)]
pub enum Scene { MainMenu, Settings, Playing, Paused, GameOver, Win, Editor }

// --- CUSTOM IMGUI SYSTEM ---
pub struct UI {
    mouse_pos: Vector2,
    active_slider: Option<usize>,
    active_dropdown: Option<usize>,
    active_text_input: Option<usize>,
    dragging_window: Option<usize>,
    drag_offset: Vector2,
    tooltip: Option<String>,
    theme: Theme,
}

#[derive(Clone)]
pub struct Theme { pub bg: Color, pub bg_hover: Color, pub border: Color, pub text: Color, pub accent: Color }

impl UI {
    pub fn new() -> Self { Self { mouse_pos: Vec2::ZERO, active_slider: None, active_dropdown: None, active_text_input: None, dragging_window: None, drag_offset: Vec2::ZERO, tooltip: None, theme: Theme::default() } }
    pub fn update_mouse(&mut self) {
        let (x, y) = mouse_position();
        self.mouse_pos = Vec2::new(x, y);
        self.tooltip = None; 
    }
    pub fn render_tooltip(&self) {
        if let Some(t) = &self.tooltip {
            let dims = measure_text(t, None, 16, 1.0);
            let x = self.mouse_pos.x + 15.0; let y = self.mouse_pos.y + 15.0;
            draw_rectangle(x, y, dims.width + 10.0, dims.height + 5.0, Color::new(0.0, 0.0, 0.0, 0.9));
            draw_text(t, x + 5.0, y + dims.height, 16.0, WHITE);
        }
    }
    pub fn panel(&self, x: f32, y: f32, w: f32, h: f32, title: &str) {
        draw_rectangle(x, y, w, h, Color::new(0.1, 0.1, 0.2, 0.9));
        draw_rectangle_lines(x, y, w, h, 2.0, self.theme.border);
        draw_text(title, x + 10.0, y + 25.0, 24.0, self.theme.accent);
    }
    pub fn draggable_panel(&mut self, id: usize, x: &mut f32, y: &mut f32, w: f32, h: f32, title: &str) {
        let title_h = 30.0;
        let title_rect = Rect::new(*x, *y, w, title_h);
        if is_mouse_button_pressed(MouseButton::Left) && title_rect.contains(self.mouse_pos) {
            self.dragging_window = Some(id);
            self.drag_offset = self.mouse_pos - Vec2::new(*x, *y);
        }
        if is_mouse_button_released(MouseButton::Left) { self.dragging_window = None; }
        if self.dragging_window == Some(id) {
            *x = self.mouse_pos.x - self.drag_offset.x;
            *y = self.mouse_pos.y - self.drag_offset.y;
        }
        draw_rectangle(*x, *y, w, h, Color::new(0.1, 0.1, 0.2, 0.95));
        draw_rectangle(*x, *y, w, title_h, self.theme.accent);
        draw_rectangle_lines(*x, *y, w, h, 2.0, self.theme.border);
        draw_text(title, *x + 10.0, *y + 20.0, 20.0, BLACK);
    }
    pub fn label(&self, x: f32, y: f32, text: &str, color: Color) { draw_text(text, x, y, 20.0, color); }
    pub fn button(&mut self, id: usize, x: f32, y: f32, w: f32, h: f32, label: &str, tooltip: Option<&str>) -> bool {
        let rect = Rect::new(x, y, w, h);
        let hovered = rect.contains(self.mouse_pos);
        let clicked = hovered && is_mouse_button_pressed(MouseButton::Left);
        let bg_color = if hovered { self.theme.bg_hover } else { self.theme.bg };
        draw_rectangle(x, y, w, h, bg_color);
        draw_rectangle_lines(x, y, w, h, 2.0, if hovered { self.theme.accent } else { self.theme.border });
        let text_dims = measure_text(label, None, 20, 1.0);
        draw_text(label, x + (w - text_dims.width) / 2.0, y + (h + text_dims.height) / 2.0 - 4.0, 20.0, self.theme.text);
        if hovered && tooltip.is_some() { self.tooltip = tooltip.map(|s| s.to_string()); }
        clicked
    }
    pub fn slider(&mut self, id: usize, x: f32, y: f32, w: f32, h: f32, value: f32, min: f32, max: f32, label: &str) -> f32 {
        let rect = Rect::new(x, y, w, h);
        let hovered = rect.contains(self.mouse_pos);
        if is_mouse_button_pressed(MouseButton::Left) && hovered { self.active_slider = Some(id); }
        if is_mouse_button_released(MouseButton::Left) { self.active_slider = None; }
        let mut new_val = value;
        if self.active_slider == Some(id) {
            let normalized = ((self.mouse_pos.x - x) / w).clamp(0.0, 1.0);
            new_val = min + normalized * (max - min);
        }
        draw_rectangle(x, y, w, h, Color::from_hex(0x1a1a2e));
        draw_rectangle_lines(x, y, w, h, 2.0, self.theme.border);
        let fill_w = ((new_val - min) / (max - min)) * w;
        draw_rectangle(x, y, fill_w, h, self.theme.accent);
        draw_text(label, x, y - 5.0, 20.0, self.theme.text);
        draw_text(&format!("{:.2}", new_val), x + w - 30.0, y - 5.0, 20.0, YELLOW);
        new_val
    }
    pub fn progress_bar(&self, x: f32, y: f32, w: f32, h: f32, value: f32, max: f32, color: Color) {
        draw_rectangle(x, y, w, h, DARKGRAY);
        let fill = (value / max) * w;
        draw_rectangle(x, y, fill, h, color);
        draw_rectangle_lines(x, y, w, h, 2.0, self.theme.border);
    }
    pub fn checkbox(&mut self, id: usize, x: f32, y: f32, label: &str, checked: bool) -> bool {
        let size = 20.0;
        let rect = Rect::new(x, y, size, size);
        let hovered = rect.contains(self.mouse_pos);
        let clicked = hovered && is_mouse_button_pressed(MouseButton::Left);
        draw_rectangle(x, y, size, size, if checked { self.theme.accent } else { DARKGRAY });
        draw_rectangle_lines(x, y, size, size, 2.0, if hovered { YELLOW } else { self.theme.border });
        draw_text(label, x + size + 10.0, y + 16.0, 20.0, self.theme.text);
        if clicked { !checked } else { checked }
    }
    pub fn cycle_button(&mut self, id: usize, x: f32, y: f32, w: f32, h: f32, label: &str, current_val: &str) -> bool {
        let rect = Rect::new(x, y, w, h);
        let hovered = rect.contains(self.mouse_pos);
        let clicked = hovered && is_mouse_button_pressed(MouseButton::Left);
        let bg_color = if hovered { self.theme.bg_hover } else { self.theme.bg };
        draw_rectangle(x, y, w, h, bg_color);
        draw_rectangle_lines(x, y, w, h, 2.0, if hovered { YELLOW } else { self.theme.border });
        draw_text(label, x + 5.0, y + 16.0, 16.0, self.theme.text);
        let val_dims = measure_text(current_val, None, 20, 1.0);
        draw_text(current_val, x + w - val_dims.width - 5.0, y + (h + val_dims.height) / 2.0 - 4.0, 20.0, YELLOW);
        clicked
    }
    pub fn dropdown(&mut self, id: usize, x: f32, y: f32, w: f32, h: f32, label: &str, options: &[&str], current_idx: usize) -> usize {
        let rect = Rect::new(x, y, w, h);
        let hovered = rect.contains(self.mouse_pos);
        if is_mouse_button_pressed(MouseButton::Left) {
            if hovered && self.active_dropdown.is_none() { self.active_dropdown = Some(id); }
            else if self.active_dropdown == Some(id) {
                for (i, _opt) in options.iter().enumerate() {
                    let opt_rect = Rect::new(x, y + (i as f32 + 1.0) * h, w, h);
                    if opt_rect.contains(self.mouse_pos) { self.active_dropdown = None; return i; }
                }
                self.active_dropdown = None;
            }
        }
        let bg_color = if hovered { self.theme.bg_hover } else { self.theme.bg };
        draw_rectangle(x, y, w, h, bg_color);
        draw_rectangle_lines(x, y, w, h, 2.0, self.theme.border);
        draw_text(label, x + 5.0, y + 16.0, 16.0, self.theme.text);
        draw_text(options[current_idx], x + 100.0, y + 16.0, 20.0, YELLOW);
        if self.active_dropdown == Some(id) {
            for (i, opt) in options.iter().enumerate() {
                let opt_y = y + (i as f32 + 1.0) * h;
                let opt_rect = Rect::new(x, opt_y, w, h);
                let opt_hovered = opt_rect.contains(self.mouse_pos);
                draw_rectangle(x, opt_y, w, h, if opt_hovered { self.theme.bg_hover } else { self.theme.bg });
                draw_rectangle_lines(x, opt_y, w, h, 2.0, self.theme.border);
                draw_text(opt, x + 5.0, opt_y + 16.0, 20.0, self.theme.text);
            }
        }
        current_idx
    }
    pub fn tab_bar(&mut self, id: usize, x: f32, y: f32, w: f32, h: f32, tabs: &[&str], current_tab: usize) -> usize {
        let tab_w = w / tabs.len() as f32;
        for (i, tab) in tabs.iter().enumerate() {
            let tx = x + i as f32 * tab_w;
            let rect = Rect::new(tx, y, tab_w, h);
            let hovered = rect.contains(self.mouse_pos);
            let active = i == current_tab;
            if hovered && is_mouse_button_pressed(MouseButton::Left) { return i; }
            let bg_color = if active { self.theme.accent } else if hovered { self.theme.bg_hover } else { self.theme.bg };
            draw_rectangle(tx, y, tab_w, h, bg_color);
            draw_rectangle_lines(tx, y, tab_w, h, 2.0, self.theme.border);
            let text_dims = measure_text(tab, None, 18, 1.0);
            draw_text(tab, tx + (tab_w - text_dims.width) / 2.0, y + (h + text_dims.height) / 2.0 - 4.0, 18.0, if active { BLACK } else { self.theme.text });
        }
        current_tab
    }
    pub fn text_input(&mut self, id: usize, x: f32, y: f32, w: f32, h: f32, label: &str, buffer: &mut String) -> bool {
        let rect = Rect::new(x, y, w, h);
        let hovered = rect.contains(self.mouse_pos);
        if is_mouse_button_pressed(MouseButton::Left) { self.active_text_input = if hovered { Some(id) } else { None }; }
        let mut enter_pressed = false;
        if self.active_text_input == Some(id) {
            while let Some(c) = get_char_pressed() { if c.is_ascii_graphic() || c == ' ' { buffer.push(c); } }
            if is_key_pressed(KeyCode::Backspace) { buffer.pop(); }
            if is_key_pressed(KeyCode::Enter) { self.active_text_input = None; enter_pressed = true; }
        }
        let bg_color = if self.active_text_input == Some(id) { self.theme.bg_hover } else { self.theme.bg };
        draw_rectangle(x, y, w, h, bg_color);
        draw_rectangle_lines(x, y, w, h, 2.0, self.theme.border);
        draw_text(label, x, y - 5.0, 20.0, self.theme.text);
        draw_text(buffer, x + 5.0, y + h - 6.0, 20.0, self.theme.text);
        if self.active_text_input == Some(id) && (get_time() % 1.0) < 0.5 {
            let text_dims = measure_text(buffer, None, 20, 1.0);
            draw_line(x + 5.0 + text_dims.width + 2.0, y + 5.0, x + 5.0 + text_dims.width + 2.0, y + h - 5.0, 1.0, self.theme.text);
        }
        enter_pressed
    }
}

impl Theme {
    pub fn default() -> Self { Self { bg: Color::from_hex(0x2a2a3e), bg_hover: Color::from_hex(0x4a4a6e), border: WHITE, text: WHITE, accent: BLUE } }
    pub fn dark() -> Self { Self { bg: Color::from_hex(0x111111), bg_hover: Color::from_hex(0x333333), border: LIGHTGRAY, text: WHITE, accent: Color::from_hex(0x0055ff) } }
    pub fn contrast() -> Self { Self { bg: BLACK, bg_hover: WHITE, border: WHITE, text: WHITE, accent: YELLOW } }
}

// --- THE ENGINE ---
pub struct Engine {
    pub entities: Vec<Entity>,
    pub scene: Scene,
    pub camera: Camera2D,
    pub shake_time: f32,
    pub score: i32,
    pub health: i32,
    pub max_health: i32,
    pub keys_held: i32,
    pub blocks_held: i32,
    pub invincibility_timer: f32,
    pub particles: ParticleSystem,
    pub input: Input,
    pub stars: Vec<(f32, f32, f32)>,
    pub assets: Assets,
    pub ui: UI,
    
    pub show_debug: bool,
    pub sfx_volume: f32,
    pub music_volume: f32,
    pub time_scale: f32,
    pub theme_idx: usize,
    pub darkness_level: f32,
    pub camera_bounds: bool,
    
    pub coyote_timer: f32,
    pub jump_buffer_timer: f32,
    pub editor_selected_tile: Tag,
    pub editor_grid_size: f32,
    pub editor_grid_sizes: [f32; 3],
    pub editor_grid_index: usize,
    pub editor_tab: usize,
    pub editor_filename: String,
    
    pub transition_alpha: f32,
    pub transition_target: Option<Scene>,
    
    pub player_spawn: Vector2,
    pub has_checkpoint: bool,
    pub music_playing: bool,
    
    pub help_window_pos: Vector2,
    
    pub combo_timer: f32,
    pub combo_count: i32,
}

impl Engine {
    pub async fn new() -> Self {
        let assets = Assets::load().await;
        let mut rng = rand::thread_rng();
        let stars = (0..50).map(|_| {
            (rng.gen_range(0.0..2000.0), rng.gen_range(0.0..1000.0), rng.gen_range(0.2..0.8))
        }).collect();

        let mut engine = Self {
            entities: Vec::new(), scene: Scene::MainMenu, camera: Camera2D::default(),
            shake_time: 0.0, score: 0, health: 5, max_health: 5, keys_held: 0, blocks_held: 0, invincibility_timer: 0.0,
            particles: ParticleSystem::new(), input: Input, stars, assets, ui: UI::new(),
            show_debug: false, sfx_volume: 0.5, music_volume: 0.2, time_scale: 1.0, theme_idx: 0, darkness_level: 0.0, camera_bounds: true,
            coyote_timer: 0.0, jump_buffer_timer: 0.0,
            editor_selected_tile: Tag::Platform, editor_grid_size: 40.0, editor_grid_sizes: [20.0, 40.0, 80.0], editor_grid_index: 1, editor_tab: 0, editor_filename: "level.txt".to_string(),
            transition_alpha: 0.0, transition_target: None,
            player_spawn: Vec2::ZERO, has_checkpoint: false, music_playing: false,
            help_window_pos: Vec2::new(50.0, 250.0),
            combo_timer: 0.0, combo_count: 0,
        };
        engine.load_settings();
        engine.ui.theme = match engine.theme_idx { 1 => Theme::dark(), 2 => Theme::contrast(), _ => Theme::default() };
        engine
    }

    pub fn save_settings(&self) {
        let data = format!(
            "sfx={}\nmusic={}\ntheme={}\ndebug={}\ndark={}\ncam_bounds={}\n",
            self.sfx_volume, self.music_volume, self.theme_idx, self.show_debug, self.darkness_level, self.camera_bounds
        );
        let _ = fs::write("settings.txt", data);
    }
    pub fn load_settings(&mut self) {
        if let Ok(data) = fs::read_to_string("settings.txt") {
            for line in data.lines() {
                let parts: Vec<&str> = line.split('=').collect();
                if parts.len() == 2 {
                    match parts[0] {
                        "sfx" => self.sfx_volume = parts[1].parse().unwrap_or(0.5),
                        "music" => self.music_volume = parts[1].parse().unwrap_or(0.2),
                        "theme" => self.theme_idx = parts[1].parse().unwrap_or(0),
                        "debug" => self.show_debug = parts[1] == "true",
                        "dark" => self.darkness_level = parts[1].parse().unwrap_or(0.0),
                        "cam_bounds" => self.camera_bounds = parts[1] == "true",
                        _ => {}
                    }
                }
            }
        }
    }

    pub fn request_scene(&mut self, target: Scene) { self.transition_target = Some(target); }

    pub fn load_default_level(&mut self) {
        self.entities.clear();
        self.health = self.max_health;
        self.keys_held = 0; self.blocks_held = 0; self.invincibility_timer = 0.0; self.has_checkpoint = false;
        self.combo_count = 0; self.combo_timer = 0.0;
        
        let level = "
.........................................
....T...............F....................
....................C....................
.................######...B..............
......E.................K.....O..........
....######............~~.................
..*..........V..........Q.........E......
............G....######...........D......
.P............X...........^^^^......T....
#########################################
";
        self.parse_level_string(level);
    }

    fn parse_level_string(&mut self, level: &str) {
        let tile = self.editor_grid_size;
        for (y, line) in level.lines().enumerate() {
            for (x, ch) in line.chars().enumerate() {
                let px = x as f32 * tile; let py = y as f32 * tile;
                match ch {
                    '#' => { let mut e = Entity::new(px, py, tile, tile, DARKGRAY).with_tag(Tag::Platform).make_static(); e.frames.push(self.assets.platform_tex); self.entities.push(e); },
                    'P' => { let mut e = Entity::new(px, py, 30.0, 40.0, BLUE).with_tag(Tag::Player); e.frames = self.assets.player_frames.clone(); self.entities.push(e); self.player_spawn = Vec2::new(px, py); },
                    'E' => { let mut e = Entity::new(px, py, 30.0, 30.0, RED).with_tag(Tag::Enemy); e.frames = self.assets.enemy_frames.clone(); self.entities.push(e); },
                    'F' => { let mut e = Entity::new(px, py, 30.0, 30.0, PINK).with_tag(Tag::FlyingEnemy); e.frames.push(self.assets.flying_enemy_tex); self.entities.push(e); },
                    'C' => { let mut e = Entity::new(px, py, 20.0, 20.0, YELLOW).with_tag(Tag::Coin); e.frames.push(self.assets.coin_tex); self.entities.push(e); },
                    '~' => { let mut e = Entity::new(px, py, 80.0, 20.0, PURPLE).with_tag(Tag::MovingPlatform).make_moving(150.0, 50.0); e.frames.push(self.assets.moving_tex); self.entities.push(e); },
                    'V' => { let mut e = Entity::new(px, py, 80.0, 20.0, PURPLE).with_tag(Tag::VerticalPlatform).make_moving(150.0, 50.0); e.frames.push(self.assets.v_move_tex); self.entities.push(e); },
                    'G' => { let mut e = Entity::new(px, py, 25.0, 25.0, GREEN).with_tag(Tag::PowerUp); e.frames.push(self.assets.powerup_tex); self.entities.push(e); },
                    'K' => { let mut e = Entity::new(px, py, 25.0, 25.0, GOLD).with_tag(Tag::Key); e.frames.push(self.assets.key_tex); self.entities.push(e); },
                    'D' => { let mut e = Entity::new(px, py, 40.0, 80.0, BROWN).with_tag(Tag::Door).make_static(); e.frames.push(self.assets.door_tex); self.entities.push(e); },
                    'X' => { let mut e = Entity::new(px, py, 20.0, 40.0, WHITE).with_tag(Tag::Checkpoint).make_static(); e.frames.push(self.assets.checkpoint_tex); self.entities.push(e); },
                    '*' => { let mut e = Entity::new(px, py, tile, tile, RED).with_tag(Tag::Hazard).make_static(); e.frames.push(self.assets.hazard_tex); self.entities.push(e); },
                    '^' => { let mut e = Entity::new(px, py, tile, 20.0, LIGHTGREEN).with_tag(Tag::Bouncer).make_static(); e.frames.push(self.assets.bouncer_tex); self.entities.push(e); },
                    'B' => { let mut e = Entity::new(px, py, 25.0, 25.0, WHITE).with_tag(Tag::BlockPickup); e.frames.push(self.assets.block_pickup_tex); self.entities.push(e); },
                    'O' => { let mut e = Entity::new(px, py, tile, tile, BROWN).with_tag(Tag::Crate).make_pushable(); e.frames.push(self.assets.crate_tex); self.entities.push(e); },
                    'L' => { let mut e = Entity::new(px, py, 30.0, 30.0, GRAY).with_tag(Tag::Lever).make_static(); e.frames.push(self.assets.lever_tex); self.entities.push(e); },
                    'H' => { let mut e = Entity::new(px, py, 20.0, 20.0, RED).with_tag(Tag::HealthPickup); e.frames.push(self.assets.health_tex); self.entities.push(e); },
                    'T' => { let mut e = Entity::new(px, py, 40.0, 80.0, CYAN).with_tag(Tag::Portal).make_static(); e.frames.push(self.assets.portal_tex); self.entities.push(e); },
                    'Q' => { let mut e = Entity::new(px, py, tile, tile, BROWN).with_tag(Tag::Breakable).make_static(); e.frames.push(self.assets.breakable_tex); self.entities.push(e); },
                    _ => {}
                }
            }
        }
    }

    pub fn save_level_to_file(&self, filename: &str) {
        let mut level_str = String::new();
        let tile = self.editor_grid_size;
        let max_x = self.entities.iter().map(|e| e.position.x).fold(0.0, f32::max) / tile;
        let max_y = self.entities.iter().map(|e| e.position.y).fold(0.0, f32::max) / tile;
        for y in 0..=(max_y as usize) {
            for x in 0..=(max_x as usize) {
                let px = x as f32 * tile; let py = y as f32 * tile;
                let mut ch = '.';
                for ent in &self.entities {
                    if (ent.position.x - px).abs() < 1.0 && (ent.position.y - py).abs() < 1.0 {
                        ch = match ent.tag {
                            Some(Tag::Platform) => '#', Some(Tag::Player) => 'P', Some(Tag::Enemy) => 'E',
                            Some(Tag::FlyingEnemy) => 'F', Some(Tag::Coin) => 'C', Some(Tag::MovingPlatform) => '~',
                            Some(Tag::VerticalPlatform) => 'V', Some(Tag::PowerUp) => 'G', Some(Tag::Key) => 'K', 
                            Some(Tag::Door) => 'D', Some(Tag::Checkpoint) => 'X', Some(Tag::Hazard) => '*', 
                            Some(Tag::Bouncer) => '^', Some(Tag::BlockPickup) => 'B', Some(Tag::PlayerBlock) => '#', 
                            Some(Tag::Crate) => 'O', Some(Tag::Lever) => 'L', Some(Tag::HealthPickup) => 'H',
                            Some(Tag::Portal) => 'T', Some(Tag::Breakable) => 'Q',
                            _ => '.'
                        };
                        break;
                    }
                }
                level_str.push(ch);
            }
            level_str.push('\n');
        }
        let _ = fs::write(filename, level_str);
    }

    pub fn load_level_from_file(&mut self, filename: &str) {
        match fs::read_to_string(filename) {
            Ok(content) => {
                self.entities.clear();
                self.parse_level_string(&content);
                self.health = self.max_health;
                self.keys_held = 0; self.blocks_held = 0; self.invincibility_timer = 0.0; self.has_checkpoint = false;
            },
            Err(_) => println!("No {} found. Loading default.", filename),
        }
    }

    pub fn shake(&mut self, duration: f32) { if self.shake_time < duration { self.shake_time = duration; } }
    
    pub fn add_score(&mut self, base_score: i32) {
        self.combo_timer = 2.0; // 2 seconds to chain
        self.combo_count += 1;
        self.score += base_score * self.combo_count;
    }

    pub async fn run(&mut self) {
        loop {
            let real_dt = get_frame_time(); 
            let game_dt = real_dt * self.time_scale; 
            self.ui.update_mouse();

            if let Some(target) = self.transition_target {
                self.transition_alpha += real_dt * 3.0;
                if self.transition_alpha >= 1.0 {
                    if self.scene == Scene::Playing && target != Scene::Playing {
                        stop_sound(self.assets.music_sound);
                        self.music_playing = false;
                    }
                    self.scene = target;
                    self.transition_target = None;
                }
            } else if self.transition_alpha > 0.0 {
                self.transition_alpha -= real_dt * 3.0;
                self.transition_alpha = self.transition_alpha.max(0.0);
            }

            if self.transition_alpha < 0.99 {
                match self.scene {
                    Scene::MainMenu => self.update_menu(),
                    Scene::Settings => self.update_settings(),
                    Scene::Playing => {
                        if !self.music_playing {
                            play_sound(self.assets.music_sound, PlaySoundParams { looped: true, volume: self.music_volume });
                            self.music_playing = true;
                        }
                        if is_key_pressed(KeyCode::Escape) { self.request_scene(Scene::Paused); }
                        else { self.update_playing(game_dt); }
                    },
                    Scene::Paused => self.update_paused(),
                    Scene::GameOver | Scene::Win => self.update_endgame(),
                    Scene::Editor => self.update_editor(),
                }
            }
            self.render().await;
        }
    }

    fn update_menu(&mut self) {
        let cx = screen_width() / 2.0;
        if self.ui.button(1, cx - 100.0, 200.0, 200.0, 50.0, "Play Default", Some("Start the built-in level")) {
            self.load_default_level(); self.request_scene(Scene::Playing);
        }
        if self.ui.button(2, cx - 100.0, 270.0, 200.0, 50.0, "Load Custom", Some("Load level.txt from disk")) {
            self.load_level_from_file("level.txt"); self.request_scene(Scene::Playing);
        }
        if self.ui.button(3, cx - 100.0, 340.0, 200.0, 50.0, "Level Editor", Some("Create your own level")) {
            self.entities.clear(); self.request_scene(Scene::Editor);
        }
        if self.ui.button(4, cx - 100.0, 410.0, 200.0, 50.0, "Settings", Some("Adjust volume, themes, and debug")) {
            self.request_scene(Scene::Settings);
        }
        let mut hx = self.help_window_pos.x; let mut hy = self.help_window_pos.y;
        self.ui.draggable_panel(1, &mut hx, &mut hy, 300.0, 160.0, "How to Play");
        self.help_window_pos = Vec2::new(hx, hy);
        self.ui.label(hx + 10.0, hy + 45.0, "WASD/Arrows: Move", WHITE);
        self.ui.label(hx + 10.0, hy + 70.0, "Space: Jump (Double jump unlocked later)", WHITE);
        self.ui.label(hx + 10.0, hy + 95.0, "Shift: Dash (Breaks cracked blocks)", WHITE);
        self.ui.label(hx + 10.0, hy + 120.0, "X/J: Shoot, E: Interact, B: Place Block", WHITE);
    }

    fn update_settings(&mut self) {
        let cx = screen_width() / 2.0;
        self.sfx_volume = self.ui.slider(1, cx - 150.0, 120.0, 300.0, 30.0, self.sfx_volume, 0.0, 1.0, "SFX Volume");
        self.music_volume = self.ui.slider(2, cx - 150.0, 180.0, 300.0, 30.0, self.music_volume, 0.0, 1.0, "Music Volume");
        self.time_scale = self.ui.slider(3, cx - 150.0, 240.0, 300.0, 30.0, self.time_scale, 0.1, 2.0, "Game Speed");
        self.darkness_level = self.ui.slider(4, cx - 150.0, 300.0, 300.0, 30.0, self.darkness_level, 0.0, 0.95, "Darkness Level");
        self.show_debug = self.ui.checkbox(1, cx - 100.0, 350.0, "Show AI Debug", self.show_debug);
        self.camera_bounds = self.ui.checkbox(2, cx - 100.0, 380.0, "Camera Bounds", self.camera_bounds);
        let themes = ["Default", "Dark", "Contrast"];
        self.theme_idx = self.ui.dropdown(1, cx - 100.0, 420.0, 200.0, 30.0, "UI Theme:", &themes, self.theme_idx);
        self.ui.theme = match self.theme_idx { 0 => Theme::default(), 1 => Theme::dark(), 2 => Theme::contrast(), _ => Theme::default() };
        if self.ui.button(5, cx - 100.0, 470.0, 200.0, 50.0, "Back", None) {
            self.save_settings();
            self.request_scene(Scene::MainMenu);
        }
    }

    fn update_paused(&mut self) {
        let cx = screen_width() / 2.0;
        if is_key_pressed(KeyCode::Escape) { self.request_scene(Scene::Playing); }
        if self.ui.button(1, cx - 100.0, 250.0, 200.0, 50.0, "Resume", None) { self.request_scene(Scene::Playing); }
        if self.ui.button(2, cx - 100.0, 320.0, 200.0, 50.0, "Main Menu", None) { self.request_scene(Scene::MainMenu); }
    }

    fn update_endgame(&mut self) { if self.input.enter() { self.request_scene(Scene::MainMenu); } }

    fn update_editor(&mut self) {
        let (mx, my) = mouse_position();
        let tile = self.editor_grid_size;
        let grid_x = (mx / tile).floor() * tile;
        let grid_y = (my / tile).floor() * tile;

        if is_mouse_button_down(MouseButton::Left) && mx > 150.0 {
            let mut exists = false;
            for ent in &self.entities {
                if ent.position.x == grid_x && ent.position.y == grid_y { exists = true; break; }
            }
            if !exists {
                let mut e = Entity::new(grid_x, grid_y, tile, tile, WHITE);
                match self.editor_selected_tile {
                    Tag::Platform => { e.frames.push(self.assets.platform_tex); e = e.with_tag(Tag::Platform).make_static(); }
                    Tag::Player => { e.size = Vec2::new(30.0, 40.0); e.color = BLUE; e.frames = self.assets.player_frames.clone(); e = e.with_tag(Tag::Player); }
                    Tag::Enemy => { e.size = Vec2::new(30.0, 30.0); e.color = RED; e.frames = self.assets.enemy_frames.clone(); e = e.with_tag(Tag::Enemy); }
                    Tag::FlyingEnemy => { e.size = Vec2::new(30.0, 30.0); e.color = PINK; e.frames.push(self.assets.flying_enemy_tex); e = e.with_tag(Tag::FlyingEnemy); }
                    Tag::Coin => { e.size = Vec2::new(20.0, 20.0); e.color = YELLOW; e.frames.push(self.assets.coin_tex); e = e.with_tag(Tag::Coin); }
                    Tag::MovingPlatform => { e.size = Vec2::new(80.0, 20.0); e.color = PURPLE; e.frames.push(self.assets.moving_tex); e = e.with_tag(Tag::MovingPlatform).make_moving(150.0, 50.0); }
                    Tag::VerticalPlatform => { e.size = Vec2::new(80.0, 20.0); e.color = PURPLE; e.frames.push(self.assets.v_move_tex); e = e.with_tag(Tag::VerticalPlatform).make_moving(150.0, 50.0); }
                    Tag::PowerUp => { e.size = Vec2::new(25.0, 25.0); e.color = GREEN; e.frames.push(self.assets.powerup_tex); e = e.with_tag(Tag::PowerUp); }
                    Tag::Key => { e.size = Vec2::new(25.0, 25.0); e.color = GOLD; e.frames.push(self.assets.key_tex); e = e.with_tag(Tag::Key); }
                    Tag::Door => { e.size = Vec2::new(40.0, 80.0); e.color = BROWN; e.frames.push(self.assets.door_tex); e = e.with_tag(Tag::Door).make_static(); }
                    Tag::Checkpoint => { e.size = Vec2::new(20.0, 40.0); e.color = WHITE; e.frames.push(self.assets.checkpoint_tex); e = e.with_tag(Tag::Checkpoint).make_static(); }
                    Tag::Hazard => { e.frames.push(self.assets.hazard_tex); e = e.with_tag(Tag::Hazard).make_static(); }
                    Tag::Bouncer => { e.size = Vec2::new(tile, 20.0); e.frames.push(self.assets.bouncer_tex); e = e.with_tag(Tag::Bouncer).make_static(); }
                    Tag::BlockPickup => { e.size = Vec2::new(25.0, 25.0); e.frames.push(self.assets.block_pickup_tex); e = e.with_tag(Tag::BlockPickup); }
                    Tag::Crate => { e.frames.push(self.assets.crate_tex); e = e.with_tag(Tag::Crate).make_pushable(); }
                    Tag::Lever => { e.size = Vec2::new(30.0, 30.0); e.frames.push(self.assets.lever_tex); e = e.with_tag(Tag::Lever).make_static(); }
                    Tag::HealthPickup => { e.size = Vec2::new(20.0, 20.0); e.frames.push(self.assets.health_tex); e = e.with_tag(Tag::HealthPickup); }
                    Tag::Portal => { e.size = Vec2::new(40.0, 80.0); e.color = CYAN; e.frames.push(self.assets.portal_tex); e = e.with_tag(Tag::Portal).make_static(); }
                    Tag::Breakable => { e.frames.push(self.assets.breakable_tex); e = e.with_tag(Tag::Breakable).make_static(); }
                    _ => {}
                }
                self.entities.push(e);
            }
        }

        if is_mouse_button_down(MouseButton::Right) {
            self.entities.retain(|e| !(e.position.x == grid_x && e.position.y == grid_y));
        }

        self.ui.panel(10.0, 10.0, 130.0, 630.0, "Editor");
        let tabs = ["World", "Entity", "Item"];
        self.editor_tab = self.ui.tab_bar(1, 20.0, 45.0, 110.0, 30.0, &tabs, self.editor_tab);
        
        let buttons: &[(Tag, &str, &str)] = match self.editor_tab {
            0 => &[
                (Tag::Platform, "Platform", "Solid ground"),
                (Tag::MovingPlatform, "Move H", "Moves Left/Right"),
                (Tag::VerticalPlatform, "Move V", "Moves Up/Down"),
                (Tag::Door, "Door", "Toggled by lever"),
                (Tag::Lever, "Lever", "Toggles doors"),
                (Tag::Checkpoint, "Checkpoint", "Respawn"),
                (Tag::Hazard, "Hazard", "Spikes/Lava"),
                (Tag::Bouncer, "Bouncer", "Spring"),
                (Tag::Crate, "Crate", "Pushable"),
                (Tag::Breakable, "Breakable", "Cracked wall"),
                (Tag::Portal, "Portal", "Teleporter"),
            ],
            1 => &[
                (Tag::Player, "Player", "Spawn point"),
                (Tag::Enemy, "Enemy", "Ground AI"),
                (Tag::FlyingEnemy, "Fly Enemy", "Homing AI"),
            ],
            _ => &[
                (Tag::Coin, "Coin", "Score"),
                (Tag::PowerUp, "DoubleJump", "Upgrade"),
                (Tag::Key, "Key", "Opens door"),
                (Tag::BlockPickup, "Block Item", "Gives blocks"),
                (Tag::HealthPickup, "Health", "Restores 1 HP"),
            ],
        };

        for (i, (tag, label, tip)) in buttons.iter().enumerate() {
            let y = 85.0 + (i as f32 * 35.0);
            if self.ui.button(i + 10, 20.0, y, 110.0, 30.0, label, Some(tip)) {
                self.editor_selected_tile = tag.clone();
            }
        }

        let mut y_offset = 85.0 + (buttons.len() as f32 * 35.0) + 10.0;
        if self.ui.cycle_button(30, 20.0, y_offset, 110.0, 30.0, "Grid Size", &format!("{}px", self.editor_grid_size)) {
            self.editor_grid_index = (self.editor_grid_index + 1) % self.editor_grid_sizes.len();
            self.editor_grid_size = self.editor_grid_sizes[self.editor_grid_index];
        }
        y_offset += 40.0;

        self.ui.text_input(1, 20.0, y_offset, 110.0, 30.0, "Filename:", &mut self.editor_filename);
        y_offset += 50.0;

        if self.ui.button(20, 20.0, y_offset, 110.0, 30.0, "Save Level", None) { 
            let fname = if self.editor_filename.is_empty() { "level.txt".to_string() } else { self.editor_filename.clone() };
            self.save_level_to_file(&fname); 
        }
        y_offset += 40.0;
        if self.ui.button(21, 20.0, y_offset, 110.0, 30.0, "Test Play", None) {
            if self.entities.iter().any(|e| e.tag == Some(Tag::Player)) {
                self.health = self.max_health; self.keys_held = 0; self.blocks_held = 0; self.invincibility_timer = 0.0; self.has_checkpoint = false;
                self.request_scene(Scene::Playing);
            }
        }
        y_offset += 40.0;
        if self.ui.button(22, 20.0, y_offset, 110.0, 30.0, "Main Menu", None) { self.request_scene(Scene::MainMenu); }
    }

    fn evaluate_mdp(&self, enemy: &Entity, player_pos: Vector2) -> Vec<(AIState, f32)> {
        let dist = enemy.position.distance(player_pos);
        let is_player_above = player_pos.y < enemy.position.y - 10.0;
        let is_close = dist < enemy.vision_range;
        if is_close {
            if is_player_above { vec![(AIState::Attack, 0.6), (AIState::Flee, 0.3), (AIState::Chase, 0.1)] }
            else { vec![(AIState::Chase, 0.7), (AIState::Flee, 0.2), (AIState::Attack, 0.1)] }
        } else { vec![(AIState::Patrol, 0.9), (AIState::Chase, 0.1)] }
    }

    fn update_playing(&mut self, dt: f32) {
        let gravity = 800.0; let move_speed = 200.0; let jump_force = -400.0; let wall_slide_speed = 50.0;
        if self.invincibility_timer > 0.0 { self.invincibility_timer -= dt; }
        if self.combo_timer > 0.0 { self.combo_timer -= dt; if self.combo_timer <= 0.0 { self.combo_count = 0; } }

        self.jump_buffer_timer -= dt;
        if self.input.jump_pressed() { self.jump_buffer_timer = 0.1; }

        let player_pos = self.entities.iter()
            .find(|e| e.tag == Some(Tag::Player) && e.active)
            .map(|e| e.position).unwrap_or(Vector2::ZERO);

        if self.input.interact_pressed() {
            for ent in &mut self.entities {
                if ent.tag == Some(Tag::Lever) && ent.active {
                    let dist = ent.position.distance(player_pos);
                    if dist < 50.0 {
                        ent.is_activated = !ent.is_activated;
                        play_sound(self.assets.lever_sound, PlaySoundParams { looped: false, volume: 0.6 * self.sfx_volume });
                        for d in &mut self.entities {
                            if d.tag == Some(Tag::Door) { d.active = !ent.is_activated; self.particles.emit(d.position, 10, GOLD); }
                        }
                        break;
                    }
                }
            }
        }

        for ent in &mut self.entities {
            if !ent.active { continue; }
            ent.touching_wall = 0; 
            if ent.portal_cooldown > 0.0 { ent.portal_cooldown -= dt; }
            
            if !ent.frames.is_empty() {
                if ent.velocity.x.abs() > 10.0 || ent.velocity.y.abs() > 10.0 {
                    ent.frame_timer -= dt;
                    if ent.frame_timer <= 0.0 { ent.current_frame = (ent.current_frame + 1) % ent.frames.len(); ent.frame_timer = 0.15; }
                } else { ent.current_frame = 0; }
            }

            if ent.tag == Some(Tag::Projectile) {
                if let Some(life) = &mut ent.lifespan { *life -= dt; if *life <= 0.0 { ent.active = false; } }
            }

            if ent.tag == Some(Tag::MovingPlatform) || ent.tag == Some(Tag::VerticalPlatform) {
                let is_vert = ent.tag == Some(Tag::VerticalPlatform);
                let target_pos = if is_vert { ent.start_pos.y + ent.move_range * ent.direction } else { ent.start_pos.x + ent.move_range * ent.direction };
                let vel = ent.move_speed * ent.direction;
                if is_vert { ent.velocity.y = vel; ent.position.y += vel * dt; } 
                else { ent.velocity.x = vel; ent.position.x += vel * dt; }
                let current_pos = if is_vert { ent.position.y } else { ent.position.x };
                if (current_pos >= target_pos && ent.direction > 0.0) || (current_pos <= ent.start_pos.y && ent.direction < 0.0 && is_vert) || (current_pos <= ent.start_pos.x && ent.direction < 0.0 && !is_vert) {
                    ent.direction *= -1.0;
                }
            }

            if ent.tag == Some(Tag::Player) {
                if ent.dash_timer > 0.0 {
                    ent.dash_timer -= dt;
                    ent.velocity.x = ent.direction * 1000.0;
                    ent.velocity.y = 0.0; 
                    self.particles.emit(ent.position, 1, BLUE);
                } else {
                    if self.input.left() { ent.velocity.x = -move_speed; ent.direction = -1.0; }
                    else if self.input.right() { ent.velocity.x = move_speed; ent.direction = 1.0; }
                    else { ent.velocity.x = 0.0; }

                    if ent.is_grounded { self.coyote_timer = 0.1; ent.current_jumps = 0; } 
                    else { self.coyote_timer -= dt; }

                    if !ent.is_grounded && ent.touching_wall != 0 && ent.velocity.y > 0.0 {
                        ent.velocity.y = ent.velocity.y.min(wall_slide_speed);
                    }

                    if self.jump_buffer_timer > 0.0 {
                        if self.coyote_timer > 0.0 {
                            ent.velocity.y = jump_force; ent.is_grounded = false; self.coyote_timer = 0.0; self.jump_buffer_timer = 0.0;
                            play_sound(self.assets.jump_sound, PlaySoundParams { looped: false, volume: 0.4 * self.sfx_volume });
                        } else if ent.touching_wall != 0 && !ent.is_grounded {
                            ent.velocity.y = jump_force; ent.velocity.x = -ent.touching_wall as f32 * move_speed * 1.5; ent.direction = -ent.touching_wall as f32; self.jump_buffer_timer = 0.0;
                            play_sound(self.assets.jump_sound, PlaySoundParams { looped: false, volume: 0.4 * self.sfx_volume });
                        } else if ent.current_jumps < ent.max_jumps {
                            ent.velocity.y = jump_force; ent.current_jumps += 1; self.jump_buffer_timer = 0.0;
                            play_sound(self.assets.jump_sound, PlaySoundParams { looped: false, volume: 0.4 * self.sfx_volume });
                        }
                    }
                }

                if ent.dash_cooldown > 0.0 { ent.dash_cooldown -= dt; }
                if self.input.dash_pressed() && ent.dash_cooldown <= 0.0 && ent.dash_timer <= 0.0 {
                    ent.dash_timer = 0.15;
                    ent.dash_cooldown = 1.0;
                    self.invincibility_timer = self.invincibility_timer.max(0.15);
                    play_sound(self.assets.dash_sound, PlaySoundParams { looped: false, volume: 0.4 * self.sfx_volume });
                }

                if self.input.shoot_pressed() {
                    let mut proj = Entity::new(ent.position.x + ent.direction * 30.0, ent.position.y + 10.0, 15.0, 15.0, ORANGE).with_tag(Tag::Projectile);
                    proj.velocity.x = 400.0 * ent.direction; proj.lifespan = Some(2.0); proj.frames.push(self.assets.projectile_tex);
                    self.entities.push(proj);
                    play_sound(self.assets.shoot_sound, PlaySoundParams { looped: false, volume: 0.3 * self.sfx_volume });
                }

                if self.input.place_block_pressed() && self.blocks_held > 0 {
                    let grid_x = ((ent.position.x + ent.direction * 40.0) / tile).floor() * tile;
                    let grid_y = (ent.position.y / tile).floor() * tile;
                    let can_place = !self.entities.iter().any(|e| e.position.x == grid_x && e.position.y == grid_y);
                    if can_place {
                        let mut block = Entity::new(grid_x, grid_y, tile, tile, GRAY).with_tag(Tag::PlayerBlock).make_static();
                        block.frames.push(self.assets.platform_tex);
                        self.entities.push(block);
                        self.blocks_held -= 1;
                        play_sound(self.assets.place_sound, PlaySoundParams { looped: false, volume: 0.4 * self.sfx_volume });
                    }
                }
            }

            if ent.tag == Some(Tag::Enemy) {
                ent.ai_decision_timer -= dt;
                if ent.ai_decision_timer <= 0.0 {
                    let probabilities = self.evaluate_mdp(ent, player_pos);
                    ent.ai_state = sample_mdp(&probabilities);
                    ent.ai_decision_timer = 0.5;
                }
                match ent.ai_state {
                    AIState::Patrol => { if ent.is_grounded { ent.velocity.x = 50.0 * ent.direction; } }
                    AIState::Chase => { let dir = if player_pos.x > ent.position.x { 1.0 } else { -1.0 }; ent.direction = dir; if ent.is_grounded { ent.velocity.x = 80.0 * dir; } }
                    AIState::Attack => { let dir = if player_pos.x > ent.position.x { 1.0 } else { -1.0 }; ent.direction = dir; if ent.is_grounded { ent.velocity.x = 120.0 * dir; ent.velocity.y = jump_force * 0.8; ent.is_grounded = false; } }
                    AIState::Flee => { let dir = if player_pos.x > ent.position.x { -1.0 } else { 1.0 }; ent.direction = dir; if ent.is_grounded { ent.velocity.x = 100.0 * dir; } }
                }
            }

            if ent.tag == Some(Tag::FlyingEnemy) {
                let dir_vec = (player_pos - ent.position).normalize_or_zero();
                ent.velocity = dir_vec * 80.0;
                ent.direction = if ent.velocity.x > 0.0 { 1.0 } else { -1.0 };
            }

            if !ent.is_static && ent.tag != Some(Tag::MovingPlatform) && ent.tag != Some(Tag::VerticalPlatform) && ent.tag != Some(Tag::Projectile) && ent.tag != Some(Tag::FlyingEnemy) {
                ent.velocity.y += gravity * dt;
            }
        }

        let mut static_indices = Vec::new();
        let mut dynamic_indices = Vec::new();
        for i in 0..self.entities.len() {
            if !self.entities[i].active { continue; }
            if self.entities[i].is_static || self.entities[i].tag == Some(Tag::MovingPlatform) || self.entities[i].tag == Some(Tag::VerticalPlatform) {
                static_indices.push(i);
            } else {
                dynamic_indices.push(i);
            }
        }

        // Move X & Resolve
        for &i in &dynamic_indices {
            self.entities[i].position.x += self.entities[i].velocity.x * dt;
            self.entities[i].is_grounded = false;

            for &j in &static_indices {
                if i == j || !self.entities[j].active { continue; }
                if self.entities[i].bounds().intersect(self.entities[j].bounds()) {
                    // Portal Logic
                    if self.entities[j].tag == Some(Tag::Portal) && self.entities[i].portal_cooldown <= 0.0 {
                        // Find other portal
                        let my_pos = self.entities[j].position;
                        let mut target_pos = Vector2::ZERO;
                        let mut found = false;
                        for p in &self.entities {
                            if p.tag == Some(Tag::Portal) && p.position != my_pos { target_pos = p.position; found = true; break; }
                        }
                        if found {
                            self.entities[i].position = target_pos;
                            self.entities[i].portal_cooldown = 0.5;
                            self.particles.emit(my_pos, 20, CYAN);
                            self.particles.emit(target_pos, 20, CYAN);
                            play_sound(self.assets.teleport_sound, PlaySoundParams { looped: false, volume: 0.5 * self.sfx_volume });
                            continue; // Skip solid collision
                        }
                    }

                    // Breakable Logic
                    if self.entities[j].tag == Some(Tag::Breakable) {
                        let is_dashing = self.entities[i].tag == Some(Tag::Player) && self.entities[i].dash_timer > 0.0;
                        let is_projectile = self.entities[i].tag == Some(Tag::Projectile);
                        
                        if is_dashing || is_projectile {
                            self.entities[j].health -= 1;
                            if self.entities[j].health <= 0.0 {
                                self.entities[j].active = false;
                                self.particles.emit(self.entities[j].position, 20, BROWN);
                                play_sound(self.assets.break_sound, PlaySoundParams { looped: false, volume: 0.6 * self.sfx_volume });
                            }
                            if is_projectile { self.entities[i].active = false; }
                            continue; // Don't stop momentum when breaking
                        }
                    }

                    if self.entities[i].velocity.x > 0.0 {
                        self.entities[i].position.x = self.entities[j].bounds().x - self.entities[i].size.x;
                        self.entities[i].touching_wall = 1; 
                        if self.entities[i].tag == Some(Tag::Enemy) && self.entities[i].ai_state == AIState::Patrol { self.entities[i].direction = -1.0; }
                    } else if self.entities[i].velocity.x < 0.0 {
                        self.entities[i].position.x = self.entities[j].bounds().x + self.entities[j].bounds().w;
                        self.entities[i].touching_wall = -1; 
                        if self.entities[i].tag == Some(Tag::Enemy) && self.entities[i].ai_state == AIState::Patrol { self.entities[i].direction = 1.0; }
                    }
                    self.entities[i].velocity.x = 0.0;
                }
            }
            
            if self.entities[i].tag == Some(Tag::Player) {
                for &j in &dynamic_indices {
                    if i == j || !self.entities[j].active { continue; }
                    if self.entities[j].tag == Some(Tag::Crate) {
                        if self.entities[i].bounds().intersect(self.entities[j].bounds()) {
                            let overlap = if self.entities[i].velocity.x > 0.0 {
                                (self.entities[i].bounds().x + self.entities[i].size.x) - self.entities[j].bounds().x
                            } else {
                                self.entities[i].bounds().x - (self.entities[j].bounds().x + self.entities[j].bounds().w)
                            };
                            self.entities[j].position.x += overlap;
                            self.entities[j].velocity.x = self.entities[i].velocity.x; 
                            self.entities[i].position.x -= overlap;
                        }
                    }
                }
            }
            
            if self.entities[i].tag == Some(Tag::Projectile) {
                for &j in &dynamic_indices {
                    if i == j || !self.entities[j].active { continue; }
                    if self.entities[i].bounds().intersect(self.entities[j].bounds()) {
                        if self.entities[j].tag == Some(Tag::Enemy) || self.entities[j].tag == Some(Tag::FlyingEnemy) {
                            self.entities[j].active = false; self.entities[i].active = false;
                            self.shake(0.2); self.particles.emit(self.entities[j].position, 15, RED);
                            play_sound(self.assets.stomp_sound, PlaySoundParams { looped: false, volume: 0.5 * self.sfx_volume });
                            self.add_score(10);
                        }
                    }
                }
            }
        }

        // Move Y & Resolve
        for &i in &dynamic_indices {
            self.entities[i].position.y += self.entities[i].velocity.y * dt;

            for &j in &static_indices {
                if i == j || !self.entities[j].active { continue; }
                if self.entities[i].bounds().intersect(self.entities[j].bounds()) {
                    if self.entities[j].tag == Some(Tag::Bouncer) && self.entities[i].velocity.y > 0.0 {
                        self.entities[i].velocity.y = -800.0; 
                        play_sound(self.assets.bounce_sound, PlaySoundParams { looped: false, volume: 0.6 * self.sfx_volume });
                        continue;
                    }

                    if self.entities[i].velocity.y > 0.0 {
                        self.entities[i].position.y = self.entities[j].bounds().y - self.entities[i].size.y;
                        self.entities[i].is_grounded = true; self.entities[i].current_jumps = 0;
                        if self.entities[j].tag == Some(Tag::MovingPlatform) || self.entities[j].tag == Some(Tag::VerticalPlatform) {
                            self.entities[i].position.x += self.entities[j].velocity.x * dt;
                            self.entities[i].position.y += self.entities[j].velocity.y * dt;
                        }
                    } else if self.entities[i].velocity.y < 0.0 {
                        self.entities[i].position.y = self.entities[j].bounds().y + self.entities[j].bounds().h;
                    }
                    self.entities[i].velocity.y = 0.0;

                    if self.entities[j].tag == Some(Tag::Hazard) && self.entities[i].tag == Some(Tag::Player) {
                        self.health = 0; 
                    }

                    if self.entities[i].tag == Some(Tag::Player) {
                        if self.entities[j].tag == Some(Tag::Enemy) || self.entities[j].tag == Some(Tag::FlyingEnemy) {
                            let can_stomp = self.entities[i].velocity.y >= 0.0 && self.entities[j].tag != Some(Tag::FlyingEnemy);
                            if can_stomp { 
                                self.entities[j].active = false; self.entities[i].velocity.y = jump_force * 0.6;
                                self.shake(0.2); self.particles.emit(self.entities[j].position, 15, RED);
                                play_sound(self.assets.stomp_sound, PlaySoundParams { looped: false, volume: 0.5 * self.sfx_volume });
                                self.add_score(15);
                            } else if self.invincibility_timer <= 0.0 { 
                                self.health -= 1; self.invincibility_timer = 1.5; self.shake(0.3);
                                self.particles.emit(self.entities[i].position, 10, WHITE);
                                play_sound(self.assets.hurt_sound, PlaySoundParams { looped: false, volume: 0.5 * self.sfx_volume });
                                self.entities[i].velocity.x = -100.0 * self.entities[i].direction;
                                self.entities[i].velocity.y = -200.0;
                            }
                        } else if self.entities[j].tag == Some(Tag::Coin) {
                            self.entities[j].active = false;
                            self.add_score(10);
                            self.particles.emit(self.entities[j].position, 10, YELLOW);
                            play_sound(self.assets.coin_sound, PlaySoundParams { looped: false, volume: 0.5 * self.sfx_volume });
                        } else if self.entities[j].tag == Some(Tag::PowerUp) {
                            self.entities[j].active = false; self.entities[i].max_jumps = 2;
                            self.particles.emit(self.entities[j].position, 20, GREEN);
                            play_sound(self.assets.powerup_sound, PlaySoundParams { looped: false, volume: 0.6 * self.sfx_volume });
                        } else if self.entities[j].tag == Some(Tag::Key) {
                            self.entities[j].active = false; self.keys_held += 1;
                            self.particles.emit(self.entities[j].position, 10, GOLD);
                            play_sound(self.assets.coin_sound, PlaySoundParams { looped: false, volume: 0.5 * self.sfx_volume });
                        } else if self.entities[j].tag == Some(Tag::Checkpoint) && !self.entities[j].is_activated {
                            self.entities[j].is_activated = true; self.entities[j].color = GREEN;
                            self.player_spawn = self.entities[j].position; self.has_checkpoint = true;
                            self.particles.emit(self.entities[j].position, 15, GREEN);
                            play_sound(self.assets.checkpoint_sound, PlaySoundParams { looped: false, volume: 0.6 * self.sfx_volume });
                        } else if self.entities[j].tag == Some(Tag::BlockPickup) {
                            self.entities[j].active = false; self.blocks_held += 1;
                            self.particles.emit(self.entities[j].position, 10, WHITE);
                            play_sound(self.assets.coin_sound, PlaySoundParams { looped: false, volume: 0.5 * self.sfx_volume });
                        } else if self.entities[j].tag == Some(Tag::HealthPickup) {
                            if self.health < self.max_health {
                                self.entities[j].active = false; self.health += 1;
                                self.particles.emit(self.entities[j].position, 15, RED);
                                play_sound(self.assets.heal_sound, PlaySoundParams { looped: false, volume: 0.6 * self.sfx_volume });
                            }
                        }
                    }
                }
            }
            
            if self.entities[i].tag == Some(Tag::Crate) {
                for &j in &dynamic_indices {
                    if i == j || !self.entities[j].active { continue; }
                    if self.entities[j].tag == Some(Tag::Crate) {
                        if self.entities[i].bounds().intersect(self.entities[j].bounds()) {
                            if self.entities[i].velocity.y > 0.0 {
                                self.entities[i].position.y = self.entities[j].bounds().y - self.entities[i].size.y;
                                self.entities[i].is_grounded = true; 
                            } else if self.entities[i].velocity.y < 0.0 {
                                self.entities[i].position.y = self.entities[j].bounds().y + self.entities[j].bounds().h;
                            }
                            self.entities[i].velocity.y = 0.0;
                        }
                    }
                }
            }
        }

        self.particles.update(dt);

        let mut player_alive = false;
        let mut coins_left = 0;
        for ent in &self.entities {
            if ent.tag == Some(Tag::Player) && ent.active {
                player_alive = true;
                if ent.position.y > 1000.0 { player_alive = false; }
            }
            if ent.tag == Some(Tag::Coin) && ent.active { coins_left += 1; }
        }

        if !player_alive || self.health <= 0 {
            if self.has_checkpoint {
                self.health = self.max_health; self.invincibility_timer = 1.5;
                if let Some(p) = self.entities.iter_mut().find(|e| e.tag == Some(Tag::Player)) {
                    p.position = self.player_spawn; p.velocity = Vec2::ZERO; p.active = true; p.dash_timer = 0.0; p.dash_cooldown = 0.0; p.portal_cooldown = 0.0;
                }
                for ent in &mut self.entities { if ent.tag == Some(Tag::Projectile) { ent.active = false; } }
            } else { self.request_scene(Scene::GameOver); }
        }

        if coins_left == 0 { self.request_scene(Scene::Win); }

        let mut min_x = f32::MAX; let mut max_x = f32::MIN;
        let mut min_y = f32::MAX; let mut max_y = f32::MIN;
        for ent in &self.entities {
            if ent.is_static || ent.tag == Some(Tag::MovingPlatform) || ent.tag == Some(Tag::VerticalPlatform) {
                min_x = min_x.min(ent.position.x); max_x = max_x.max(ent.position.x + ent.size.x);
                min_y = min_y.min(ent.position.y); max_y = max_y.max(ent.position.y + ent.size.y);
            }
        }

        let target_pos = self.entities.iter().find(|e| e.tag == Some(Tag::Player) && e.active).map(|e| e.position).unwrap_or(Vec2::ZERO);
        let lerp_factor = 1.0 - (-8.0 * dt).exp(); 
        self.camera.target += (target_pos - self.camera.target) * lerp_factor;
        
        if self.camera_bounds {
            let half_w = screen_width() / 2.0; let half_h = screen_height() / 2.0;
            if max_x - min_x > screen_width() { self.camera.target.x = self.camera.target.x.max(min_x + half_w).min(max_x - half_w); } 
            else { self.camera.target.x = (min_x + max_x) / 2.0; }
            if max_y - min_y > screen_height() { self.camera.target.y = self.camera.target.y.max(min_y + half_h).min(max_y - half_h); } 
            else { self.camera.target.y = (min_y + max_y) / 2.0; }
        }
        
        self.camera.offset = Vec2::new(screen_width() / 2.0, screen_height() / 2.0);
        self.camera.zoom = Vec2::new(1.0, -1.0);

        if self.shake_time > 0.0 {
            self.shake_time -= dt;
            let mut rng = rand::thread_rng();
            self.camera.offset.x += rng.gen_range(-10.0..10.0) * self.shake_time;
            self.camera.offset.y += rng.gen_range(-10.0..10.0) * self.shake_time;
        }
    }

    async fn render(&self) {
        clear_background(Color::from_hex(0x0f0f1a));

        match self.scene {
            Scene::MainMenu => {
                draw_text("MINI RUST ENGINE", screen_width()/2.0 - 160.0, 100.0, 40.0, WHITE);
                draw_text("Featuring Portals, Combos, and Breakable Blocks", screen_width()/2.0 - 260.0, 140.0, 20.0, LIGHTGRAY);
            }
            Scene::Settings => { draw_text("SETTINGS", screen_width()/2.0 - 80.0, 80.0, 40.0, WHITE); }
            Scene::Editor => {
                let tile = self.editor_grid_size;
                for x in 0..(screen_width() / tile) as usize { draw_line(x as f32 * tile, 0.0, x as f32 * tile, screen_height(), 1.0, Color::new(1.0, 1.0, 1.0, 0.1)); }
                for y in 0..(screen_height() / tile) as usize { draw_line(0.0, y as f32 * tile, screen_width(), y as f32 * tile, 1.0, Color::new(1.0, 1.0, 1.0, 0.1)); }
                for ent in &self.entities {
                    if !ent.frames.is_empty() { draw_texture(&ent.frames[0], ent.position.x, ent.position.y, if ent.is_activated { GREEN } else { WHITE }); }
                    else { draw_rectangle(ent.position.x, ent.position.y, ent.size.x, ent.size.y, ent.color); }
                }
                let (mx, my) = mouse_position();
                let grid_x = (mx / tile).floor() * tile;
                let grid_y = (my / tile).floor() * tile;
                draw_rectangle_lines(grid_x, grid_y, tile, tile, 2.0, YELLOW);
            }
            Scene::Playing | Scene::Paused | Scene::GameOver | Scene::Win => {
                set_camera(&self.camera);

                for (x, y, depth) in &self.stars {
                    let px = *x - self.camera.target.x * depth;
                    let py = *y - self.camera.target.y * depth;
                    draw_circle(px, py, 2.0 * depth, Color::new(1.0, 1.0, 1.0, *depth));
                }

                for ent in &self.entities {
                    if ent.active {
                        let draw_color = if ent.tag == Some(Tag::Player) && self.invincibility_timer > 0.0 {
                            if (self.invincibility_timer * 20.0).floor() % 2.0 == 0.0 { WHITE } else { Color::new(1.0, 1.0, 1.0, 0.3) }
                        } else { WHITE };

                        if !ent.frames.is_empty() {
                            let tex = &ent.frames[ent.current_frame.min(ent.frames.len()-1)];
                            let final_color = if ent.tag == Some(Tag::Checkpoint) && ent.is_activated { GREEN } 
                                              else if ent.tag == Some(Tag::PlayerBlock) { LIGHTGRAY } 
                                              else if ent.tag == Some(Tag::Lever) && ent.is_activated { YELLOW }
                                              else if ent.tag == Some(Tag::Portal) { 
                                                  let pulse = (get_time() * 4.0).sin().abs(); 
                                                  Color::new(1.0, 1.0, 1.0, 0.5 + pulse * 0.5) 
                                              }
                                              else { draw_color };

                            if ent.direction < 0.0 && ent.tag != Some(Tag::Projectile) && ent.tag != Some(Tag::Checkpoint) && ent.tag != Some(Tag::Crate) && ent.tag != Some(Tag::Lever) && ent.tag != Some(Tag::MovingPlatform) && ent.tag != Some(Tag::VerticalPlatform) && ent.tag != Some(Tag::Portal) {
                                draw_texture_ex(tex, ent.position.x, ent.position.y, final_color, DrawTextureParams { flip_x: true, ..Default::default() });
                            } else {
                                draw_texture(*tex, ent.position.x, ent.position.y, final_color);
                            }
                        } else { draw_rectangle(ent.position.x, ent.position.y, ent.size.x, ent.size.y, ent.color); }
                    }
                }

                self.particles.render();

                if self.show_debug {
                    let player_pos = self.entities.iter().find(|e| e.tag == Some(Tag::Player)).map(|e| e.position + e.size * 0.5).unwrap_or(Vec2::ZERO);
                    for ent in &self.entities {
                        if (ent.tag == Some(Tag::Enemy) || ent.tag == Some(Tag::FlyingEnemy)) && ent.active {
                            let enemy_center = ent.position + ent.size * 0.5;
                            draw_circle_lines(enemy_center.x, enemy_center.y, ent.vision_range, 1.0, GREEN);
                            let dist = enemy_center.distance(player_pos);
                            let line_color = if dist < ent.vision_range { RED } else { DARKGRAY };
                            draw_line(enemy_center.x, enemy_center.y, player_pos.x, player_pos.y, 1.0, line_color);
                        }
                    }
                }

                set_default_camera();

                if self.scene == Scene::Paused {
                    draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.0, 0.0, 0.0, 0.7));
                    draw_text("PAUSED", screen_width()/2.0 - 70.0, 180.0, 40.0, WHITE);
                }

                if self.darkness_level > 0.0 {
                    let player = self.entities.iter().find(|e| e.tag == Some(Tag::Player) && e.active);
                    if let Some(p) = player {
                        let p_screen = self.camera.world_to_screen(p.position + p.size * 0.5);
                        let r = 200.0; 
                        let c = Color::new(0.0, 0.0, 0.0, self.darkness_level);
                        draw_rectangle(0.0, 0.0, screen_width(), p_screen.y - r, c);
                        draw_rectangle(0.0, p_screen.y + r, screen_width(), screen_height() - (p_screen.y + r), c);
                        draw_rectangle(0.0, p_screen.y - r, p_screen.x - r, r * 2.0, c);
                        draw_rectangle(p_screen.x + r, p_screen.y - r, screen_width() - (p_screen.x + r), r * 2.0, c);
                    }
                }

                draw_text(&format!("Score: {}", self.score), 20.0, 30.0, 30.0, WHITE);
                
                let player = self.entities.iter().find(|e| e.tag == Some(Tag::Player));
                let pulse = 1.0 + (get_time() * 4.0).sin() * 0.1;
                for i in 0..self.max_health {
                    let tex = if i < self.health { self.assets.heart_full_tex } else { self.assets.heart_empty_tex };
                    let mut params = DrawTextureParams::default();
                    if i == self.health - 1 && self.health < self.max_health {
                        params.dest_size = Some(Vec2::new(20.0 * pulse, 20.0 * pulse));
                    } else {
                        params.dest_size = Some(Vec2::new(20.0, 20.0));
                    }
                    draw_texture_ex(tex, 20.0 + (i as f32 * 25.0), 50.0, WHITE, params);
                }

                self.ui.label(20.0, 90.0, "Keys", LIGHTGRAY);
                draw_rectangle(20.0, 100.0, 20.0, 20.0, GOLD);
                draw_text(&format!("x {}", self.keys_held), 50.0, 118.0, 20.0, WHITE);

                self.ui.label(20.0, 130.0, "Blocks", LIGHTGRAY);
                draw_rectangle(20.0, 140.0, 20.0, 20.0, LIGHTGRAY);
                draw_text(&format!("x {}", self.blocks_held), 50.0, 158.0, 20.0, WHITE);

                self.ui.label(20.0, 180.0, "Dash", LIGHTGRAY);
                if let Some(p) = player {
                    let dash_cd = p.dash_cooldown / 1.0;
                    let dash_color = if dash_cd <= 0.0 { BLUE } else { DARKGRAY };
                    self.ui.progress_bar(20.0, 190.0, 100.0, 10.0, 1.0 - dash_cd, 1.0, dash_color);
                    if p.max_jumps > 1 { draw_text("Double Jump: UNLOCKED", 20.0, 220.0, 20.0, GREEN); }
                }

                if self.combo_timer > 0.0 {
                    let alpha = (self.combo_timer / 2.0).clamp(0.0, 1.0);
                    let combo_color = Color::new(1.0, 1.0, 0.0, alpha);
                    draw_text(&format!("{}x COMBO!", self.combo_count), 20.0, 260.0, 30.0, combo_color);
                    self.ui.progress_bar(20.0, 270.0, 100.0, 5.0, self.combo_timer, 2.0, YELLOW);
                }

                let mm_x = screen_width() - 170.0; let mm_y = 20.0; let mm_w = 150.0; let mm_h = 120.0; let scale = 0.1;
                draw_rectangle(mm_x, mm_y, mm_w, mm_h, Color::new(0.0, 0.0, 0.0, 0.7));
                draw_rectangle_lines(mm_x, mm_y, mm_w, mm_h, 2.0, WHITE);
                let p_pos = player.map(|p| p.position).unwrap_or(Vec2::ZERO);
                for ent in &self.entities {
                    if !ent.active { continue; }
                    let rel_x = (ent.position.x - p_pos.x) * scale + mm_w / 2.0;
                    let rel_y = (ent.position.y - p_pos.y) * scale + mm_h / 2.0;
                    if rel_x >= 0.0 && rel_x <= mm_w && rel_y >= 0.0 && rel_y <= mm_h {
                        let color = match ent.tag {
                            Some(Tag::Player) => BLUE, Some(Tag::Enemy) | Some(Tag::FlyingEnemy) => RED, Some(Tag::Coin) => YELLOW,
                            Some(Tag::Platform) | Some(Tag::PlayerBlock) | Some(Tag::Door) | Some(Tag::Crate) | Some(Tag::Breakable) => GRAY,
                            Some(Tag::MovingPlatform) | Some(Tag::VerticalPlatform) => PURPLE, Some(Tag::Checkpoint) => if ent.is_activated { GREEN } else { WHITE },
                            Some(Tag::Portal) => CYAN,
                            _ => continue,
                        };
                        draw_rectangle(mm_x + rel_x, mm_y + rel_y, 2.0, 2.0, color);
                    }
                }

                if self.show_debug { draw_text("AI DEBUG MODE (F1)", 20.0, screen_height() - 20.0, 20.0, GREEN); }
            }
        }
        
        if self.scene == Scene::GameOver {
            draw_text("GAME OVER", screen_width()/2.0 - 120.0, screen_height()/2.0 - 40.0, 50.0, RED);
            draw_text("Press ENTER to return to Menu", screen_width()/2.0 - 160.0, screen_height()/2.0 + 40.0, 25.0, YELLOW);
        } else if self.scene == Scene::Win {
            draw_text("YOU WIN!", screen_width()/2.0 - 100.0, screen_height()/2.0 - 40.0, 50.0, GREEN);
            draw_text("Press ENTER to return to Menu", screen_width()/2.0 - 160.0, screen_height()/2.0 + 40.0, 25.0, YELLOW);
        }

        if self.transition_alpha > 0.0 { draw_rectangle(0.0, 0.0, screen_width(), screen_height(), Color::new(0.0, 0.0, 0.0, self.transition_alpha)); }
        self.ui.render_tooltip();
        next_frame().await
    }
}

#[macroquad::main("Advanced Mini Rust Engine v15 - Portals, Combos, Breakables")]
async fn main() {
    let mut engine = Engine::new().await;
    engine.run().await;
}
