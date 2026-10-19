
use bevy::prelude::*;
use crate::*;

pub(crate) struct F9DebugPlugin;

impl Plugin for F9DebugPlugin {
    fn build(&self, app: &mut App) {
        if let Ok(path) = std::env::var("CAPTURE_SCREENSHOT_TO") {
            let after = std::env::var("CAPTURE_SCREENSHOT_AFTER")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(8.0);
            app.insert_resource(PendingCapture { countdown: after, path })
                .add_systems(Update, capture_screenshot_when_due);
        }
        if std::env::var("EDITOR").is_ok() {
            app.init_resource::<F9SaveToast>()
                .add_systems(Startup, spawn_f9_toast_ui)
                .add_systems(
                    Update,
                    (
                        nudge_player_model_tuning.run_if(debug_keys_allowed),
                        save_player_model_tuning.run_if(debug_keys_allowed),
                        tick_f9_toast,
                    ),
                );
        }
    }
}

pub(crate) fn round4(v: f32) -> f32 {
    (v * 1.0e4).round() / 1.0e4
}

pub(crate) fn ron_f32(v: f32) -> String {
    let s = format!("{}", round4(v));
    if s.contains('.') { s } else { format!("{s}.0") }
}

#[derive(Resource, Default)]
pub(crate) struct F9SaveToast {
    text: String,
    ttl: f32,
    is_error: bool,
}

impl F9SaveToast {
    pub(crate) fn ok(&mut self, msg: impl Into<String>) {
        self.text = msg.into();
        self.ttl = 2.5;
        self.is_error = false;
    }
    pub(crate) fn err(&mut self, msg: impl Into<String>) {
        self.text = msg.into();
        self.ttl = 4.0;
        self.is_error = true;
    }

    pub(crate) fn ok_also(&mut self, standalone: &str, extra: &str) {
        if self.ttl > 0.0 && !self.is_error {
            self.text.push_str(extra);
            self.ttl = 2.5;
        } else {
            self.ok(standalone);
        }
    }
}

#[derive(Component)]
struct F9ToastText;

fn spawn_f9_toast_ui(mut commands: Commands) {
    commands.spawn((
        Text::new(""),
        TextFont { font_size: FontSize::Px(18.0), ..default() },
        TextColor(Color::srgb(0.6, 1.0, 0.6)),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(48.0),
            left: Val::Percent(50.0),
            margin: UiRect::left(Val::Px(-160.0)),
            width: Val::Px(320.0),
            padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.65)),
        Visibility::Hidden,
        F9ToastText,
    ));
}

fn tick_f9_toast(
    time: Res<Time>,
    mut toast: ResMut<F9SaveToast>,
    mut q: Query<(&mut Text, &mut Visibility, &mut TextColor), With<F9ToastText>>,
) {
    let Ok((mut text, mut vis, mut color)) = q.single_mut() else {
        return;
    };
    if toast.ttl > 0.0 {
        toast.ttl -= time.delta_secs();
        if text.0 != toast.text {
            text.0 = toast.text.clone();
        }
        color.0 = if toast.is_error {
            Color::srgb(1.0, 0.6, 0.5)
        } else {
            Color::srgb(0.6, 1.0, 0.6)
        };
        *vis = Visibility::Inherited;
    } else if *vis != Visibility::Hidden {
        *vis = Visibility::Hidden;
    }
}

#[derive(Resource)]
struct PendingCapture {
    countdown: f32,
    path: String,
}

fn capture_screenshot_when_due(
    time: Res<Time>,
    mut commands: Commands,
    pending: Option<ResMut<PendingCapture>>,
) {
    let Some(mut pending) = pending else { return };
    pending.countdown -= time.delta_secs();
    if pending.countdown > 0.0 {
        return;
    }
    let path = pending.path.clone();
    commands
        .spawn(bevy::render::view::screenshot::Screenshot::primary_window())
        .observe(bevy::render::view::screenshot::save_to_disk(path.clone()));
    commands.remove_resource::<PendingCapture>();
    info!("[capture] wrote {path}");
}
