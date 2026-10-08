
use bevy::asset::{AssetId, Assets, Handle};
use bevy::camera::visibility::Visibility;
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
use bevy::transform::components::{GlobalTransform, Transform};
use bevy::ui::widget::Text;
use bevy::ui::{BackgroundColor, BorderColor, ComputedNode, ComputedStackIndex, Display, Node, OverflowAxis, UiGlobalTransform};

use shared::{NetPlayer, Zombie, ZombieKind};

pub fn prime_camera_viewport(world: &mut bevy::ecs::world::World, width_pts: f32, height_pts: f32, scale_factor: f32) {
    let mut state: SystemState<Query<(&mut Camera, &mut Projection), With<crate::FpsCamera>>> = SystemState::new(world);
    let mut query = state.get_mut(world).expect("system param validation failed");
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

    let mut ui_camera_state: SystemState<Query<&mut Camera, With<bevy::ui::IsDefaultUiCamera>>> = SystemState::new(world);
    for mut ui_camera in ui_camera_state.get_mut(world).expect("system param validation failed").iter_mut() {
        ui_camera.computed.target_info = target_info();
    }

    let mut window_state: SystemState<Query<&mut bevy::window::Window, With<bevy::window::PrimaryWindow>>> = SystemState::new(world);
    if let Ok(mut window) = window_state.get_mut(world).expect("system param validation failed").single_mut() {
        window.resolution.set(w, h);
    }
}

#[derive(Resource, Default)]
pub struct DecodedTextureCache(std::collections::HashMap<AssetId<Image>, ExtractedTexture>);

#[derive(Clone)]
pub struct ExtractedTexture {

    pub id: AssetId<Image>,
    pub width: u32,
    pub height: u32,

    pub rgba: std::sync::Arc<[u8]>,
}

pub struct ExtractedMesh {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,

    pub uvs: Vec<[f32; 2]>,

    pub indices: Vec<u32>,
    pub base_color: [f32; 4],

    pub emissive: [f32; 4],
    pub texture: Option<ExtractedTexture>,

    pub tangents: Vec<[f32; 4]>,

    pub normal_texture: Option<ExtractedTexture>,

    pub transform: Transform,

    pub is_blend: bool,

    pub mesh_id: Option<AssetId<Mesh>>,

    pub is_gpu_transformed: bool,
}

pub struct CameraSnapshot {
    pub eye: [f32; 3],
    pub rotation: [f32; 4],
    pub yaw: f32,
    pub pitch: f32,

    pub ads_blend: f32,

    pub scope_zoom: f32,

    pub wall_raise: f32,
}

pub struct ScopeCameraSnapshot {
    pub eye: [f32; 3],
    pub rotation: [f32; 4],

    pub active: bool,

    pub fov: f32,

    pub lens_world_pos: [f32; 3],

    pub lens_local_rotation: [f32; 4],
}

#[derive(Default)]
pub struct RenderSnapshot {
    pub camera: Option<CameraSnapshot>,
    pub scope_camera: Option<ScopeCameraSnapshot>,
    pub zombies: Vec<SkinnedRenderState>,
    pub remote_players: Vec<SkinnedRenderState>,
    pub static_meshes: Vec<ExtractedMesh>,
    pub light: Option<LightSnapshot>,
    pub sun: Option<SunSnapshot>,
    pub map_msaa_on: bool,

    pub shadow_quality: u8,

    pub light_quality: u8,

    pub point_light_quality: u8,

    pub render_resolution_scale: f32,

    pub zombie_texture_quality: u8,

    pub generic_ui_nodes: Vec<GenericUiNode>,
}

pub struct GenericUiNode {
    pub center: [f32; 2],
    pub size: [f32; 2],
    /// Clockwise radians, matching `UiTransform::rotation`.
    pub rotation: f32,
    pub background_color: Option<[f32; 4]>,
    pub border_color: Option<[f32; 4]>,
    pub border_width: f32,
    /// Treated as a circle, not a rounded rect, when >= half the shorter side.
    pub corner_radius: f32,
    pub text: Option<GenericUiText>,
    /// `[x, y, w, h]`, inherited from the nearest clipping ancestor (not self).
    pub clip_rect: Option<[f32; 4]>,
    /// Bevy's own computed paint order (honours `GlobalZIndex`/hierarchy); nodes
    /// are emitted sorted by it so painting follows real UI stacking, not spawn order.
    pub stack_index: u32,
}

pub struct GenericUiText {
    pub content: String,
    pub color: [f32; 4],
    pub font_size: f32,
}

fn collect_descendants(root: bevy::ecs::entity::Entity, children_query: &Query<&Children>, out: &mut Vec<bevy::ecs::entity::Entity>) {
    let Ok(children) = children_query.get(root) else { return };
    for &child in children.iter() {
        out.push(child);
        collect_descendants(child, children_query, out);
    }
}

pub struct SunSnapshot {

    pub direction: [f32; 3],
    pub color: [f32; 3],
}

pub struct LightSnapshot {
    pub position: [f32; 3],

    pub color: [f32; 3],
    pub range: f32,
}

pub struct SkinnedRenderState {
    pub transform: Transform,
    pub joint_matrices: Vec<[[f32; 4]; 4]>,

    pub kind: ZombieKind,
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

impl Default for CameraSnapshot {
    fn default() -> Self {
        Self {
            eye: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            yaw: 0.0,
            pitch: 0.0,
            ads_blend: 0.0,
            scope_zoom: 8.0,
            wall_raise: 0.0,
        }
    }
}

pub fn extract_render_snapshot(world: &mut bevy::ecs::world::World) -> RenderSnapshot {
    let mut snapshot = RenderSnapshot::default();

    let mut camera_state: SystemState<(
        Query<(&GlobalTransform, &crate::FpsCamera)>,
        Query<&crate::Gun>,
        bevy::ecs::system::Res<crate::ScopeZoom>,
    )> = SystemState::new(world);
    let (camera_query, gun_query, scope_zoom) = camera_state.get(world).expect("system param validation failed");
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

    let mut scope_state: SystemState<(
        Query<(&GlobalTransform, &bevy::camera::Camera, &bevy::camera::Projection), With<crate::ScopeCamera>>,
        Query<(&GlobalTransform, &Transform), With<crate::ScopeLens>>,
    )> = SystemState::new(world);
    let (scope_camera_query, scope_lens_query) = scope_state.get(world).expect("system param validation failed");
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

    let mut zombie_state: SystemState<(
        Query<(bevy::ecs::entity::Entity, &GlobalTransform, Option<&ZombieKind>), With<Zombie>>,
        Query<&Children>,
        Query<&SkinnedMesh>,
        Query<&GlobalTransform>,
        Res<Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>>,
    )> = SystemState::new(world);
    let (zombie_query, children_query, skinned_mesh_query, joint_globals, inverse_bindposes_assets) = zombie_state.get(world).expect("system param validation failed");
    snapshot.zombies = zombie_query
        .iter()
        .map(|(entity, transform, kind)| {
            let joint_matrices = find_first_skinned_mesh(entity, &children_query, &skinned_mesh_query)
                .and_then(|mesh_entity| skinned_mesh_query.get(mesh_entity).ok())
                .map(|skinned| joint_matrices_for(skinned, &joint_globals, &inverse_bindposes_assets))
                .unwrap_or_default();
            SkinnedRenderState { transform: transform.compute_transform(), joint_matrices, kind: kind.copied().unwrap_or_default() }
        })
        .collect();

    const LOCAL_PLAYER_EXCLUSION_RADIUS_XZ: f32 = 1.0;
    let camera_eye_xz = snapshot.camera.as_ref().map(|c| bevy::math::Vec2::new(c.eye[0], c.eye[2]));
    let mut remote_state: SystemState<(
        Query<(bevy::ecs::entity::Entity, &GlobalTransform), (With<NetPlayer>, bevy::ecs::query::Without<crate::LocalPlayer>)>,
        Query<&Children>,
        Query<&SkinnedMesh>,
        Query<&GlobalTransform>,
        Res<Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>>,
    )> = SystemState::new(world);
    let (remote_query, remote_children_query, remote_skinned_query, remote_joint_globals, remote_inverse_bindposes_assets) =
        remote_state.get(world).expect("system param validation failed");
    snapshot.remote_players = remote_query
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

            SkinnedRenderState { transform, joint_matrices, kind: ZombieKind::default() }
        })
        .collect();

    let mut light_state: SystemState<Query<(&PointLight, &GlobalTransform)>> = SystemState::new(world);
    snapshot.light = light_state.get(world).expect("system param validation failed").iter().next().map(|(light, transform)| {
        let c = light.color.to_srgba();
        LightSnapshot {
            position: transform.translation().to_array(),
            color: [c.red, c.green, c.blue],
            range: light.range,
        }
    });

    let mut sun_state: SystemState<Query<(&DirectionalLight, &GlobalTransform)>> = SystemState::new(world);
    snapshot.sun = sun_state.get(world).expect("system param validation failed").iter().next().map(|(sun, transform)| {
        let c = sun.color.to_srgba();
        SunSnapshot { direction: transform.forward().to_array(), color: [c.red, c.green, c.blue] }
    });

    let mut msaa_toggle_state: SystemState<Res<crate::MapMsaaOn>> = SystemState::new(world);
    snapshot.map_msaa_on = msaa_toggle_state.get(world).expect("system param validation failed").0;

    let mut shadow_quality_state: SystemState<Res<crate::ShadowQuality>> = SystemState::new(world);
    snapshot.shadow_quality = shadow_quality_state.get(world).expect("system param validation failed").0;
    let mut light_quality_state: SystemState<Res<crate::LightQuality>> = SystemState::new(world);
    snapshot.light_quality = light_quality_state.get(world).expect("system param validation failed").0;
    let mut point_light_quality_state: SystemState<Res<crate::PointLightQuality>> = SystemState::new(world);
    snapshot.point_light_quality = point_light_quality_state.get(world).expect("system param validation failed").0;

    let mut resolution_scale_state: SystemState<Res<crate::RenderResolutionScale>> = SystemState::new(world);
    snapshot.render_resolution_scale = resolution_scale_state.get(world).expect("system param validation failed").0;

    let mut zombie_texture_quality_state: SystemState<Res<crate::ZombieTextureQuality>> = SystemState::new(world);
    snapshot.zombie_texture_quality = zombie_texture_quality_state.get(world).expect("system param validation failed").0;

    let mut gun_visible_state: SystemState<Res<crate::GunVisible>> = SystemState::new(world);
    let gun_visible = gun_visible_state.get(world).expect("system param validation failed").0;

    let mut cars_visible_state: SystemState<Res<crate::CarsVisible>> = SystemState::new(world);
    let cars_visible = cars_visible_state.get(world).expect("system param validation failed").0;
    let mut car_entities_state: SystemState<Query<bevy::ecs::entity::Entity, With<crate::ItemGalleryTestProp>>> = SystemState::new(world);
    let car_entities: std::collections::HashSet<bevy::ecs::entity::Entity> = car_entities_state.get(world).expect("system param validation failed").iter().collect();
    let gun_mesh_entities: std::collections::HashSet<bevy::ecs::entity::Entity> = {
        let mut gun_state: SystemState<(Query<bevy::ecs::entity::Entity, With<crate::Gun>>, Query<&Children>)> = SystemState::new(world);
        let (gun_query, children_query) = gun_state.get(world).expect("system param validation failed");
        let mut out = Vec::new();
        if let Some(gun_entity) = gun_query.iter().next() {
            out.push(gun_entity);
            collect_descendants(gun_entity, &children_query, &mut out);
        }
        out.into_iter().collect()
    };

    world.init_resource::<DecodedTextureCache>();
    let mut mesh_state: SystemState<(
        Query<(bevy::ecs::entity::Entity, &Mesh3d, &MeshMaterial3d<StandardMaterial>, &GlobalTransform), Without<SkinnedMesh>>,
        Res<Assets<Mesh>>,
        Res<Assets<StandardMaterial>>,
        Res<Assets<Image>>,
        ResMut<DecodedTextureCache>,
    )> = SystemState::new(world);
    let (mesh_query, meshes, materials, images, mut texture_cache) = mesh_state.get_mut(world).expect("system param validation failed");

    let mut used_texture_ids: std::collections::HashSet<AssetId<Image>> = std::collections::HashSet::new();
    let mut decode_texture = |handle: &Handle<Image>, images: &Assets<Image>| -> Option<ExtractedTexture> {
        used_texture_ids.insert(handle.id());
        if let Some(cached) = texture_cache.0.get(&handle.id()) {
            return Some(cached.clone());
        }
        let image = images.get(handle)?;
        let data = image.data.as_ref()?;
        match image.texture_descriptor.format {
            TextureFormat::Rgba8UnormSrgb | TextureFormat::Rgba8Unorm => {
                let extracted = ExtractedTexture {
                    id: handle.id(),
                    width: image.texture_descriptor.size.width,
                    height: image.texture_descriptor.size.height,
                    rgba: std::sync::Arc::from(data.as_slice()),
                };
                texture_cache.0.insert(handle.id(), extracted.clone());
                Some(extracted)
            }
            _ => None,
        }
    };
    for (entity, mesh3d, material3d, transform) in mesh_query.iter() {
        let is_gun = gun_mesh_entities.contains(&entity);
        if is_gun && !gun_visible {
            continue;
        }
        let is_car = car_entities.contains(&entity);
        if is_car && !cars_visible {
            continue;
        }
        let Some(mesh) = meshes.get(&mesh3d.0) else { continue };
        let Some(material) = materials.get(&material3d.0) else { continue };

        let positions = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
            Some(bevy::mesh::VertexAttributeValues::Float32x3(p)) => p.clone(),
            _ => continue,
        };
        let normals = match mesh.attribute(Mesh::ATTRIBUTE_NORMAL) {
            Some(bevy::mesh::VertexAttributeValues::Float32x3(n)) => n.clone(),
            _ => Vec::new(),
        };
        let uvs = match mesh.attribute(Mesh::ATTRIBUTE_UV_0) {
            Some(bevy::mesh::VertexAttributeValues::Float32x2(uv)) => uv.clone(),
            _ => Vec::new(),
        };
        let tangents = match mesh.attribute(Mesh::ATTRIBUTE_TANGENT) {
            Some(bevy::mesh::VertexAttributeValues::Float32x4(t)) => t.clone(),
            _ => Vec::new(),
        };
        let indices = mesh
            .indices()
            .map(|i| i.iter().map(|v| v as u32).collect())
            .unwrap_or_default();

        let has_texture_handle = material.base_color_texture.is_some();
        let texture = material.base_color_texture.as_ref().and_then(|handle| decode_texture(handle, &images));

        if has_texture_handle && texture.is_none() {
            continue;
        }

        let normal_texture = if tangents.is_empty() {
            None
        } else {
            material.normal_map_texture.as_ref().and_then(|handle| decode_texture(handle, &images))
        };

        snapshot.static_meshes.push(ExtractedMesh {
            positions,
            normals,
            uvs,
            indices,
            base_color: {
                let s = material.base_color.to_srgba();
                [s.red, s.green, s.blue, s.alpha]
            },
            emissive: [
                material.emissive.red,
                material.emissive.green,
                material.emissive.blue,
                material.emissive.alpha,
            ],
            texture,
            tangents,
            normal_texture,
            transform: transform.compute_transform(),
            is_blend: matches!(
                material.alpha_mode,
                AlphaMode::Blend | AlphaMode::Premultiplied | AlphaMode::Add | AlphaMode::Multiply
            ),
            mesh_id: is_car.then(|| mesh3d.id()),
            is_gpu_transformed: is_car,
        });
    }
    texture_cache.0.retain(|id, _| used_texture_ids.contains(id));

    snapshot.generic_ui_nodes = extract_generic_ui_nodes(world);

    snapshot
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
    out: &mut Vec<GenericUiNode>,
) {
    let Ok((node, computed, stack_index, transform, children, background, border_color, text, visibility)) = query.get(entity) else { return };
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

    if visible && (background_color.is_some() || border_color.is_some() || text_snapshot.is_some()) {
        out.push(GenericUiNode {
            center: center.to_array(),
            size: size.to_array(),
            rotation,
            background_color,
            border_color,
            border_width,
            corner_radius,
            text: text_snapshot,
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
        walk_generic_ui_node(child, child_clip, visible, query, out);
    }
}

pub fn extract_generic_ui_nodes(world: &mut bevy::ecs::world::World) -> Vec<GenericUiNode> {
    let mut roots_state: SystemState<Query<bevy::ecs::entity::Entity, With<crate::NativeUiRoot>>> = SystemState::new(world);
    let roots_query = roots_state.get(world).expect("system param validation failed");
    let roots: Vec<_> = roots_query.iter().collect();

    let mut node_state: SystemState<GenericUiQuery> = SystemState::new(world);
    let node_query = node_state.get(world).expect("system param validation failed");

    let mut out = Vec::new();
    for root in roots {
        walk_generic_ui_node(root, None, true, &node_query, &mut out);
    }
    out.sort_by_key(|node| node.stack_index);
    out
}
