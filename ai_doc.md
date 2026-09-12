# Руководство разработчика игры (Gameplay Scripting Guide)

Добро пожаловать в руководство по созданию геймплея на базе движка. Архитектура проекта построена на парадигме **Data-Driven ECS** (Flecs) с физическим бэкендом **Rapier 3D**, компонентной рефлексией и плагинной системой `GamePlugin`.

---

## 1. Архитектура и порядок выполнения кадра (Frame Lifecycle)

Вся игра оформляется как реализация трейта `GamePlugin`. Движок разделяет время на **фиксированные физические тики** (Fixed Update, 60 Гц) и **переменные кадры рендера** (Update / Render).

```text
[ Кадр рендера (dt) ]
  │
  ├──► [ ФИКСИРОВАННЫЙ ЦИКЛ (60 Гц) ] while accumulator >= 1/60:
  │      ├── 1. physics.sync_from_ecs()        (Передача Transform и форм в Rapier)
  │      ├── 2. physics.step(fixed_dt)          (Шаг физической симуляции)
  │      ├── 3. physics.sync_to_ecs()          (Возврат динамических поз в ECS)
  │      ├── 4. plugin.on_collision()          (События триггеров и контактов)
  │      └── 5. plugin.on_fixed_update()       (KCC персонажа, физические силы, двери)
  │
  ├──► plugin.on_update(dt)                    (Вращение камеры мышью, логика оружия, таймеры)
  ├──► world.progress()                        (Системы Flecs)
  ├──► plugin.on_late_update(dt)               (Пост-логика, фиксация камеры)
  ├──► input.clear_frame_deltas()              (Сброс дельт мыши и одиночных нажатий)
  ├──► world.update_transforms()               (Обновление иерархии сцены)
  ├──► physics.update_render_transforms(alpha) (Интерполяция поз физики под частоту монитора)
  └──► Render & GPU Draw Calls
```

---

## 2. Трейт `GamePlugin` — точка входа геймплея

Создайте структуру игры и реализуйте для нее методы `GamePlugin`:

```rust
use ecs::World;
use engine::game::{GameInput, GamePlugin, PhysicsCollisionEvent};
use engine::physics::PhysicsWorld;

#[derive(Default)]
pub struct MyGamePlugin {
    pub player_entity: Option<u64>,
}

impl GamePlugin for MyGamePlugin {
    /// Вызывается 1 раз при запуске движка (инициализация статических ресурсов)
    fn init(&mut self, world: &World) {
        log::info!("Game Plugin Initialized");
    }

    /// Вызывается при нажатии Play в редакторе или старте игры
    fn on_start(&mut self, world: &World, physics: &mut PhysicsWorld) {
        log::info!("Game Started (Play Mode)");
        // Здесь обычно спавнятся уровень, игрок и враги
    }

    /// Вызывается при нажатии Stop в редакторе (мир откатывается к снапшоту)
    fn on_stop(&mut self, world: &World) {
        log::info!("Game Stopped");
        self.player_entity = None;
    }

    /// Обработка входа/выхода из триггеров и физических столкновений
    fn on_collision(&mut self, world: &World, event: &PhysicsCollisionEvent) {
        // Логика ловушек, зон заражения, сбора предметов
    }

    /// Фиксированный шаг физики (60 Гц). Здесь реализуется KCC и движение дверей
    fn on_fixed_update(
        &mut self,
        world: &World,
        physics: &mut PhysicsWorld,
        input: &GameInput,
        fixed_dt: f32,
    ) {
        // Управление персонажем, гравитация, движение кинематических платформ
    }

    /// Переменный шаг кадра. Здесь реализуются поворот головы мыши, стрельба и таймеры
    fn on_update(
        &mut self,
        world: &World,
        physics: &mut PhysicsWorld,
        input: &GameInput,
        dt: f32,
    ) {
        // Хитскан, рейкаст взаимодействия на E, наклон камеры
    }

    /// Вызывается после on_update и progress()
    fn on_late_update(
        &mut self,
        world: &World,
        physics: &mut PhysicsWorld,
        input: &GameInput,
        dt: f32,
    ) {
    }
}
```

Регистрация игры в `main.rs`:
```rust
fn main() -> anyhow::Result<()> {
    let engine_config = engine::EngineConfig {
        window_title: "SCP: Complex Escape".to_string(),
        window_width: 1920,
        window_height: 1080,
        msaa_samples: 1,
    };

    let game_plugin = Box::new(MyGamePlugin::default());
    let editor_app = editor::Editor::new(editor::EditorConfig::default());

    engine::run(engine_config, editor_app, game_plugin)
}
```

---

## 3. Создание кастомных компонентов (`Inspectable`)

Движок поддерживает автоматическую рефлексию: компоненты с макросом `Inspectable` **автоматически регистрируются**, отображаются в инспекторе редактора с удобными ползунками и **сериализуются в RON-сцену**.

```rust
use ecs::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Component, Inspectable, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[component(category = "Gameplay")] // Категория в инспекторе редактора
pub struct HealthComponent {
    #[inspect(range = 0.0..100.0, speed = 0.5)] // Ползунок от 0 до 100
    pub current: f32,

    #[inspect(range = 1.0..500.0)]
    pub max: f32,

    pub is_invulnerable: bool, // Чекбокс

    #[inspect(skip)] // Игнорировать поле в инспекторе редактора
    pub damage_flash_timer: f32,
}

impl Default for HealthComponent {
    fn default() -> Self {
        Self {
            current: 100.0,
            max: 100.0,
            is_invulnerable: false,
            damage_flash_timer: 0.0,
        }
    }
}
```

### Поддерживаемые атрибуты `#[inspect(...)]`:
* `#[component(category = "Имя")]` — имя группы в UI.
* `#[inspect(range = min..max)]` — числовой слайдер с границами.
* `#[inspect(speed = 0.1)]` — чувствительность перетаскивания мышью.
* `#[inspect(color)]` — отображать `[f32; 3]` как цветовую палитру RGB.
* `#[inspect(skip)]` — скрыть поле от отображения в редакторе.

---

## 4. Работа с ECS (`ecs::World`)

### Создание и удаление сущностей
```rust
// 1. Создать корневую сущность
let entity_id = world.spawn_empty("Security_Door");

// 2. Создать дочернюю сущность (например, сокет оружия или камеру)
let child_id = world.spawn_child(parent_id, "Muzzle_Flash");

// 3. Проверка жизни и удаление
if world.is_alive(entity_id) {
    world.delete_entity(entity_id);
}
```

### Доступ и модификация компонентов
```rust
let entity = world.inner.entity_from_id(entity_id);

// Добавить или перезаписать компонент
entity.set(HealthComponent { current: 80.0, max: 100.0, ..Default::default() });

// Проверить наличие
if entity.has(HealthComponent::id()) {
    // Чтение/Мутация по ссылке через замыкание (безопасно)
    entity.try_get::<&mut HealthComponent>(|hp| {
        hp.current -= 25.0;
    });
}

// Удалить компонент
entity.remove(HealthComponent::id());
```

### Встроенные компоненты движка
* `Transform { position, rotation, scale }` — локальные координаты.
* `GlobalTransform(Mat4)` — вычисленная мировая матрица.
* `CameraComponent { fovy, znear, zfar, is_active, priority }` — камера игрока.
* `LightComponent { light_type, color, intensity, range, inner_angle, outer_angle }` — свет (`Point`, `Spot`, `Directional`).
* `ModelComponent { id, mesh_index }` — связь с 3D-моделью.

---

## 5. Физика и Слои коллизий (Rapier 3D)

### Слои коллизий (`CollisionLayer`)
Чтобы пули не врезались в триггеры, а игрок не застревал в предметах, используйте битовые маски:

```rust
use engine::physics::CollisionLayer;

// Настройка коллайдера для стены/пола:
let wall_col = ColliderComponent {
    shape_type: ColliderShapeType::Cuboid,
    size: [5.0, 0.1, 5.0],
    collision_memberships: CollisionLayer::WORLD,
    collision_filters: u32::MAX, // Со стеной сталкиваются все
    ..Default::default()
};

// Настройка триггера (сенсора):
let trigger_col = ColliderComponent {
    shape_type: ColliderShapeType::Cuboid,
    size: [2.0, 2.0, 2.0],
    is_sensor: true, // Не блокирует движение, только шлет события
    collision_memberships: CollisionLayer::TRIGGER,
    collision_filters: CollisionLayer::PLAYER, // Реагирует только на игрока
    generate_collision_events: true,
    ..Default::default()
};
```

---

## 6. Рецепты и практические примеры

### Рецепт 1: FPS Контроллер персонажа (KCC + WASD + Мышь + Присед)

Создаем компонент состояния движения:
```rust
#[derive(Component, Inspectable, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[component(category = "Player")]
pub struct FpsMovementComponent {
    pub walk_speed: f32,
    pub sprint_speed: f32,
    pub crouch_speed: f32,
    pub jump_force: f32,
    pub gravity: f32,

    #[inspect(skip)] pub vertical_velocity: f32,
    #[inspect(skip)] pub is_grounded: bool,
    #[inspect(skip)] pub is_crouching: bool,
}

impl Default for FpsMovementComponent {
    fn default() -> Self {
        Self {
            walk_speed: 4.5,
            sprint_speed: 7.5,
            crouch_speed: 2.2,
            jump_force: 5.5,
            gravity: 18.0,
            vertical_velocity: 0.0,
            is_grounded: false,
            is_crouching: false,
        }
    }
}
```

#### Функция спавна персонажа с камерой:
```rust
pub fn spawn_player(world: &World, spawn_pos: glam::Vec3) -> (u64, u64) {
    let player_id = world.spawn_empty("Player");

    world.add_transform(player_id, Transform::from_translation(spawn_pos));

    // 1. Кинематическое тело для KCC
    world.add_rigid_body(player_id, RigidBodyComponent {
        body_type: BodyType::KinematicPositionBased,
        lock_rotations: true,
        ccd_enabled: true,
        ..Default::default()
    });

    // 2. Хитбокс-капсула (радиус 0.4, высота 1.8)
    world.add_collider(player_id, ColliderComponent {
        shape_type: ColliderShapeType::Capsule,
        size: [0.4, 1.8, 0.4],
        friction: 0.0,
        collision_memberships: CollisionLayer::PLAYER,
        ..Default::default()
    });

    // 3. Параметры KCC (автостеп ступеней до 35 см)
    world.add_character_controller(player_id, CharacterControllerComponent {
        autostep_max_height: 0.35,
        max_slope_climb_angle: 45.0,
        min_slope_slide_angle: 45.0,
        snap_to_ground_distance: 0.3,
        enable_autostep: true,
        enable_snap_to_ground: true,
        slide: true,
        ..Default::default()
    });

    world.inner.entity_from_id(player_id).set(FpsMovementComponent::default());

    // 4. Дочерняя камера (глаза игрока на высоте 1.65м)
    let camera_id = world.spawn_child(player_id, "FPS_Camera");
    world.add_transform(camera_id, Transform::from_xyz(0.0, 1.65, 0.0));
    world.add_camera(camera_id, CameraComponent {
        fovy: 75.0,
        is_active: true,
        priority: 10,
        ..Default::default()
    });

    (player_id, camera_id)
}
```

#### Обработка в `on_update` (Вращение головы мышью):
```rust
fn on_update(&mut self, world: &World, _physics: &mut PhysicsWorld, input: &GameInput, _dt: f32) {
    let Some(player_id) = self.player_entity else { return; };
    let camera_id = world.get_children(player_id)[0];

    let sensitivity = 0.0025;

    // 1. Поворот тела игрока по горизонтали (Yaw, ось Y)
    world.get_transform_mut(player_id, |t| {
        t.rotation *= glam::Quat::from_rotation_y(-input.look_delta.x * sensitivity);
    });

    // 2. Наклон камеры по вертикали (Pitch, ось X)
    world.get_transform_mut(camera_id, |t| {
        let pitch_delta = -input.look_delta.y * sensitivity;
        let (mut yaw, mut pitch, mut roll) = t.rotation.to_euler(glam::EulerRot::YXZ);
        pitch = (pitch + pitch_delta).clamp(-89.0f32.to_radians(), 89.0f32.to_radians());
        t.rotation = glam::Quat::from_euler(glam::EulerRot::YXZ, 0.0, pitch, 0.0);
    });

    // 3. Обновляем мировую матрицу камеры
    world.update_entity_and_children_transforms(player_id);
}
```

#### Обработка в `on_fixed_update` (Перемещение, гравитация, присед):
```rust
fn on_fixed_update(
    &mut self,
    world: &World,
    physics: &mut PhysicsWorld,
    input: &GameInput,
    fixed_dt: f32,
) {
    let Some(player_id) = self.player_entity else { return; };
    let player_ent = world.inner.entity_from_id(player_id);

    let kcc_comp = player_ent.try_get::<&CharacterControllerComponent>(|k| *k).unwrap();
    let player_rot = player_ent.try_get::<&Transform>(|t| t.rotation).unwrap();

    player_ent.try_get::<&mut FpsMovementComponent>(|move_state| {
        // 1. Приседание (Crouch)
        let is_crouching = input.is_crouch();
        if is_crouching != move_state.is_crouching {
            move_state.is_crouching = is_crouching;
            let target_height = if is_crouching { 0.9 } else { 1.8 };
            let target_offset = if is_crouching { -0.45 } else { 0.0 };

            world.get_collider_mut(player_id, |col| {
                col.size[1] = target_height;
                col.offset_position[1] = target_offset;
            });
        }

        // 2. Скорость движения
        let speed = if move_state.is_crouching {
            move_state.crouch_speed
        } else if input.is_sprint() {
            move_state.sprint_speed
        } else {
            move_state.walk_speed
        };

        // 3. Вектор направления по XZ
        let forward = (player_rot * glam::Vec3::NEG_Z).with_y(0.0).normalize_or_zero();
        let right = (player_rot * glam::Vec3::X).with_y(0.0).normalize_or_zero();
        let wish_dir = (forward * input.move_axis.y + right * input.move_axis.x).normalize_or_zero();

        // 4. Гравитация и прыжок
        if move_state.is_grounded {
            if input.is_jump() && !move_state.is_crouching {
                move_state.vertical_velocity = move_state.jump_force;
                move_state.is_grounded = false;
            } else {
                move_state.vertical_velocity = -1.5; // Snap к ступеням
            }
        } else {
            move_state.vertical_velocity = (move_state.vertical_velocity - move_state.gravity * fixed_dt).max(-45.0);
        }

        let desired_translation = (wish_dir * speed + glam::Vec3::Y * move_state.vertical_velocity) * fixed_dt;

        // 5. Солвер движения KCC
        let movement = physics.move_character_controller(
            player_id,
            &kcc_comp,
            desired_translation,
            fixed_dt,
        );

        // Удар головой о потолок
        if move_state.vertical_velocity > 0.0 && movement.translation.y < desired_translation.y * 0.5 {
            move_state.vertical_velocity = 0.0;
        }

        move_state.is_grounded = movement.is_grounded;

        // 6. Применяем результат к ECS
        world.get_transform_mut(player_id, |t| {
            t.position += movement.translation;
        });
    });
}
```

---

### Рецепт 2: Взаимодействие на клавишу `E` (Двери, Кнопки, Предметы)

```rust
// В on_update:
if input.is_interact() {
    if let Some((cam_pos, cam_rot, _)) = world.get_active_game_camera() {
        let forward = cam_rot * glam::Vec3::NEG_Z;

        // Рейкаст на 2.5 метра с фильтрацией интерактивных слоев
        let hit = physics.cast_ray(
            cam_pos,
            forward,
            2.5,
            RaycastFilter {
                exclude_entity: self.player_entity,
                ignore_sensors: false, // Ловим и сенсорные кнопки
                groups: Some(CollisionLayer::interact_raycast_groups()),
                ..Default::default()
            },
        );

        if let Some(hit) = hit {
            let target = world.inner.entity_from_id(hit.entity);

            // 1. Проверяем, дверь ли это
            if target.has(DoorComponent::id()) {
                target.try_get::<&mut DoorComponent>(|door| {
                    door.toggle();
                });
            }
            // 2. Проверяем, предмет ли это на полу (Keycard / Medkit)
            else if target.has(ItemPickupComponent::id()) {
                log::info!("Подобран предмет: {}", world.get_entity_name(hit.entity));
                world.delete_entity(hit.entity);
            }
        }
    }
}
```

---

### Рецепт 3: Хитскан-стрельба оружием (ЛКМ)

```rust
// В on_update:
if input.is_shoot_just_pressed() {
    if let Some((cam_pos, cam_rot, _)) = world.get_active_game_camera() {
        let shoot_dir = cam_rot * glam::Vec3::NEG_Z;

        let hit = physics.cast_ray(
            cam_pos,
            shoot_dir,
            150.0, // Дистанция выстрела
            RaycastFilter {
                exclude_entity: self.player_entity,
                ignore_sensors: true, // Игнорируем триггеры
                groups: Some(CollisionLayer::weapon_raycast_groups()),
                ..Default::default()
            },
        );

        if let Some(hit) = hit {
            log::info!("Попадание по #{} в точке {}", hit.entity, hit.point);

            let victim = world.inner.entity_from_id(hit.entity);

            // Наносим урон, если у цели есть здоровье
            if victim.has(HealthComponent::id()) {
                victim.try_get::<&mut HealthComponent>(|hp| {
                    if !hp.is_invulnerable {
                        hp.current = (hp.current - 34.0).max(0.0);
                        log::info!("Здоровье цели: {}/{}", hp.current, hp.max);
                    }
                });
            }

            // hit.normal дает ориентацию для спавна эффекта искр или декали
        }
    }
}
```

---

### Рецепт 4: Гермодвери комплекса (Кинематика)

Компонент двери:
```rust
#[derive(Component, Inspectable, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[component(category = "Environment")]
pub struct DoorComponent {
    pub is_open: bool,
    pub progress: f32, // 0.0 .. 1.0
    pub open_offset: [f32; 3],
    pub speed: f32,
}

impl Default for DoorComponent {
    fn default() -> Self {
        Self {
            is_open: false,
            progress: 0.0,
            open_offset: [0.0, 3.0, 0.0], // Сдвиг вверх на 3м
            speed: 2.5,
        }
    }
}

impl DoorComponent {
    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
    }
}
```

Анимация створок в `on_fixed_update`:
```rust
world.inner.each_entity::<(&mut DoorComponent, &mut Transform)>(|_e, (door, transform)| {
    let target_progress = if door.is_open { 1.0 } else { 0.0 };
    if (door.progress - target_progress).abs() > 1e-4 {
        door.progress = if door.is_open {
            (door.progress + door.speed * fixed_dt).min(1.0)
        } else {
            (door.progress - door.speed * fixed_dt).max(0.0)
        };

        // Двери ОБЯЗАНЫ иметь RigidBodyComponent::KinematicPositionBased
        // Смещение Transform автоматически передастся в физику
        let base_pos = glam::Vec3::ZERO; // или начальная позиция
        let open_vec = glam::Vec3::from(door.open_offset);
        transform.position = base_pos.lerp(base_pos + open_vec, door.progress);
    }
});
```

---

### Рецепт 5: Триггеры и опасные зоны (Тесла-ворота, SCP-914)

```rust
#[derive(Component, Default)]
pub struct TeslaHazardZone;

// В GamePlugin::on_collision:
fn on_collision(&mut self, world: &World, event: &PhysicsCollisionEvent) {
    let Some(player_id) = self.player_entity else { return; };

    // Проверяем, участвует ли наш игрок в событии
    if let Some(other_entity_id) = event.other_entity(player_id) {
        let other = world.inner.entity_from_id(other_entity_id);

        if other.has(TeslaHazardZone::id()) {
            match event.event_type {
                CollisionEventType::Started => {
                    log::warn!("ИГРОК ВОШЕЛ В ЗОНУ ТЕСЛА-ВОРОТ! МГНОВЕННАЯ СМЕРТЬ!");
                    world.inner.entity_from_id(player_id).try_get::<&mut HealthComponent>(|hp| {
                        hp.current = 0.0;
                    });
                }
                CollisionEventType::Stopped => {
                    log::info!("Игрок покинул опасную зону");
                }
            }
        }
    }
}
```

---

### Рецепт 6: Загрузка комнат с TriMesh коллизиями

Чтобы заспавнить коридор или шлюз с физическими стенами и полом:

```rust
// 1. Загружаем и регистрируем геометрию в физическом кэше (1 раз при старте уровня)
let room_asset_id = asset::AssetId::from_path("rooms/corridor_heavy.bin");
if let Some(model) = asset_server.get::<shared::BakedModel>(room_asset_id) {
    physics.register_baked_model(room_asset_id, &model);

    // 2. Спавним комнату с генерацией TriMesh коллизий на все стены и пол
    let room_entity = ecs::spawn_scene_with_colliders(
        world,
        room_asset_id,
        &model,
        glam::Vec3::new(0.0, 0.0, 0.0),
        true, // generate_colliders = true
    );
}
```

---

## 7. Справочник API ввода (`GameInput`)

| Метод | Описание |
| :--- | :--- |
| `input.is_key_down(KeyCode)` | Клавиша удерживается в текущем кадре |
| `input.is_key_just_pressed(KeyCode)` | Клавиша была нажата именно в этом кадре |
| `input.is_mouse_down(MouseButton)` | Кнопка мыши зажата |
| `input.is_mouse_just_pressed(MouseButton)` | Одиночный клик мыши |
| `input.is_jump()` | Нажатие клавиши `Space` |
| `input.is_sprint()` | Зажаты `ShiftLeft` или `ShiftRight` |
| `input.is_crouch()` | Зажаты `ControlLeft` или `KeyC` |
| `input.is_interact()` | Нажатие клавиши `KeyE` |
| `input.is_shoot()` | Зажата ЛКМ |
| `input.is_shoot_just_pressed()` | Одиночный выстрел на ЛКМ |
| `input.is_aim()` | Зажата ПКМ |
| `input.is_reload()` | Нажатие клавиши `KeyR` |
| `input.move_axis` | Вектор WASD: `x` (-1..+1: A/D), `y` (-1..+1: S/W) |
| `input.look_delta` | Дельта движения мыши `(dx, dy)` за кадр |