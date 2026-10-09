
use bevy::asset::{AssetId, Assets, Handle};
use bevy::camera::visibility::{InheritedVisibility, Visibility};
use bevy::camera::{Camera, Projection, RenderTargetInfo};
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::{With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut, SystemState};
use bevy::image::Image;
use bevy::mesh::skinning::SkinnedMesh;
use bevy::light::{DirectionalLight, PointLight};
use bevy::pbr::{MeshMaterial3d, StandardMaterial};
use bevy::material::AlphaMode;
use bevy::mesh::{Mesh, Mesh3d};
use bevy::render::render_resource::TextureFormat;
use bevy::text::{FontSize, TextColor, TextFont};
use bevy::math::Vec3;
use bevy::transform::components::{GlobalTransform, Transform};
use bevy::ui::widget::Text;
use bevy::ui::{BackgroundColor, BorderColor, ComputedNode, ComputedStackIndex, Display, Node, OverflowAxis, UiGlobalTransform};

use shared::{NetPlayer, Zombie, ZombieKind};
use metal_renderer::snapshot::*;
use metal_renderer::extract::*;

const DEBUG_CIRCLE_SEGMENTS: usize = 24;



#[derive(Resource)]
pub struct ExtractQueries {
    fps_camera: SystemState<Q<'static, 'static, (&'static mut Camera, &'static mut Projection), With<crate::FpsCamera>>>,
    ui_camera: SystemState<Q<'static, 'static, &'static mut Camera, With<bevy::ui::IsDefaultUiCamera>>>,
    window: SystemState<Q<'static, 'static, &'static mut bevy::window::Window, With<bevy::window::PrimaryWindow>>>,
    camera: SystemState<(
        Q<'static, 'static, (&'static GlobalTransform, &'static crate::FpsCamera)>,
        Q<'static, 'static, &'static crate::Gun>,
        Res<'static, crate::ScopeZoom>,
    )>,
    scope: SystemState<(
        Q<'static, 'static, (&'static GlobalTransform, &'static bevy::camera::Camera, &'static bevy::camera::Projection), With<crate::ScopeCamera>>,
        Q<'static, 'static, (&'static GlobalTransform, &'static Transform), With<crate::ScopeLens>>,
    )>,
    zombies: SystemState<(
        Q<'static, 'static, (bevy::ecs::entity::Entity, &'static GlobalTransform, Option<&'static ZombieKind>), With<Zombie>>,
        Q<'static, 'static, &'static Children>,
        Q<'static, 'static, &'static SkinnedMesh>,
        Q<'static, 'static, &'static GlobalTransform>,
        Res<'static, Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>>,
    )>,
    remote_players: SystemState<(
        Q<'static, 'static, (bevy::ecs::entity::Entity, &'static GlobalTransform), (With<NetPlayer>, Without<crate::LocalPlayer>)>,
        Q<'static, 'static, &'static Children>,
        Q<'static, 'static, &'static SkinnedMesh>,
        Q<'static, 'static, &'static GlobalTransform>,
        Res<'static, Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>>,
    )>,
    point_lights: SystemState<Q<'static, 'static, (&'static PointLight, &'static GlobalTransform)>>,
    sun: SystemState<Q<'static, 'static, (&'static DirectionalLight, &'static GlobalTransform)>>,
    meshes_changed: SystemState<MeshExtractParams<'static, 'static, MeshChanged>>,
    meshes_any: SystemState<MeshExtractParams<'static, 'static, Without<SkinnedMesh>>>,
    meshes_removed: SystemState<bevy::ecs::lifecycle::RemovedComponents<'static, 'static, Mesh3d>>,
    ui_roots: SystemState<Q<'static, 'static, bevy::ecs::entity::Entity, With<crate::NativeUiRoot>>>,
    hitboxes: SystemState<(
        Res<'static, crate::ShowHitboxes>,
        Q<'static, 'static, (&'static Transform, &'static shared::ZombieHitboxBones), With<Zombie>>,
        Q<'static, 'static, &'static GlobalTransform>,
    )>,
    ui_nodes: SystemState<(
        GenericUiQuery<'static, 'static>,
        Res<'static, Assets<Image>>,
        ResMut<'static, DecodedTextureCache>,
    )>,
}

impl ExtractQueries {
    fn new(world: &mut bevy::ecs::world::World) -> Self {
        world.init_resource::<DecodedTextureCache>();
        Self {
            fps_camera: SystemState::new(world),
            ui_camera: SystemState::new(world),
            window: SystemState::new(world),
            camera: SystemState::new(world),
            scope: SystemState::new(world),
            zombies: SystemState::new(world),
            remote_players: SystemState::new(world),
            point_lights: SystemState::new(world),
            sun: SystemState::new(world),
            meshes_changed: SystemState::new(world),
            meshes_any: SystemState::new(world),
            meshes_removed: SystemState::new(world),
            ui_roots: SystemState::new(world),
            hitboxes: SystemState::new(world),
            ui_nodes: SystemState::new(world),
        }
    }
}

fn take_queries(world: &mut bevy::ecs::world::World) -> ExtractQueries {
    match world.remove_resource::<ExtractQueries>() {
        Some(queries) => queries,
        None => ExtractQueries::new(world),
    }
}

pub fn prime_camera_viewport(world: &mut bevy::ecs::world::World, width_pts: f32, height_pts: f32, scale_factor: f32) {
    let mut queries = take_queries(world);
    let mut query = queries.fps_camera.get_mut(world).expect("system param validation failed");
    let (w, h) = (width_pts.max(1.0), height_pts.max(1.0));

    let target_info = || {
        Some(RenderTargetInfo {
            physical_size: bevy::math::UVec2::new((w * scale_factor) as u32, (h * scale_factor) as u32),
            scale_factor,
        })
    };
    if let Ok((mut camera, mut projection)) = query.single_mut() {

        camera.computed.target_info = target_info();
        projection.update(w, h);
        camera.computed.clip_from_view = projection.get_clip_from_view();
    }

    for mut ui_camera in queries.ui_camera.get_mut(world).expect("system param validation failed").iter_mut() {
        ui_camera.computed.target_info = target_info();
    }

    if let Ok(mut window) = queries.window.get_mut(world).expect("system param validation failed").single_mut() {
        window.resolution.set(w, h);
    }
    world.insert_resource(queries);
}


fn find_first_skinned_mesh(
    root: bevy::ecs::entity::Entity,
    children_query: &Query<&Children>,
    skinned_query: &Query<&SkinnedMesh>,
) -> Option<bevy::ecs::entity::Entity> {
    let mut queue = std::collections::VecDeque::from([root]);
    while let Some(entity) = queue.pop_front() {
        if skinned_query.contains(entity) {
            return Some(entity);
        }
        if let Ok(children) = children_query.get(entity) {
            queue.extend(children.iter());
        }
    }
    None
}

fn joint_matrices_for(
    skinned: &SkinnedMesh,
    globals: &Query<&GlobalTransform>,
    inverse_bindposes_assets: &Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>,
) -> Vec<[[f32; 4]; 4]> {
    let Some(inverse_bindposes) = inverse_bindposes_assets.get(&skinned.inverse_bindposes) else {
        return Vec::new();
    };
    skinned
        .joints
        .iter()
        .zip(inverse_bindposes.iter())
        .map(|(&joint, inverse_bind)| {
            let world = globals.get(joint).map(GlobalTransform::to_matrix).unwrap_or_default();
            (world * *inverse_bind).to_cols_array_2d()
        })
        .collect()
}

pub fn extract_render_snapshot(
    world: &mut bevy::ecs::world::World,
    geometry_already_uploaded: impl Fn(AssetId<Mesh>) -> bool,
) -> RenderSnapshot {
    let mut snapshot = RenderSnapshot::default();

    let mut queries = take_queries(world);
    let (camera_query, gun_query, scope_zoom) = queries.camera.get(world).expect("system param validation failed");
    if let Some((transform, cam)) = camera_query.iter().next() {
        snapshot.camera = Some(CameraSnapshot {
            eye: transform.translation().to_array(),
            rotation: transform.rotation().to_array(),
            yaw: cam.yaw,
            pitch: cam.pitch,
            ads_blend: gun_query.iter().next().map(|g| g.ads_blend).unwrap_or(0.0),
            scope_zoom: scope_zoom.0,
            wall_raise: gun_query.iter().next().map(|g| g.wall_raise).unwrap_or(0.0),
        });
    }

    let (scope_camera_query, scope_lens_query) = queries.scope.get(world).expect("system param validation failed");
    if let Some((transform, camera, projection)) = scope_camera_query.iter().next() {
        let fov = match projection {
            bevy::camera::Projection::Perspective(p) => p.fov,
            _ => 0.0,
        };
        let (lens_world_pos, lens_local_rotation) = scope_lens_query
            .iter()
            .next()
            .map(|(global, local)| (global.translation().to_array(), local.rotation.to_array()))
            .unwrap_or((transform.translation().to_array(), [0.0, 0.0, 0.0, 1.0]));
        snapshot.scope_camera = Some(ScopeCameraSnapshot {
            eye: transform.translation().to_array(),
            rotation: transform.rotation().to_array(),
            active: camera.is_active,
            fov,
            lens_world_pos,
            lens_local_rotation,
        });
    }

    let (zombie_query, children_query, skinned_mesh_query, joint_globals, inverse_bindposes_assets) =
        queries.zombies.get(world).expect("system param validation failed");
    snapshot.skinned_actors = zombie_query
        .iter()
        .map(|(entity, transform, kind)| {
            let joint_matrices = find_first_skinned_mesh(entity, &children_query, &skinned_mesh_query)
                .and_then(|mesh_entity| skinned_mesh_query.get(mesh_entity).ok())
                .map(|skinned| joint_matrices_for(skinned, &joint_globals, &inverse_bindposes_assets))
                .unwrap_or_default();
            SkinnedRenderState {
                transform: transform.compute_transform(),
                joint_matrices,
                model: shared::zombie_kind_metal_model_name(kind.copied().unwrap_or_default()),
            }
        })
        .collect();

    const LOCAL_PLAYER_EXCLUSION_RADIUS_XZ: f32 = 1.0;
    let camera_eye_xz = snapshot.camera.as_ref().map(|c| bevy::math::Vec2::new(c.eye[0], c.eye[2]));
    let (remote_query, remote_children_query, remote_skinned_query, remote_joint_globals, remote_inverse_bindposes_assets) =
        queries.remote_players.get(world).expect("system param validation failed");
    snapshot.skinned_actors.extend(
        remote_query
        .iter()
        .map(|(entity, t)| (entity, t.compute_transform()))
        .filter(|(_, t)| match camera_eye_xz {
            Some(eye_xz) => {
                let pos_xz = bevy::math::Vec2::new(t.translation.x, t.translation.z);
                pos_xz.distance(eye_xz) > LOCAL_PLAYER_EXCLUSION_RADIUS_XZ
            }
            None => true,
        })
        .map(|(entity, transform)| {
            let joint_matrices = find_first_skinned_mesh(entity, &remote_children_query, &remote_skinned_query)
                .and_then(|mesh_entity| remote_skinned_query.get(mesh_entity).ok())
                .map(|skinned| joint_matrices_for(skinned, &remote_joint_globals, &remote_inverse_bindposes_assets))
                .unwrap_or_default();

            SkinnedRenderState { transform, joint_matrices, model: "soldier" }
        }),
    );

    let eye = snapshot.camera.as_ref().map(|c| Vec3::from_array(c.eye)).unwrap_or(Vec3::ZERO);
    snapshot.light = queries
        .point_lights
        .get(world)
        .expect("system param validation failed")
        .iter()
        .min_by(|(_, a), (_, b)| {
            a.translation().distance_squared(eye).total_cmp(&b.translation().distance_squared(eye))
        })
        .map(|(light, transform)| {
        let c = light.color.to_srgba();
        LightSnapshot {
            position: transform.translation().to_array(),
            color: [c.red, c.green, c.blue],
            range: light.range,
        }
    });

    snapshot.sun = queries.sun.get(world).expect("system param validation failed").iter().next().map(|(sun, transform)| {
        let c = sun.color.to_srgba();
        SunSnapshot { direction: transform.forward().to_array(), color: [c.red, c.green, c.blue] }
    });

    snapshot.map_msaa_on = world.resource::<crate::MapMsaaOn>().0;
    snapshot.shadow_quality = world.resource::<crate::ShadowQuality>().0;
    snapshot.light_quality = world.resource::<crate::LightQuality>().0;
    snapshot.point_light_quality = world.resource::<crate::PointLightQuality>().0;
    snapshot.render_resolution_scale = world.resource::<crate::RenderResolutionScale>().0;

    let mut cache = world.remove_resource::<StaticMeshCache>().unwrap_or_default();
    let mut used_texture_ids: std::collections::HashSet<AssetId<Image>> = std::collections::HashSet::new();
    let mut freed_any = false;

    for entity in queries
        .meshes_removed
        .get_mut(world)
        .expect("system param validation failed")
        .read()
        .collect::<Vec<_>>()
    {
        if let Some(slot) = cache.slot_of.remove(&entity) {
            cache.entries[slot].visible = false;
            cache.free_slots.push(slot);
            freed_any = true;
        }
        cache.awaiting_textures.remove(&entity);
    }

    {
        let (mesh_query, meshes, materials, images, mut texture_cache) =
            queries.meshes_changed.get_mut(world).expect("system param validation failed");
        for (entity, mesh3d, material3d, transform, visibility) in mesh_query.iter() {
            store_extracted_mesh(
                &mut cache, entity, mesh3d, material3d, transform, visibility,
                &meshes, &materials, &images, &mut texture_cache, &mut used_texture_ids,
                &geometry_already_uploaded,
            );
        }
    }

    let retry: Vec<bevy::ecs::entity::Entity> = cache.awaiting_textures.iter().copied().collect();
    if !retry.is_empty() {
        let (mesh_query, meshes, materials, images, mut texture_cache) =
            queries.meshes_any.get_mut(world).expect("system param validation failed");
        for entity in retry {
            let Ok((_, mesh3d, material3d, transform, visibility)) = mesh_query.get(entity) else {
                cache.awaiting_textures.remove(&entity);
                continue;
            };
            store_extracted_mesh(
                &mut cache, entity, mesh3d, material3d, transform, visibility,
                &meshes, &materials, &images, &mut texture_cache, &mut used_texture_ids,
                &geometry_already_uploaded,
            );
        }
    }

    if freed_any {
        let live: std::collections::HashSet<AssetId<Image>> = cache
            .entries
            .iter()
            .flat_map(|entry| [entry.texture.as_ref(), entry.normal_texture.as_ref()])
            .flatten()
            .map(|texture| texture.id)
            .collect();
        world.resource_mut::<DecodedTextureCache>().0.retain(|id, _| live.contains(id));
    }

    snapshot.static_meshes = cache.entries.clone();
    world.insert_resource(cache);

    snapshot.debug_lines = extract_hitbox_lines(world, &mut queries);
    snapshot.generic_ui_nodes = extract_generic_ui_nodes(world, &mut queries, &mut used_texture_ids);

    world.insert_resource(queries);
    snapshot
}


fn push_circle(out: &mut Vec<DebugLine>, centre: Vec3, axis_u: Vec3, axis_v: Vec3, radius: f32, color: [f32; 4]) {
    let point = |i: usize| {
        let angle = i as f32 / DEBUG_CIRCLE_SEGMENTS as f32 * std::f32::consts::TAU;
        centre + (axis_u * angle.cos() + axis_v * angle.sin()) * radius
    };
    for i in 0..DEBUG_CIRCLE_SEGMENTS {
        out.push(DebugLine { a: point(i).to_array(), b: point(i + 1).to_array(), color });
    }
}

fn push_sphere(out: &mut Vec<DebugLine>, centre: Vec3, radius: f32, color: [f32; 4]) {
    push_circle(out, centre, Vec3::X, Vec3::Y, radius, color);
    push_circle(out, centre, Vec3::X, Vec3::Z, radius, color);
    push_circle(out, centre, Vec3::Y, Vec3::Z, radius, color);
}

fn ring_axes(direction: Vec3) -> (Vec3, Vec3) {
    let reference = if direction.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
    let u = direction.cross(reference).normalize_or_zero();
    (u, direction.cross(u).normalize_or_zero())
}

fn push_capsule(out: &mut Vec<DebugLine>, a: Vec3, b: Vec3, radius: f32, color: [f32; 4]) {
    let Some(direction) = (b - a).try_normalize() else {
        push_sphere(out, a, radius, color);
        return;
    };
    let (u, v) = ring_axes(direction);
    push_circle(out, a, u, v, radius, color);
    push_circle(out, b, u, v, radius, color);
    for offset in [u, -u, v, -v] {
        out.push(DebugLine { a: (a + offset * radius).to_array(), b: (b + offset * radius).to_array(), color });
    }
    push_sphere(out, a, radius, color);
    push_sphere(out, b, radius, color);
}

fn push_cylinder(out: &mut Vec<DebugLine>, centre: Vec3, radius: f32, half_height: f32, color: [f32; 4]) {
    let top = centre + Vec3::Y * half_height;
    let bottom = centre - Vec3::Y * half_height;
    push_circle(out, top, Vec3::X, Vec3::Z, radius, color);
    push_circle(out, bottom, Vec3::X, Vec3::Z, radius, color);
    for offset in [Vec3::X, -Vec3::X, Vec3::Z, -Vec3::Z] {
        out.push(DebugLine { a: (top + offset * radius).to_array(), b: (bottom + offset * radius).to_array(), color });
    }
}

fn extract_hitbox_lines(world: &mut bevy::ecs::world::World, queries: &mut ExtractQueries) -> Vec<DebugLine> {
    let (show, zombies, globals) = queries.hitboxes.get(world).expect("system param validation failed");
    if !show.0 {
        return Vec::new();
    }

    const OUTER: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
    const HEAD: [f32; 4] = [1.0, 0.2, 0.2, 1.0];
    const TORSO: [f32; 4] = [1.0, 0.8, 0.0, 1.0];
    const LIMB: [f32; 4] = [0.2, 0.6, 1.0, 1.0];
    const JOINT: [f32; 4] = [0.0, 1.0, 0.0, 1.0];

    let mut out = Vec::new();
    for (transform, bones) in zombies.iter() {
        push_cylinder(
            &mut out,
            transform.translation,
            shared::ZOMBIE_OUTER_RADIUS,
            shared::ZOMBIE_OUTER_HEIGHT / 2.0,
            OUTER,
        );

        let bone_pos = |e: bevy::ecs::entity::Entity| globals.get(e).ok().map(GlobalTransform::translation);
        let mut joints = Vec::new();

        if let Some(head) = bones.head.and_then(bone_pos) {
            push_sphere(&mut out, head, shared::HEAD_HITBOX_RADIUS, HEAD);
            joints.push(head);
        }
        if let (Some(belly), Some(chest)) = (bones.belly.and_then(bone_pos), bones.chest.and_then(bone_pos)) {
            push_capsule(&mut out, belly, chest, shared::TORSO_HITBOX_RADIUS, TORSO);
            joints.push(belly);
            joints.push(chest);
        }
        for chain in [&bones.left_arm, &bones.right_arm, &bones.left_leg, &bones.right_leg] {
            for joint in chain.iter().filter_map(|j| j.and_then(bone_pos)) {
                joints.push(joint);
            }
            for pair in chain.windows(2) {
                if let (Some(a), Some(b)) = (pair[0].and_then(bone_pos), pair[1].and_then(bone_pos)) {
                    push_capsule(&mut out, a, b, shared::LIMB_HITBOX_RADIUS, LIMB);
                }
            }
        }
        for joint in joints {
            push_sphere(&mut out, joint, 0.04, JOINT);
        }
    }
    out
}

/// Every `TextFont` in this project uses `FontSize::Px`; viewport/rem-relative
/// sizes would need the viewport size to resolve, which this path doesn't have.
fn font_size_px(size: FontSize) -> f32 {
    match size {
        FontSize::Px(px) => px,
        other => other.eval(bevy::math::Vec2::ZERO, 16.0),
    }
}

type GenericUiQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Node,
        &'static ComputedNode,
        &'static ComputedStackIndex,
        &'static UiGlobalTransform,
        Option<&'static Children>,
        Option<&'static BackgroundColor>,
        Option<&'static BorderColor>,
        Option<(&'static Text, &'static TextColor, &'static TextFont)>,
        Option<&'static Visibility>,
        Option<&'static bevy::ui::widget::ImageNode>,
    ),
>;

fn intersect_clip_rect(accumulated: Option<[f32; 4]>, own: [f32; 4]) -> [f32; 4] {
    let Some(a) = accumulated else { return own };
    let x0 = a[0].max(own[0]);
    let y0 = a[1].max(own[1]);
    let x1 = (a[0] + a[2]).min(own[0] + own[2]);
    let y1 = (a[1] + a[3]).min(own[1] + own[3]);
    [x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0)]
}

fn walk_generic_ui_node(
    entity: bevy::ecs::entity::Entity,
    inherited_clip: Option<[f32; 4]>,
    inherited_visible: bool,
    query: &GenericUiQuery,
    images: &Assets<Image>,
    texture_cache: &mut DecodedTextureCache,
    used_texture_ids: &mut std::collections::HashSet<AssetId<Image>>,
    out: &mut Vec<GenericUiNode>,
) {
    let Ok((node, computed, stack_index, transform, children, background, border_color, text, visibility, image)) =
        query.get(entity)
    else {
        return;
    };
    // `Display::None` drops the whole subtree from layout, so stop here. `Visibility::Hidden`
    // only skips painting — layout still applies and a child may force itself back visible,
    // so that case keeps walking.
    if node.display == Display::None {
        return;
    }
    let visible = match visibility {
        Some(Visibility::Hidden) => false,
        Some(Visibility::Visible) => true,
        _ => inherited_visible,
    };
    let inv_scale = computed.inverse_scale_factor();
    let size = computed.size() * inv_scale;
    let affine = transform.affine();
    let (_, rotation, _) = affine.to_scale_angle_translation();
    let center = affine.translation * inv_scale;

    let background_color = background.map(|bg| {
        let c = bg.0.to_srgba();
        [c.red, c.green, c.blue, c.alpha]
    });
    let border = computed.border;
    let border_width = ((border.min_inset.x + border.min_inset.y + border.max_inset.x + border.max_inset.y) / 4.0) * inv_scale;
    let border_color = border_color.filter(|_| border_width > 0.0).map(|bc| {
        let c = bc.top.to_srgba();
        [c.red, c.green, c.blue, c.alpha]
    });
    let corner_radius = computed.border_radius.top_left * inv_scale;
    let text_snapshot = text.map(|(text, color, font)| {
        let c = color.0.to_srgba();
        GenericUiText { content: text.0.clone(), color: [c.red, c.green, c.blue, c.alpha], font_size: font_size_px(font.font_size) }
    });

    let image_snapshot = image
        .filter(|_| visible)
        .and_then(|image| decode_texture(&image.image, images, texture_cache, used_texture_ids));

    if visible
        && (background_color.is_some()
            || border_color.is_some()
            || text_snapshot.is_some()
            || image_snapshot.is_some())
    {
        out.push(GenericUiNode {
            center: center.to_array(),
            size: size.to_array(),
            rotation,
            background_color,
            border_color,
            border_width,
            corner_radius,
            text: text_snapshot,
            image: image_snapshot,
            clip_rect: inherited_clip,
            stack_index: stack_index.0,
        });
    }

    let clips_children = matches!(node.overflow.x, OverflowAxis::Clip | OverflowAxis::Hidden | OverflowAxis::Scroll)
        || matches!(node.overflow.y, OverflowAxis::Clip | OverflowAxis::Hidden | OverflowAxis::Scroll);
    let child_clip = if clips_children {
        let own_rect = [center.x - size.x / 2.0, center.y - size.y / 2.0, size.x, size.y];
        Some(intersect_clip_rect(inherited_clip, own_rect))
    } else {
        inherited_clip
    };

    let Some(children) = children else { return };
    for &child in children.iter() {
        walk_generic_ui_node(child, child_clip, visible, query, images, texture_cache, used_texture_ids, out);
    }
}

fn extract_generic_ui_nodes(
    world: &mut bevy::ecs::world::World,
    queries: &mut ExtractQueries,
    used_texture_ids: &mut std::collections::HashSet<AssetId<Image>>,
) -> Vec<GenericUiNode> {
    let roots: Vec<_> = queries
        .ui_roots
        .get(world)
        .expect("system param validation failed")
        .iter()
        .collect();

    let (node_query, images, mut texture_cache) =
        queries.ui_nodes.get_mut(world).expect("system param validation failed");

    let mut out = Vec::new();
    for root in roots {
        walk_generic_ui_node(root, None, true, &node_query, &images, &mut texture_cache, used_texture_ids, &mut out);
    }
    out.sort_by_key(|node| node.stack_index);
    out
}
