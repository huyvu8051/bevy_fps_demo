
use metal::{Device, MTLResourceOptions};

pub type Mat4 = [[f32; 4]; 4];

pub fn mat4_identity() -> Mat4 {
    [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]
}

pub fn mat4_mul(a: Mat4, b: Mat4) -> Mat4 {
    let mut out = mat4_identity();
    for col in 0..4 {
        for row in 0..4 {
            out[col][row] =
                a[0][row] * b[col][0] + a[1][row] * b[col][1] + a[2][row] * b[col][2] + a[3][row] * b[col][3];
        }
    }
    out
}

fn mat4_from_trs(t: [f32; 3], r: [f32; 4], s: [f32; 3]) -> Mat4 {
    let (x, y, z, w) = (r[0], r[1], r[2], r[3]);
    let (x2, y2, z2) = (x + x, y + y, z + z);
    let (xx, xy, xz) = (x * x2, x * y2, x * z2);
    let (yy, yz, zz) = (y * y2, y * z2, z * z2);
    let (wx, wy, wz) = (w * x2, w * y2, w * z2);
    [
        [(1.0 - (yy + zz)) * s[0], (xy + wz) * s[0], (xz - wy) * s[0], 0.0],
        [(xy - wz) * s[1], (1.0 - (xx + zz)) * s[1], (yz + wx) * s[1], 0.0],
        [(xz + wy) * s[2], (yz - wx) * s[2], (1.0 - (xx + yy)) * s[2], 0.0],
        [t[0], t[1], t[2], 1.0],
    ]
}

fn quat_normalize(q: [f32; 4]) -> [f32; 4] {
    let len = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt();
    if len > 1e-6 { [q[0] / len, q[1] / len, q[2] / len, q[3] / len] } else { [0.0, 0.0, 0.0, 1.0] }
}

fn quat_nlerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
    let b = if dot < 0.0 { [-b[0], -b[1], -b[2], -b[3]] } else { b };
    quat_normalize([
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ])
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct SkinnedVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub joints: [u16; 4],
    pub weights: [f32; 4],
}

pub struct SkinnedMesh {
    pub vertex_buffer: metal::Buffer,
    pub index_buffer: metal::Buffer,
    pub index_count: usize,
    pub base_color_texture: Option<metal::Texture>,
    pub base_color_factor: [f32; 4],
}

#[derive(Clone, Copy)]
struct Trs {
    t: [f32; 3],
    r: [f32; 4],
    s: [f32; 3],
}

enum ChannelData {
    Translation(Vec<[f32; 3]>),
    Rotation(Vec<[f32; 4]>),
    Scale(Vec<[f32; 3]>),
}

struct Channel {
    node: usize,
    times: Vec<f32>,
    data: ChannelData,
}

pub struct AnimationClip {
    channels: Vec<Channel>,
    pub duration: f32,
}

pub struct SkinnedModel {
    pub meshes: Vec<SkinnedMesh>,
    pub animations: Vec<AnimationClip>,
    node_count: usize,
    node_children: Vec<Vec<usize>>,
    node_rest_trs: Vec<Trs>,
    scene_roots: Vec<usize>,
    joint_nodes: Vec<usize>,
    inverse_bind_matrices: Vec<Mat4>,
}

fn sample_channel(times: &[f32], sample_time: f32) -> (usize, usize, f32) {
    if times.len() < 2 {
        return (0, 0, 0.0);
    }
    if sample_time <= times[0] {
        return (0, 0, 0.0);
    }
    if sample_time >= *times.last().unwrap() {
        let last = times.len() - 1;
        return (last, last, 0.0);
    }
    let mut hi = 1;
    while hi < times.len() && times[hi] < sample_time {
        hi += 1;
    }
    let lo = hi - 1;
    let span = (times[hi] - times[lo]).max(1e-6);
    let t = (sample_time - times[lo]) / span;
    (lo, hi, t)
}

impl SkinnedModel {

    pub fn compute_joint_matrices(&self, clip_index: usize, time_secs: f32) -> Vec<Mat4> {
        let mut local_trs = self.node_rest_trs.clone();

        if let Some(clip) = self.animations.get(clip_index) {
            let t = if clip.duration > 0.0 { time_secs.rem_euclid(clip.duration) } else { 0.0 };
            for channel in &clip.channels {
                let (lo, hi, blend) = sample_channel(&channel.times, t);
                let trs = &mut local_trs[channel.node];
                match &channel.data {
                    ChannelData::Translation(values) => trs.t = lerp3(values[lo], values[hi], blend),
                    ChannelData::Rotation(values) => trs.r = quat_nlerp(values[lo], values[hi], blend),
                    ChannelData::Scale(values) => trs.s = lerp3(values[lo], values[hi], blend),
                }
            }
        }

        let mut world = vec![mat4_identity(); self.node_count];
        let mut stack: Vec<(usize, Mat4)> = self.scene_roots.iter().map(|&r| (r, mat4_identity())).collect();
        while let Some((node, parent_world)) = stack.pop() {
            let trs = local_trs[node];
            let local = mat4_from_trs(trs.t, trs.r, trs.s);
            let node_world = mat4_mul(parent_world, local);
            world[node] = node_world;
            for &child in &self.node_children[node] {
                stack.push((child, node_world));
            }
        }

        self.joint_nodes
            .iter()
            .zip(self.inverse_bind_matrices.iter())
            .map(|(&joint_node, &inverse_bind)| mat4_mul(world[joint_node], inverse_bind))
            .collect()
    }
}

fn read_mat4(raw: [[f32; 4]; 4]) -> Mat4 {
    raw
}

pub fn load_skinned_gltf(device: &Device, path: &std::path::Path) -> Option<SkinnedModel> {
    let (document, buffers, images) = match gltf::import(path) {
        Ok(result) => result,
        Err(err) => {
            eprintln!("[skinned_gltf] failed to load {path:?}: {err}");
            return None;
        }
    };

    let textures: Vec<Option<metal::Texture>> = images
        .iter()
        .map(|image| Some(super::metal_texture::upload_texture(device, image.width, image.height, &super::gltf_scene::to_rgba8(image))))
        .collect();

    let node_count = document.nodes().count();
    let mut node_children: Vec<Vec<usize>> = vec![Vec::new(); node_count];
    let mut node_rest_trs: Vec<Trs> = Vec::with_capacity(node_count);
    for node in document.nodes() {
        let (t, r, s) = node.transform().decomposed();
        node_rest_trs.push(Trs { t, r, s });
        for child in node.children() {
            node_children[node.index()].push(child.index());
        }
    }
    let scene_roots: Vec<usize> = document
        .default_scene()
        .or_else(|| document.scenes().next())
        .map(|scene| scene.nodes().map(|n| n.index()).collect())
        .unwrap_or_default();

    let Some(skin) = document.skins().next() else {
        eprintln!("[skinned_gltf] {path:?} has no skin");
        return None;
    };
    let joint_nodes: Vec<usize> = skin.joints().map(|j| j.index()).collect();
    let inverse_bind_matrices: Vec<Mat4> = skin
        .reader(|buffer| Some(&buffers[buffer.index()]))
        .read_inverse_bind_matrices()
        .map(|iter| iter.map(read_mat4).collect())
        .unwrap_or_else(|| vec![mat4_identity(); joint_nodes.len()]);

    let mut animations = Vec::new();
    for animation in document.animations() {
        let mut channels = Vec::new();
        let mut duration = 0.0f32;
        for channel in animation.channels() {
            let node = channel.target().node().index();
            let reader = channel.reader(|buffer| Some(&buffers[buffer.index()]));
            let Some(times): Option<Vec<f32>> = reader.read_inputs().map(|it| it.collect()) else { continue };
            if let Some(&last) = times.last() {
                duration = duration.max(last);
            }
            let Some(outputs) = reader.read_outputs() else { continue };
            let data = match outputs {
                gltf::animation::util::ReadOutputs::Translations(it) => ChannelData::Translation(it.collect()),
                gltf::animation::util::ReadOutputs::Rotations(it) => ChannelData::Rotation(it.into_f32().collect()),
                gltf::animation::util::ReadOutputs::Scales(it) => ChannelData::Scale(it.collect()),
                gltf::animation::util::ReadOutputs::MorphTargetWeights(_) => continue,
            };
            channels.push(Channel { node, times, data });
        }
        animations.push(AnimationClip { channels, duration });
    }

    let mut meshes = Vec::new();
    for node in document.nodes() {
        if node.skin().is_none() {
            continue;
        }
        let Some(mesh) = node.mesh() else { continue };
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
            let Some(positions): Option<Vec<[f32; 3]>> = reader.read_positions().map(|it| it.collect()) else { continue };
            let normals: Vec<[f32; 3]> =
                reader.read_normals().map(|it| it.collect()).unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);
            let uvs: Vec<[f32; 2]> = reader
                .read_tex_coords(0)
                .map(|uv| uv.into_f32().collect())
                .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);
            let joints: Vec<[u16; 4]> = reader
                .read_joints(0)
                .map(|it| it.into_u16().collect())
                .unwrap_or_else(|| vec![[0, 0, 0, 0]; positions.len()]);
            let weights: Vec<[f32; 4]> = reader
                .read_weights(0)
                .map(|it| it.into_f32().collect())
                .unwrap_or_else(|| vec![[1.0, 0.0, 0.0, 0.0]; positions.len()]);
            let Some(indices): Option<Vec<u32>> = reader.read_indices().map(|it| it.into_u32().collect()) else { continue };
            if positions.is_empty() || indices.is_empty() {
                continue;
            }

            let vertices: Vec<SkinnedVertex> = (0..positions.len())
                .map(|i| SkinnedVertex {
                    position: positions[i],
                    normal: normals[i],
                    uv: uvs[i],
                    joints: joints[i],
                    weights: weights[i],
                })
                .collect();

            let vertex_buffer = device.new_buffer_with_data(
                vertices.as_ptr() as *const _,
                (vertices.len() * std::mem::size_of::<SkinnedVertex>()) as u64,
                MTLResourceOptions::StorageModeShared,
            );
            let index_buffer = device.new_buffer_with_data(
                indices.as_ptr() as *const _,
                (indices.len() * std::mem::size_of::<u32>()) as u64,
                MTLResourceOptions::StorageModeShared,
            );

            let pbr = primitive.material().pbr_metallic_roughness();

            let base_color_texture =
                pbr.base_color_texture().and_then(|info| textures.get(info.texture().source().index()).cloned().flatten());

            meshes.push(SkinnedMesh {
                vertex_buffer,
                index_buffer,
                index_count: indices.len(),
                base_color_texture,
                base_color_factor: pbr.base_color_factor(),
            });
        }
    }

    eprintln!(
        "[skinned_gltf] loaded {} meshes, {} joints, {} animations from {path:?}",
        meshes.len(),
        joint_nodes.len(),
        animations.len()
    );

    Some(SkinnedModel {
        meshes,
        animations,
        node_count,
        node_children,
        node_rest_trs,
        scene_roots,
        joint_nodes,
        inverse_bind_matrices,
    })
}
