# Спецификация и техническая документация IOR Engine

Данный документ представляет собой строгое техническое описание архитектуры, API и подсистем IOR Engine. Предназначен для разработчиков прикладной логики (геймплейных программистов).

---

## 1. Архитектура и модель выполнения кадра

Архитектура построена на разделении данных и логики посредством **Data-Driven ECS** (`flecs_ecs`), физического бэкенда симуляции твердых тел и кинематики (`rapier3d`), а также модульной системы прикладных плагинов (`GamePlugin`).

### 1.1. Временная шкала кадра (Frame Execution Timeline)

IOR Engine разделяет симуляцию на фиксированные шаги физики (**Fixed Update**) и переменные шаги рендера (**Variable Update**).

```text
====================================== НАЧАЛО КАДРА (dt) ======================================
  1. Опрос ввода ОС (Winit WindowEvents -> GameInput).
  2. Накопление времени: physics_accumulator += min(dt, 0.05).

  ┌─── ФИКСИРОВАННЫЙ ЦИКЛ СИМУЛЯЦИИ (пока accumulator >= 1/60 c, максимум 3 подшага):
  │     1. physics.sync_from_ecs(&world)
  │        - Передача измененных поз кинематических и статических тел в Rapier.
  │        - Синхронизация форм и параметров коллайдеров при их мутации в ECS.
  │     2. physics.step(fixed_dt)
  │        - Выполнение узкого/широкого фазового анализа Rapier, решение контактов и CCD.
  │     3. physics.sync_to_ecs(&world)
  │        - Передача поз динамических твердых тел обратно в Transform / GlobalTransform.
  │     4. plugin.on_collision(&world, event)
  │        - Диспетчеризация очереди событий коллизий (Started / Stopped) и сенсоров.
  │     5. plugin.on_fixed_update(&world, &mut physics, &input, fixed_dt)
  │        - Кинематический контроллер персонажей (KCC), расчет гравитации, движение платформ.
  │     6. physics_accumulator -= fixed_dt
  └───

  3. plugin.on_update(&world, &mut physics, &input, dt)
     - Обработка дельт мыши, хитскан-запросы, таймеры, пользовательская логика.
  4. world.progress()
     - Исполнение нативных систем Flecs ECS.
  5. plugin.on_late_update(&world, &mut physics, &input, dt)
     - Пост-обработка позиционирования зависимых узлов.
  6. input.clear_frame_deltas()
     - Сброс мгновенных дельт мыши (`look_delta`) и очередей нажатий текущего кадра.
  7. world.update_transforms()
     - Вычисление и распространение локальных матриц Transform в GlobalTransform по иерархии.
  8. physics.update_render_transforms(&world, alpha)
     - Интерполяция поз между предыдущим и текущим шагами физики:
       Pos = lerp(prev, curr, alpha), Rot = slerp(prev, curr, alpha).
  9. Подготовка дескрипторов рендера и передача буферов в GPU.
====================================== КОНЕЦ КАДРА ============================================
```

---

## 2. Модуль прикладной логики (`GamePlugin`)

Точкой входа для реализации игровых систем является трейт `GamePlugin`.

```rust
pub trait GamePlugin: Send + Sync + 'static {
    /// Инициализация постоянных структур данных при создании инстанса движка.
    fn init(&mut self, world: &World);

    /// Вызывается при переходе сцены в состояние Play / Simulate.
    fn on_start(&mut self, world: &World, physics: &mut PhysicsWorld) {}

    /// Вызывается при остановке симуляции и возврате в режим редактирования.
    fn on_stop(&mut self, world: &World);

    /// Обработка физических контактов и пересечений сенсоров (вызывается на тике физики).
    fn on_collision(&mut self, world: &World, event: &PhysicsCollisionEvent) {}

    /// Логика, привязанная к фиксированному шагу симуляции (детерминированный fixed_dt).
    fn on_fixed_update(
        &mut self,
        world: &World,
        physics: &mut PhysicsWorld,
        input: &GameInput,
        fixed_dt: f32,
    ) {}

    /// Основной цикл обновления прикладной логики (переменный dt).
    fn on_update(
        &mut self,
        world: &World,
        physics: &mut PhysicsWorld,
        input: &GameInput,
        dt: f32,
    ) {}

    /// Пост-обработка кадра после вычисления систем ECS.
    fn on_late_update(
        &mut self,
        world: &World,
        physics: &mut PhysicsWorld,
        input: &GameInput,
        dt: f32,
    ) {}
}
```

---

## 3. Базовые компоненты ядра (Core Components)

Все базовые компоненты движка являются сериализуемыми, поддерживают типобезопасную модификацию через ECS и не требуют ручной регистрации прикладным кодом.

### 3.1. Пространственные компоненты (`Transform`, `GlobalTransform`)

```rust
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    pub position: glam::Vec3,
    pub rotation: glam::Quat,
    pub scale: glam::Vec3,
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Default)]
pub struct GlobalTransform(pub glam::Mat4);
```
* **Правило иерархии:** При наличии родительской сущности (`child_of`) значение `Transform` интерпретируется как локальное смещение относительно родителя. `GlobalTransform` представляет абсолютную 4x4 матрицу преобразования в мировом пространстве.

### 3.2. Компоненты рендеринга (`CameraComponent`, `ModelComponent`, `LightComponent`)

```rust
#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CameraComponent {
    pub fovy: f32,             // Угол вертикального поля зрения в градусах
    pub znear: f32,            // Ближняя плоскость отсечения
    pub zfar: f32,             // Дальняя плоскость отсечения
    pub is_active: bool,       // Флаг активности
    pub priority: i32,         // Приоритет в очереди рендера (меньший рендерится раньше)
    pub culling_mask: u32,     // Битовая маска видимых слоев RenderLayers
    pub clear_depth: bool,     // Флаг сброса буфера глубины перед отрисовкой камеры
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct ModelComponent {
    pub id: asset::AssetId,    // Хеш-идентификатор запеченной модели (.bin)
    pub mesh_index: usize,     // Индекс сабмеша в структуре BakedModel
    pub render_layer: u32,     // Битовый слой принадлежности объекта
}

#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct LightComponent {
    pub light_type: LightType, // Directional = 0, Point = 1, Spot = 2
    pub color: [f32; 3],       // RGB [0.0 .. 1.0]
    pub intensity: f32,        // Сила излучения
    pub range: f32,            // Радиус действия затухания
    pub inner_angle: f32,      // Внутренний угол конуса в радианах (Spot)
    pub outer_angle: f32,      // Внешний угол конуса в радианах (Spot)
}
```

### 3.3. Физические компоненты (`RigidBodyComponent`, `ColliderComponent`, `CharacterControllerComponent`)

```rust
#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RigidBodyComponent {
    pub body_type: BodyType,       // Dynamic, Fixed, KinematicPositionBased, KinematicVelocityBased
    pub mass: f32,                 // Масса в килограммах
    pub linear_damping: f32,       // Коэффициент линейного сопротивления среды
    pub angular_damping: f32,      // Коэффициент углового сопротивления среды
    pub gravity_scale: f32,        // Множитель гравитации (1.0 = стандартная)
    pub ccd_enabled: bool,         // Непрерывное обнаружение коллизий (против туннелирования)
    pub lock_rotations: bool,      // Блокировка вращения по всем осям
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ColliderComponent {
    pub shape_type: ColliderShapeType, // Cuboid, Sphere, Capsule, Cylinder, TriMesh, ConvexHull
    pub size: [f32; 3],                // Полуразмеры (Cuboid: [hx, hy, hz], Capsule/Cyl: [r, hh, _])
    pub friction: f32,                 // Коэффициент сухого трения
    pub restitution: f32,              // Коэффициент упругости (0.0 = нет отскока)
    pub is_sensor: bool,               // Режим триггера (прохождение без твердого контакта)
    pub offset_position: [f32; 3],     // Локальное смещение центра формы относительно тела
    pub offset_rotation: [f32; 4],     // Локальный поворот формы [x, y, z, w]
    pub collision_memberships: u32,    // Битовая маска принадлежности к группам
    pub collision_filters: u32,        // Битовая маска взаимодействующих групп
    pub generate_collision_events: bool, // Генерация событий в PhysicsCollisionEvent
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CharacterControllerComponent {
    pub autostep_max_height: f32,      // Максимальная высота препятствия для зашагивания (м)
    pub autostep_min_width: f32,       // Минимальная ширина ступени (м)
    pub max_slope_climb_angle: f32,    // Предельный угол подъема по уклону в градусах
    pub min_slope_slide_angle: f32,    // Угол начала автоматического соскальзывания в градусах
    pub snap_to_ground_distance: f32,  // Дистанция прижима к поверхности при спусках (м)
    pub enable_autostep: bool,
    pub enable_snap_to_ground: bool,
    pub slide: bool,                   // Разрешение скольжения вдоль препятствий
}
```

---

## 4. Пользовательские компоненты и рефлексия (`Inspectable`)

Для создания прикладных компонентов с автоматической поддержкой отображения в интерфейсе инспектора, валидации диапазонов и сохранения в RON-файлы используется макрос `Inspectable`.

```rust
use ecs::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Component, Inspectable, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[component(category = "Combat")]
pub struct WeaponStats {
    #[inspect(range = 0.0..1000.0, speed = 1.0)]
    pub damage: f32,

    #[inspect(range = 0.01..2.0, speed = 0.01)]
    pub fire_rate: f32,

    #[inspect(color)]
    pub muzzle_flash_color: [f32; 3],

    #[inspect(skip)]
    pub time_since_last_shot: f32,
}

impl Default for WeaponStats {
    fn default() -> Self {
        Self {
            damage: 25.0,
            fire_rate: 0.1,
            muzzle_flash_color: [1.0, 0.8, 0.2],
            time_since_last_shot: 0.0,
        }
    }
}
```

### Спецификация атрибутов `#[inspect(...)]`:
| Атрибут | Назначение |
| :--- | :--- |
| `#[component(category = "Имя")]` | Задает заголовок подгруппы компонента в меню инспектора редактора. |
| `#[inspect(range = MIN..MAX)]` | Генерирует интерфейсный слайдер с жестким ограничением диапазона. |
| `#[inspect(speed = STEP)]` | Задает шаг изменения числового значения при перетаскивании мышью. |
| `#[inspect(color)]` | Интерпретирует тип `[f32; 3]` как RGB-цвет и отображает палитру выбора. |
| `#[inspect(skip)]` | Исключает поле из инспектора редактора (сохраняя сериализацию). |

---

## 5. Пространственные запросы и физический мир (`PhysicsWorld`)

### 5.1. Маски фильтрации коллизий (`CollisionLayer`)

Фильтрация контактов и запросов рейкастинга строится на битовых масках `u32`:

```rust
use engine::physics::CollisionLayer;

pub struct CollisionLayer;
impl CollisionLayer {
    pub const NONE: u32         = 0;
    pub const WORLD: u32        = 1 << 0; // Статическое окружение (стены, пол, потолок)
    pub const ACTOR: u32        = 1 << 1; // Управляемые сущности, персонажи, хитбоксы
    pub const INTERACTABLE: u32 = 1 << 2; // Интерактивные триггеры, кнопки, консоли
    pub const SENSOR_ZONE: u32  = 1 << 3; // Зоны событий (триггеры объема, ловушки)
    pub const DYNAMIC_PROP: u32 = 1 << 4; // Физические динамические объекты
}
```

### 5.2. Лучевые запросы (Raycasting)

```rust
let filter = RaycastFilter {
    exclude_entity: Some(player_entity_id), // Игнорировать тело инициатора
    ignore_sensors: true,                   // Не пересекать триггерные объемы
    solid: true,                            // Считать коллизию внутри геометрии пересечением
    groups: Some(InteractionGroups::new(
        Group::ALL,
        Group::from_bits_truncate(CollisionLayer::WORLD | CollisionLayer::ACTOR),
    )),
};

if let Some(hit) = physics.cast_ray(origin, direction, max_distance, filter) {
    // hit.entity   -> u64 идентификатор сущности в ECS
    // hit.point    -> glam::Vec3 абсолютная мировая координата пересечения
    // hit.normal   -> glam::Vec3 нормаль поверхности в точке контакта
    // hit.distance -> f32 метрическая дистанция от origin до point
}
```

### 5.3. Кинематический контроллер персонажа (KCC)

`move_character_controller` производит итеративное вычисление вектора перемещения с учетом автостепа лестниц, трения, скольжения по наклонным плоскостям и препятствиям, исключая коллайдер самого персонажа из broad-phase запроса.

```rust
let movement = physics.move_character_controller(
    entity_id,
    &character_controller_component,
    desired_translation_vector,
    fixed_dt,
);

// movement.translation -> результирующий вектор безопасного смещения
// movement.is_grounded  -> флаг нахождения на опорной поверхности
```

---

## 6. Многопроходный рендеринг и слои (`RenderLayers`)

IOR Engine поддерживает отрисовку произвольного количества камер в одном кадре с раздельным сбросом глубины.

```rust
use ecs::RenderLayers;

impl RenderLayers {
    pub const DEFAULT: u32   = 1 << 0; // Слой 1: Основная геометрия сцены
    pub const VIEWMODEL: u32 = 1 << 1; // Слой 2: Оверлей от 1-го лица (оружие, руки)
    pub const UI_3D: u32     = 1 << 2; // Слой 3: Мировые голографические интерфейсы
    pub const ALL: u32       = u32::MAX;
}
```

### Схема реализации Viewmodel без клиппинга геометрии:
1. **Основная камера (World Camera):**
   * `priority: 0`
   * `culling_mask: RenderLayers::DEFAULT`
   * `clear_depth: false`
   * Рендерит геометрию уровня, динамические пропы и других акторов с широким углом обзора (`fovy: 85.0`, `znear: 0.1`).
2. **Камера оверлея (Viewmodel Camera):**
   * Является дочерней к основной камере.
   * `priority: 1` (рендерится строго поверх результатов первой камеры).
   * `culling_mask: RenderLayers::VIEWMODEL` (обрабатывает только меши оружия и рук).
   * `clear_depth: true` (**очищает Z-буфер перед проходом**).
   * Фиксированный угол обзора (`fovy: 54.0`, `znear: 0.01`). Меши гарантированно не пересекаются с мировыми полигонами.

---

## 7. Подсистема префабов (`Prefab`)

Префаб представляет собой сериализованный снимок дерева сущностей с сохранением компонентного состава и относительных локальных трансформаций.

### 7.1. Структура связи (`PrefabRef`)
Экземпляры префаба на сцене маркируются системным компонентом:
```rust
#[derive(Component, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrefabRef {
    pub path: String, // Относительный путь к файлу шаблона (например, "prefabs/turret.prefab")
}
```
* **Инвариант целостности:** Внутри самого файла шаблона значение `prefab_ref` всегда принудительно устанавливается в `None`, исключая рекурсивную циклическую зависимость.

### 7.2. Программный API работы с префабами
```rust
// Инстанцирование префаба в заданной точке с получением корневого Entity ID
let instance_id = world.spawn_prefab(
    "prefabs/door_blast.prefab",
    glam::Vec3::new(10.0, 0.0, -5.0),
    glam::Quat::IDENTITY,
)?;

// Сохранение произвольного поддерева сущности в файл префаба
world.save_entity_as_prefab(root_entity_id, "res/prefabs/door_blast.prefab")?;

// Горячая синхронизация: обновление всех инстансов префаба на активной сцене 
// с сохранением их уникальных координат, масштаба, поворота и имен
world.reload_prefab_instances("res/prefabs/door_blast.prefab");
```

---

## 8. Шаблоны реализации прикладных механик (Code Recipes)

### 8.1. Полноценный контроллер перемещения от первого лица (FPS Controller)

Компонент состояния контроллера:
```rust
use ecs::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Component, Inspectable, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[component(category = "Controller")]
pub struct KinematicMovementState {
    #[inspect(range = 1.0..20.0, speed = 0.1)]
    pub walk_speed: f32,
    #[inspect(range = 1.0..30.0, speed = 0.1)]
    pub sprint_speed: f32,
    #[inspect(range = 1.0..10.0, speed = 0.1)]
    pub crouch_speed: f32,
    #[inspect(range = 1.0..15.0, speed = 0.1)]
    pub jump_impulse: f32,
    #[inspect(range = 1.0..50.0, speed = 0.5)]
    pub gravity: f32,

    #[inspect(skip)] pub vertical_velocity: f32,
    #[inspect(skip)] pub is_grounded: bool,
    #[inspect(skip)] pub is_crouching: bool,
}

impl Default for KinematicMovementState {
    fn default() -> Self {
        Self {
            walk_speed: 4.5,
            sprint_speed: 8.0,
            crouch_speed: 2.0,
            jump_impulse: 5.5,
            gravity: 19.62,
            vertical_velocity: 0.0,
            is_grounded: false,
            is_crouching: false,
        }
    }
}
```

Функция сборки сущности игрока:
```rust
use ecs::{BodyType, CameraComponent, CharacterControllerComponent, ColliderComponent, 
          ColliderShapeType, RigidBodyComponent, Transform, World};

pub fn build_first_person_actor(world: &World, origin: glam::Vec3) -> (u64, u64) {
    let actor_id = world.spawn_empty("Actor_Root");
    world.add_transform(actor_id, Transform::from_translation(origin));

    // 1. Кинематическое твердое тело для интеграции в солвер Rapier
    world.add_rigid_body(actor_id, RigidBodyComponent {
        body_type: BodyType::KinematicPositionBased,
        lock_rotations: true,
        ccd_enabled: true,
        ..Default::default()
    });

    // 2. Вертикальная капсула хитбокса (радиус 0.4м, высота 1.8м)
    world.add_collider(actor_id, ColliderComponent {
        shape_type: ColliderShapeType::Capsule,
        size: [0.4, 1.8, 0.4],
        friction: 0.0, // Исключает трение о вертикальные препятствия
        collision_memberships: engine::physics::CollisionLayer::ACTOR,
        ..Default::default()
    });

    // 3. Параметры KCC
    world.add_character_controller(actor_id, CharacterControllerComponent {
        autostep_max_height: 0.35,
        max_slope_climb_angle: 45.0,
        min_slope_slide_angle: 45.0,
        snap_to_ground_distance: 0.3,
        enable_autostep: true,
        enable_snap_to_ground: true,
        slide: true,
    });

    world.inner.entity_from_id(actor_id).set(KinematicMovementState::default());

    // 4. Дочерняя камера (глаза) со смещением по оси Y
    let camera_id = world.spawn_child(actor_id, "Camera_Eyes");
    world.add_transform(camera_id, Transform::from_xyz(0.0, 1.65, 0.0));
    world.add_camera(camera_id, CameraComponent {
        fovy: 80.0,
        znear: 0.05,
        zfar: 500.0,
        priority: 0,
        culling_mask: ecs::RenderLayers::DEFAULT,
        clear_depth: false,
        is_active: true,
    });

    (actor_id, camera_id)
}
```

Обработка вращения головы в переменном шаге (`on_update`):
```rust
fn handle_mouse_look(world: &World, actor_id: u64, camera_id: u64, input: &engine::game::GameInput) {
    const SENSITIVITY: f32 = 0.0022;

    // Вращение тела по оси Y (Yaw)
    world.get_transform_mut(actor_id, |t| {
        t.rotation *= glam::Quat::from_rotation_y(-input.look_delta.x * SENSITIVITY);
    });

    // Наклон камеры по оси X (Pitch) с фиксацией в диапазоне [-89°, +89°]
    world.get_transform_mut(camera_id, |t| {
        let (_, mut pitch, _) = t.rotation.to_euler(glam::EulerRot::YXZ);
        pitch = (pitch - input.look_delta.y * SENSITIVITY).clamp(-89.0f32.to_radians(), 89.0f32.to_radians());
        t.rotation = glam::Quat::from_rotation_x(pitch);
    });

    // Рекурсивный пересчет GlobalTransform дочерней цепочки
    world.update_entity_and_children_transforms(actor_id);
}
```

Исполнение кинематики в детерминированном шаге (`on_fixed_update`):
```rust
fn step_actor_kinematics(
    world: &World,
    physics: &mut engine::physics::PhysicsWorld,
    actor_id: u64,
    input: &engine::game::GameInput,
    fixed_dt: f32,
) {
    let entity = world.inner.entity_from_id(actor_id);
    let kcc = entity.try_get::<&CharacterControllerComponent>(|k| *k).unwrap();
    let rotation = entity.try_get::<&Transform>(|t| t.rotation).unwrap();

    entity.try_get::<&mut KinematicMovementState>(|state| {
        // Динамическое приседание с изменением формы коллайдера
        let crouch_requested = input.is_crouch();
        if crouch_requested != state.is_crouching {
            state.is_crouching = crouch_requested;
            let height = if state.is_crouching { 0.9 } else { 1.8 };
            let offset_y = if state.is_crouching { -0.45 } else { 0.0 };

            world.get_collider_mut(actor_id, |col| {
                col.size[1] = height;
                col.offset_position[1] = offset_y;
            });
        }

        // Выбор скорости
        let speed = if state.is_crouching {
            state.crouch_speed
        } else if input.is_sprint() {
            state.sprint_speed
        } else {
            state.walk_speed
        };

        // Базис направления в плоскости XZ
        let forward = (rotation * glam::Vec3::NEG_Z).with_y(0.0).normalize_or_zero();
        let right = (rotation * glam::Vec3::X).with_y(0.0).normalize_or_zero();
        let planar_direction = (forward * input.move_axis.y + right * input.move_axis.x).normalize_or_zero();

        // Расчет вертикальной компоненты
        if state.is_grounded {
            if input.is_jump() && !state.is_crouching {
                state.vertical_velocity = state.jump_impulse;
                state.is_grounded = false;
            } else {
                state.vertical_velocity = -1.5; // Прижимной импульс для ступеней
            }
        } else {
            state.vertical_velocity = (state.vertical_velocity - state.gravity * fixed_dt).max(-45.0);
        }

        let desired_delta = (planar_direction * speed + glam::Vec3::Y * state.vertical_velocity) * fixed_dt;

        // Вызов пространственного солвера движения
        let result = physics.move_character_controller(actor_id, &kcc, desired_delta, fixed_dt);

        // Сброс восходящей скорости при ударе головой о потолок
        if state.vertical_velocity > 0.0 && result.translation.y < desired_delta.y * 0.5 {
            state.vertical_velocity = 0.0;
        }

        state.is_grounded = result.is_grounded;

        // Применение смещения в ECS
        world.get_transform_mut(actor_id, |t| {
            t.position += result.translation;
        });
    });
}
```

---

### 8.2. Хитскан-запрос (Оружие / Трассировка выстрела)

```rust
pub struct HitscanResult {
    pub target_entity: u64,
    pub impact_point: glam::Vec3,
    pub surface_normal: glam::Vec3,
}

pub fn execute_hitscan(
    world: &World,
    physics: &engine::physics::PhysicsWorld,
    shooter_entity: u64,
    range: f32,
) -> Option<HitscanResult> {
    let (cam_pos, cam_rot, _) = world.get_active_game_camera()?;
    let ray_direction = cam_rot * glam::Vec3::NEG_Z;

    let filter = RaycastFilter {
        exclude_entity: Some(shooter_entity),
        ignore_sensors: true,
        solid: true,
        groups: Some(engine::physics::CollisionLayer::weapon_raycast_groups()),
    };

    physics.cast_ray(cam_pos, ray_direction, range, filter).map(|hit| {
        HitscanResult {
            target_entity: hit.entity,
            impact_point: hit.point,
            surface_normal: hit.normal,
        }
    })
}
```

---

### 8.3. Кинематические препятствия (Автоматические гермозатворы)

Компонент кинематической двери:
```rust
#[derive(Component, Inspectable, Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[component(category = "Mechanisms")]
pub struct SlidingDoorMechanism {
    pub is_open: bool,
    pub progress: f32,
    pub travel_distance: glam::Vec3,
    pub velocity: f32,
}

impl Default for SlidingDoorMechanism {
    fn default() -> Self {
        Self {
            is_open: false,
            progress: 0.0,
            travel_distance: glam::Vec3::new(0.0, 3.0, 0.0),
            velocity: 2.0,
        }
    }
}
```

Функция обновления позиционирования (`on_fixed_update`):
```rust
pub fn update_sliding_doors(world: &World, fixed_dt: f32) {
    world.inner.each_entity::<(&mut SlidingDoorMechanism, &mut Transform)>(|_entity, (door, transform)| {
        let target = if door.is_open { 1.0 } else { 0.0 };
        if (door.progress - target).abs() > 1e-4 {
            let step = door.velocity * fixed_dt;
            door.progress = if door.is_open {
                (door.progress + step).min(1.0)
            } else {
                (door.progress - step).max(0.0)
            };

            // Двери должны иметь RigidBodyComponent::KinematicPositionBased.
            // Модификация локального Transform автоматически передается в Rapier 
            // через body.set_next_kinematic_position на следующем шаге sync_from_ecs.
            transform.position = door.travel_distance * door.progress;
        }
    });
}
```

---

### 8.4. Обработка триггеров и сенсоров объема (`on_collision`)

```rust
#[derive(Component, Default)]
pub struct LethalVolumeSensor;

impl engine::game::GamePlugin for SimulationPlugin {
    fn on_collision(&mut self, world: &World, event: &engine::physics::PhysicsCollisionEvent) {
        // Фильтруем только факт первичного входа в сенсор
        if event.event_type != engine::physics::CollisionEventType::Started || !event.is_sensor {
            return;
        }

        let Some(player_id) = self.local_player_id else { return; };

        // Использование встроенного хелпера проверки участия сущности
        if let Some(hazard_entity_id) = event.other_entity(player_id) {
            let hazard_entity = world.inner.entity_from_id(hazard_entity_id);

            if hazard_entity.has(LethalVolumeSensor::id()) {
                log::warn!(
                    "Сущность #{} вошла в зону летального поражения #{}",
                    player_id,
                    hazard_entity_id
                );
                
                // Применение урона
                world.inner.entity_from_id(player_id).try_get::<&mut HealthComponent>(|hp| {
                    hp.current = 0.0;
                });
            }
        }
    }
}
```

---

### 8.5. Загрузка статических уровней с физической геометрией (`TriMesh`)

Для исключения падения акторов сквозь визуальные модели геометрии комнат, генерация коллайдеров и регистрация вершин производится пакетно при загрузке:

```rust
pub fn load_level_module(
    world: &World,
    physics: &mut engine::physics::PhysicsWorld,
    asset_server: &asset::AssetServer,
    asset_path: &str,
    world_origin: glam::Vec3,
) -> anyhow::Result<u64> {
    let asset_id = asset::AssetId::from_path(asset_path);

    // 1. Получение структуры запеченной модели
    let model = asset_server
        .get::<shared::BakedModel>(asset_id)
        .ok_or_else(|| anyhow::anyhow!("Asset not loaded in AssetServer: {}", asset_path))?;

    // 2. Регистрация сырых вершин мешей в кэше физического мира
    physics.register_baked_model(asset_id, &model);

    // 3. Рекурсивное создание сущностей сцены с автоматической генерацией TriMesh
    let root_entity_id = ecs::spawn_scene_with_colliders(
        world,
        asset_id,
        &model,
        world_origin,
        true, // generate_colliders = true
    );

    Ok(root_entity_id)
}
```