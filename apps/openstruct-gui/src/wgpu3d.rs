//! Optional wgpu pass for shaded frame sections in the 3D viewport.
//!
//! The existing egui painter remains responsible for grids, wireframes, labels
//! and picking. This pass paints solid section boxes underneath those overlays.
//! It uses the main egui render pass (no depth attachment), so visible box faces
//! are culled and sorted back-to-front on the CPU before drawing.

use crate::canvas::Camera3D;
use bytemuck::{Pod, Zeroable};
use eframe::egui;
use eframe::egui_wgpu::{self, wgpu};
use eframe::wgpu::util::DeviceExt as _;
use nalgebra::{Matrix4, Perspective3, Point3, Vector3};
use openstruct::elements::{cardinal_offset, Element};
use openstruct::fem::frame_rotation;
use openstruct::model::Model;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct SectionVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

impl SectionVertex {
    const ATTRS: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];

    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRS,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Uniforms {
    pub view_proj: [[f32; 4]; 4],
    pub clip: [f32; 4],
    pub color: [f32; 4],
    pub light: [f32; 4],
}

pub struct SectionsCallback {
    pub vertices: Vec<SectionVertex>,
    pub uniforms: Uniforms,
}

struct SectionResources {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    vertex_buffer: wgpu::Buffer,
    vertex_capacity: usize,
}

/// Create the pipeline once and keep it beside egui's render resources.
/// Returns false when the app is not running on the wgpu backend.
pub fn install(cc: &eframe::CreationContext<'_>) -> bool {
    let Some(render_state) = cc.wgpu_render_state.as_ref() else {
        return false;
    };
    let device = &render_state.device;
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("openstruct-sections"),
        source: wgpu::ShaderSource::Wgsl(include_str!("wgpu3d.wgsl").into()),
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("openstruct-sections"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("openstruct-sections"),
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("openstruct-sections"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[Some(SectionVertex::layout())],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(render_state.target_format.into())],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });
    let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("openstruct-sections-uniforms"),
        contents: bytemuck::bytes_of(&Uniforms::zeroed()),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("openstruct-sections"),
        layout: &layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform_buffer.as_entire_binding(),
        }],
    });
    let vertex_capacity = 24 * 3;
    let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("openstruct-sections-vertices"),
        size: vertex_capacity as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    render_state.renderer.write().callback_resources.insert(SectionResources {
        pipeline,
        bind_group,
        uniform_buffer,
        vertex_buffer,
        vertex_capacity,
    });
    true
}

impl egui_wgpu::CallbackTrait for SectionsCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(resources) = resources.get_mut::<SectionResources>() {
            queue.write_buffer(&resources.uniform_buffer, 0, bytemuck::bytes_of(&self.uniforms));
            if !self.vertices.is_empty() {
                let bytes = bytemuck::cast_slice(&self.vertices);
                if bytes.len() > resources.vertex_capacity {
                    resources.vertex_capacity = bytes.len().next_multiple_of(4);
                    resources.vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("openstruct-sections-vertices"),
                        size: resources.vertex_capacity as u64,
                        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    });
                }
                queue.write_buffer(&resources.vertex_buffer, 0, bytes);
            }
        }
        Vec::new()
    }

    fn paint(
        &self,
        info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        if self.vertices.is_empty() {
            return;
        }
        let Some(resources) = resources.get::<SectionResources>() else {
            return;
        };
        let viewport = info.viewport_in_pixels();
        render_pass.set_viewport(
            viewport.left_px as f32,
            viewport.top_px as f32,
            viewport.width_px as f32,
            viewport.height_px as f32,
            0.0,
            1.0,
        );
        render_pass.set_pipeline(&resources.pipeline);
        render_pass.set_bind_group(0, &resources.bind_group, &[]);
        render_pass.set_vertex_buffer(0, resources.vertex_buffer.slice(..));
        render_pass.draw(0..self.vertices.len() as u32, 0..1);
        // egui assumes a full-screen viewport for the meshes that follow this
        // callback. Leaving the canvas viewport active would scale/clip the
        // labels and other overlays painted after the 3D pass.
        render_pass.set_viewport(
            0.0,
            0.0,
            info.screen_size_px[0] as f32,
            info.screen_size_px[1] as f32,
            0.0,
            1.0,
        );
    }
}

/// Camera and shading uniforms matching the painter viewport's perspective.
pub fn camera_uniforms(camera: &Camera3D, aspect: f32, clip_z: Option<f64>, color: egui::Color32) -> Uniforms {
    let eye = camera.eye();
    let (_, up, _) = camera.basis();
    let view = Matrix4::look_at_rh(
        &Point3::new(eye[0], eye[1], eye[2]),
        &Point3::new(camera.target[0], camera.target[1], camera.target[2]),
        &Vector3::new(up[0], up[1], up[2]),
    );
    // The painter uses focal = height * 1.15, i.e. a half-angle of atan(0.5 / 1.15).
    let fov_y = 2.0 * (0.5_f64 / 1.15).atan();
    let projection = Perspective3::new(aspect.max(0.01) as f64, fov_y, 0.05, 5000.0).to_homogeneous();
    let view_proj = projection * view;
    let mut matrix = [[0.0_f32; 4]; 4];
    for column in 0..4 {
        for row in 0..4 {
            matrix[column][row] = view_proj[(row, column)] as f32;
        }
    }
    // dot(world, xyz) + w < 0 is clipped. Keep everything below clip_z.
    let clip = match clip_z {
        Some(z) => [0.0, 0.0, -1.0, z as f32],
        None => [0.0, 0.0, 0.0, 1.0],
    };
    Uniforms {
        view_proj: matrix,
        clip,
        color: [
            color.r() as f32 / 255.0,
            color.g() as f32 / 255.0,
            color.b() as f32 / 255.0,
            1.0,
        ],
        light: [0.4, 0.25, 0.85, 0.0],
    }
}

/// Solid section boxes for frames on one story (or all frames when `story_z`
/// is `None`). Faces are culled against the eye and sorted far-to-near because
/// egui's render pass has no depth buffer.
pub fn section_vertices(model: &Model, story_z: Option<f64>, eye: [f64; 3]) -> Vec<SectionVertex> {
    let mut faces: Vec<(f64, [SectionVertex; 6])> = Vec::new();
    for element in model.elements.values() {
        let (Some(ni), Some(nj)) = (model.nodes.get(&element.node_i), model.nodes.get(&element.node_j)) else {
            continue;
        };
        if let Some(z) = story_z {
            let both = (ni.z - z).abs() < 1e-6 && (nj.z - z).abs() < 1e-6;
            let starts_here = (ni.z.min(nj.z) - z).abs() < 1e-6;
            if !both && !starts_here {
                continue;
            }
        }
        let Some(section) = model.sections.get(&element.section_id) else {
            continue;
        };
        add_frame_faces(element, ni.x, ni.y, ni.z, nj.x, nj.y, nj.z, section.width, section.depth, eye, &mut faces);
    }
    for area in model.areas.values() {
        let points = area.polygon(model);
        if points.len() < 3 {
            continue;
        }
        if let Some(z) = story_z {
            if !points.iter().all(|point| (point[2] - z).abs() < 1e-6) {
                continue;
            }
        }
        add_area_faces(&points, eye, &mut faces);
    }
    faces.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut vertices = Vec::with_capacity(faces.len() * 6);
    for (_, face) in faces {
        vertices.extend_from_slice(&face);
    }
    vertices
}

fn add_frame_faces(
    element: &Element,
    xi: f64,
    yi: f64,
    zi: f64,
    xj: f64,
    yj: f64,
    zj: f64,
    width: f64,
    depth: f64,
    eye: [f64; 3],
    faces: &mut Vec<(f64, [SectionVertex; 6])>,
) {
    let dx = xj - xi;
    let dy = yj - yi;
    let dz = zj - zi;
    let length = (dx * dx + dy * dy + dz * dz).sqrt();
    if length < 1e-9 || width <= 0.0 || depth <= 0.0 {
        return;
    }
    let rotation = frame_rotation(dx / length, dy / length, dz / length);
    let axis = [dx / length, dy / length, dz / length];
    let local_y = [rotation[(0, 1)], rotation[(1, 1)], rotation[(2, 1)]];
    let local_z = [rotation[(0, 2)], rotation[(1, 2)], rotation[(2, 2)]];
    let offset = cardinal_offset(
        element.cardinal_point,
        width,
        depth,
        [xi, yi, zi],
        [xj, yj, zj],
    );
    let half_y = [local_y[0] * depth / 2.0, local_y[1] * depth / 2.0, local_y[2] * depth / 2.0];
    let half_z = [local_z[0] * width / 2.0, local_z[1] * width / 2.0, local_z[2] * width / 2.0];
    let start = [xi + offset[0], yi + offset[1], zi + offset[2]];
    let end = [xj + offset[0], yj + offset[1], zj + offset[2]];
    let corner = |base: [f64; 3], sy: f64, sz: f64| -> [f64; 3] {
        [
            base[0] + half_y[0] * sy + half_z[0] * sz,
            base[1] + half_y[1] * sy + half_z[1] * sz,
            base[2] + half_y[2] * sy + half_z[2] * sz,
        ]
    };
    let c = [
        corner(start, -1.0, -1.0),
        corner(start, -1.0, 1.0),
        corner(start, 1.0, 1.0),
        corner(start, 1.0, -1.0),
        corner(end, -1.0, -1.0),
        corner(end, -1.0, 1.0),
        corner(end, 1.0, 1.0),
        corner(end, 1.0, -1.0),
    ];
    let mut quad = |indices: [usize; 4], normal: [f64; 3]| {
        let center = [
            (c[indices[0]][0] + c[indices[1]][0] + c[indices[2]][0] + c[indices[3]][0]) / 4.0,
            (c[indices[0]][1] + c[indices[1]][1] + c[indices[2]][1] + c[indices[3]][1]) / 4.0,
            (c[indices[0]][2] + c[indices[1]][2] + c[indices[2]][2] + c[indices[3]][2]) / 4.0,
        ];
        if normal[0] * (eye[0] - center[0]) + normal[1] * (eye[1] - center[1]) + normal[2] * (eye[2] - center[2]) <= 0.0 {
            return;
        }
        let normal = [normal[0] as f32, normal[1] as f32, normal[2] as f32];
        let vertex = |p: [f64; 3]| SectionVertex {
            position: [p[0] as f32, p[1] as f32, p[2] as f32],
            normal,
        };
        let face = [
            vertex(c[indices[0]]),
            vertex(c[indices[1]]),
            vertex(c[indices[2]]),
            vertex(c[indices[0]]),
            vertex(c[indices[2]]),
            vertex(c[indices[3]]),
        ];
        let depth = (center[0] - eye[0]).powi(2) + (center[1] - eye[1]).powi(2) + (center[2] - eye[2]).powi(2);
        faces.push((depth, face));
    };
    let end_normal = [axis[0], axis[1], axis[2]];
    let neg_axis = [-axis[0], -axis[1], -axis[2]];
    quad([0, 3, 2, 1], neg_axis);
    quad([4, 5, 6, 7], end_normal);
    quad([0, 1, 5, 4], [-local_z[0], -local_z[1], -local_z[2]]);
    quad([2, 3, 7, 6], local_z);
    quad([0, 4, 7, 3], [-local_y[0], -local_y[1], -local_y[2]]);
    quad([1, 2, 6, 5], local_y);
}

fn add_area_faces(points: &[[f64; 3]], eye: [f64; 3], faces: &mut Vec<(f64, [SectionVertex; 6])>) {
    let mut normal = [0.0; 3];
    for index in 0..points.len() {
        let a = points[index];
        let b = points[(index + 1) % points.len()];
        normal[0] += (a[1] - b[1]) * (a[2] + b[2]);
        normal[1] += (a[2] - b[2]) * (a[0] + b[0]);
        normal[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }
    let length = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
    if length < 1e-12 {
        return;
    }
    normal = [normal[0] / length, normal[1] / length, normal[2] / length];
    let centroid = points.iter().fold([0.0; 3], |mut sum, point| {
        sum[0] += point[0];
        sum[1] += point[1];
        sum[2] += point[2];
        sum
    });
    let n = points.len() as f64;
    let centroid = [centroid[0] / n, centroid[1] / n, centroid[2] / n];
    if normal[0] * (eye[0] - centroid[0]) + normal[1] * (eye[1] - centroid[1]) + normal[2] * (eye[2] - centroid[2]) < 0.0 {
        normal = [-normal[0], -normal[1], -normal[2]];
    }
    let normal = [normal[0] as f32, normal[1] as f32, normal[2] as f32];
    for index in 1..points.len() - 1 {
        let triangle = [points[0], points[index], points[index + 1]];
        let face = [
            SectionVertex { position: triangle[0].map(|v| v as f32), normal },
            SectionVertex { position: triangle[1].map(|v| v as f32), normal },
            SectionVertex { position: triangle[2].map(|v| v as f32), normal },
            SectionVertex { position: triangle[0].map(|v| v as f32), normal },
            SectionVertex { position: triangle[1].map(|v| v as f32), normal },
            SectionVertex { position: triangle[2].map(|v| v as f32), normal },
        ];
        let center = [
            (triangle[0][0] + triangle[1][0] + triangle[2][0]) / 3.0,
            (triangle[0][1] + triangle[1][1] + triangle[2][1]) / 3.0,
            (triangle[0][2] + triangle[1][2] + triangle[2][2]) / 3.0,
        ];
        let depth = (center[0] - eye[0]).powi(2) + (center[1] - eye[1]).powi(2) + (center[2] - eye[2]).powi(2);
        faces.push((depth, face));
    }
}
