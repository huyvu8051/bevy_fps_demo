// --- Splash screen: OPEN DIY studio logo shown briefly at launch ---
// Self-contained: one component, one spawn helper (called once from
// setup(), crate::lib), one Update system ticking it down. Desktop/web
// only for now — the iOS CADisplayLink renderer (cadisplaylink module)
// draws its own HUD straight from ECS state via hand-written Metal code,
// not bevy_ui, so a plain bevy_ui node like this one is invisible there
// until someone ports it the same way HealthUi/AmmoUi were.

use bevy::prelude::*;
use crate::*;

const SPLASH_SCREEN_DURATION_SECS: f32 = 2.0;

#[derive(Component)]
pub(crate) struct SplashScreen(Timer);

// Called once from setup() right after the UI camera is spawned (so
// IsDefaultUiCamera already exists for this node to render on). Highest
// GlobalZIndex in the project on purpose — above the settings menu
// (STATIC_UI_Z_INDEX + 50) and the map-select overlay (+100), since this
// is meant to cover literally everything else during the first second.
pub(crate) fn spawn_splash_screen(commands: &mut Commands, asset_server: &AssetServer) {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                position_type: PositionType::Absolute,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            // Matches the logo art's own baked-in background (see
            // open_diy_logo_no_subtitle.svg's <rect>) so the letterboxed
            // edges (image is square, window usually isn't) blend in
            // instead of flashing black.
            BackgroundColor(Color::WHITE),
            GlobalZIndex(STATIC_UI_Z_INDEX + 1000),
            SplashScreen(Timer::from_seconds(SPLASH_SCREEN_DURATION_SECS, TimerMode::Once)),
        ))
        .with_children(|root| {
            root.spawn((
                ImageNode::new(asset_server.load("branding/open_diy_splash.png")),
                Node { width: Val::Percent(60.0), max_width: Val::Px(480.0), aspect_ratio: Some(1.0), ..default() },
            ));
        });
}

pub(crate) fn update_splash_screen(mut commands: Commands, time: Res<Time>, mut splash: Query<(Entity, &mut SplashScreen)>) {
    for (entity, mut splash) in &mut splash {
        splash.0.tick(time.delta());
        if splash.0.just_finished() {
            commands.entity(entity).despawn();
        }
    }
}
