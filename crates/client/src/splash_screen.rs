
use bevy::prelude::*;
use crate::*;

pub(crate) const SPLASH_SCREEN_DURATION_SECS: f32 = 2.0;

#[derive(Component)]
pub(crate) struct SplashScreen(Timer);

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
